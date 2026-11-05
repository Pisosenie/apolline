// Who's listening: the apps recording from a microphone, heard from the sound server as streams
// come and go rather than asked on a timer.

use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::shell::Message;

pub fn subscription() -> Subscription<Message> {
    Subscription::run(watch)
}

fn watch() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| {
        let mut last: Option<Vec<String>> = None;
        loop {
            let mut command = tokio::process::Command::new("pactl");
            command
                .arg("subscribe")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true);
            crate::child::tie(command.as_std_mut(), libc::SIGTERM);
            let mut child = command.spawn().ok();
            let mut events = child
                .as_mut()
                .and_then(|child| child.stdout.take())
                .map(|out| BufReader::new(out).lines());
            loop {
                let now = crate::controls::run("pactl", &["-f", "json", "list", "source-outputs"])
                    .await
                    .map(|json| listening(&json))
                    .unwrap_or_default();
                if last.as_ref() != Some(&now) {
                    last = Some(now.clone());
                    if output.send(Message::Listening(now)).await.is_err() {
                        return;
                    }
                }
                let changed = match &mut events {
                    Some(lines) => recording_changed(lines).await,
                    None => false,
                };
                if !changed {
                    // No pactl or no sound server: look again in a while.
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    break;
                }
            }
        }
    })
}

// Waits for a recording stream to come, go or pause, then lets its burst pass.
async fn recording_changed(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
) -> bool {
    loop {
        match lines.next_line().await {
            Ok(Some(line)) if line.contains(" on source-output #") => break,
            Ok(Some(_)) => {}
            _ => return false,
        }
    }
    let settle = std::time::Duration::from_millis(150);
    while let Ok(Ok(Some(_))) = tokio::time::timeout(settle, lines.next_line()).await {}
    true
}

// The apps with a running recording stream, once each. A paused stream isn't listening, and a
// volume meter (pavucontrol's peak detect) reads levels, not what's said.
fn listening(json: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(streams)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    let mut apps: Vec<String> = Vec::new();
    for stream in &streams {
        let said = |key: &str| stream["properties"][key].as_str().filter(|s| !s.is_empty());
        if stream["corked"].as_bool() == Some(true) || said("media.name") == Some("Peak detect") {
            continue;
        }
        let name = said("application.name")
            .or(said("application.process.binary"))
            .unwrap_or("An app")
            .to_owned();
        if !apps.contains(&name) {
            apps.push(name);
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_running_recordings_count_and_each_app_once() {
        let json = r#"[
            {"corked": false, "properties": {"application.name": "Firefox", "media.name": "AudioCallbackDriver"}},
            {"corked": false, "properties": {"application.name": "Firefox", "media.name": "second"}},
            {"corked": true, "properties": {"application.name": "Telegram"}},
            {"corked": false, "properties": {"application.name": "pavucontrol", "media.name": "Peak detect"}},
            {"corked": false, "properties": {"application.process.binary": "arecord"}}
        ]"#;
        assert_eq!(listening(json), ["Firefox", "arecord"]);
        assert!(listening("not json").is_empty());
    }
}

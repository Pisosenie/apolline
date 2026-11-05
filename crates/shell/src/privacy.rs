// Who's listening: the apps recording from a microphone, read again when the sound server says a
// recording stream came or went, rather than asked on a timer.

pub async fn read() -> Vec<String> {
    crate::controls::run("pactl", &["-f", "json", "list", "source-outputs"])
        .await
        .map(|json| listening(&json))
        .unwrap_or_default()
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

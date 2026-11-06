// Live wallpapers (videos, shaders and Wallpaper Engine scenes) are drawn by apolline-wall, which
// the shell runs as a child: commands go to its stdin as JSON lines, events come back on its
// stdout. The shell decides what each screen shows; the child only draws it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::futures::channel::mpsc::{UnboundedSender, unbounded};
use iced::futures::{SinkExt, Stream, StreamExt};
use serde_json::{Value, json};

use crate::shell::Message;

const VIDEO: [&str; 9] = [
    "mp4", "webm", "mkv", "mov", "m4v", "avi", "ogv", "wmv", "gif",
];
const SHADER: [&str; 2] = ["frag", "glsl"];
// A child that keeps dying is let be after this many tries, until the shell starts again.
const TRIES: u32 = 3;

#[derive(Clone, Debug)]
pub enum Event {
    // The child is up and takes commands through this.
    Started(UnboundedSender<String>),
    Ready {
        source: PathBuf,
        poster: PathBuf,
    },
    Failed {
        source: Option<PathBuf>,
        message: String,
    },
    // Not installed, or it gave up: the screens show their posters instead.
    Gone(String),
}

// What a wallpaper's name means when it isn't a picture: a video, a shader, or a Wallpaper
// Engine project folder.
pub fn source(name: &str) -> Option<PathBuf> {
    let path = crate::wallpaper::expand(name)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is = |list: &[&str]| list.iter().any(|known| known.eq_ignore_ascii_case(ext));
    let live = if path.is_dir() {
        path.join("project.json").is_file()
    } else {
        is(&VIDEO) || is(&SHADER)
    };
    live.then_some(path)
}

// Where the frame apolline-wall last sent for a live wallpaper is kept, so the lock screen,
// the switcher and the accent have a picture of it.
pub fn poster(name: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let cache = apolline_config::xdg_dir("XDG_CACHE_HOME", ".cache")?;
    Some(cache.join(format!("apolline/live/{:016x}.png", hasher.finish())))
}

// Keeps a copy of the child's poster, which lives in its runtime folder only while it runs.
pub fn keep_poster(name: &str, from: &Path) -> bool {
    let Some(to) = poster(name) else {
        return false;
    };
    to.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::copy(from, &to))
        .is_ok()
}

pub fn show(output: Option<&str>, source: &Path) -> String {
    let mut show = json!({ "source": source });
    if let Some(output) = output {
        show["output"] = json!(output);
    }
    json!({ "show": show }).to_string()
}

pub fn pause(output: Option<&str>, paused: bool) -> String {
    let mut pause = json!({ "paused": paused });
    if let Some(output) = output {
        pause["output"] = json!(output);
    }
    json!({ "pause": pause }).to_string()
}

pub fn accent(hex: &str) -> String {
    json!({ "uniforms": { "accent": hex } }).to_string()
}

fn event(line: &str) -> Option<Event> {
    let value: Value = serde_json::from_str(line).ok()?;
    if let Some(ready) = value.get("ready") {
        return Some(Event::Ready {
            source: ready.get("source")?.as_str()?.into(),
            poster: ready.get("poster")?.as_str()?.into(),
        });
    }
    let error = value.get("error")?;
    Some(Event::Failed {
        source: error
            .get("source")
            .and_then(Value::as_str)
            .map(PathBuf::from),
        message: error.get("message")?.as_str()?.to_owned(),
    })
}

pub fn subscription(wanted: bool) -> iced::Subscription<Message> {
    if wanted {
        iced::Subscription::run(run).map(Message::Live)
    } else {
        iced::Subscription::none()
    }
}

fn run() -> impl Stream<Item = Event> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    iced::stream::channel(8, async |mut output| {
        let mut tries = 0;
        let gone = loop {
            let mut command = tokio::process::Command::new("apolline-wall");
            command
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .kill_on_drop(true);
            crate::child::tie(command.as_std_mut(), libc::SIGTERM);
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(err) => break format!("apolline-wall: {err}"),
            };
            let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
                break "apolline-wall gave no pipes".to_owned();
            };
            let (commands, mut sent) = unbounded::<String>();
            if output.send(Event::Started(commands)).await.is_err() {
                return;
            }
            let started = std::time::Instant::now();
            let mut lines = BufReader::new(stdout).lines();
            loop {
                let next =
                    iced::futures::future::select(Box::pin(lines.next_line()), sent.next()).await;
                match next {
                    iced::futures::future::Either::Left((Ok(Some(line)), _)) => {
                        if let Some(event) = event(&line)
                            && output.send(event).await.is_err()
                        {
                            return;
                        }
                    }
                    iced::futures::future::Either::Left(_) => break,
                    iced::futures::future::Either::Right((Some(line), _)) => {
                        let written = stdin.write_all(line.as_bytes()).await;
                        if written.is_err() || stdin.write_all(b"\n").await.is_err() {
                            break;
                        }
                    }
                    // The shell let go of its sender: nothing more to say, so the child goes.
                    iced::futures::future::Either::Right((None, _)) => return,
                }
            }
            let _ = child.kill().await;
            // A child that ran for a while gets its tries back.
            if started.elapsed() > Duration::from_secs(60) {
                tries = 0;
            }
            tries += 1;
            if tries >= TRIES {
                break "apolline-wall kept stopping".to_owned();
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        };
        let _ = output.send(Event::Gone(gone)).await;
        std::future::pending::<()>().await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_events_are_apolline_walls_json() {
        assert_eq!(
            show(Some("DP-1"), Path::new("/w/clip.mp4")),
            r#"{"show":{"output":"DP-1","source":"/w/clip.mp4"}}"#
        );
        assert_eq!(pause(None, true), r#"{"pause":{"paused":true}}"#);
        assert_eq!(accent("#86bdf0"), r##"{"uniforms":{"accent":"#86bdf0"}}"##);
        let ready = event(r#"{"ready":{"output":"DP-1","source":"/w/a","poster":"/run/p.png"}}"#);
        assert!(
            matches!(ready, Some(Event::Ready { ref poster, .. }) if poster == Path::new("/run/p.png"))
        );
        let failed = event(r#"{"error":{"message":"no mpv"}}"#);
        assert!(
            matches!(failed, Some(Event::Failed { source: None, ref message }) if message == "no mpv")
        );
        assert!(event(r#"{"status":{}}"#).is_none());
    }

    #[test]
    fn videos_shaders_and_project_folders_are_live_and_pictures_are_not() {
        let dir = std::env::temp_dir().join(format!("apolline-live-{}", std::process::id()));
        let project = dir.join("scene");
        std::fs::create_dir_all(&project).unwrap();
        let named = |path: &Path| path.to_str().unwrap().to_owned();
        assert_eq!(
            source(&named(&project)),
            None,
            "a folder without project.json"
        );
        std::fs::write(project.join("project.json"), "{}").unwrap();
        assert_eq!(source(&named(&project)), Some(project.clone()));
        assert!(source(&named(&dir.join("clip.WEBM"))).is_some());
        assert!(source(&named(&dir.join("waves.frag"))).is_some());
        assert_eq!(source(&named(&dir.join("photo.jpg"))), None);
        assert_eq!(source("fog.jpg"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

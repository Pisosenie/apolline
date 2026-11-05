use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream as StdStream;
use std::path::PathBuf;
use std::time::Duration;

use iced::Subscription;
use iced::futures::channel::mpsc::Sender;
use iced::futures::{SinkExt, Stream};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::controls::Nudge;
use crate::shell::Message;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Mode(String),
    NextMode,
    NoMode,
    Switcher,
    Launcher,
    Panel,
    Clipboard,
    Keys,
    Replay,
    Terminal,
    Machine,
    Updates,
    // Runs [[module]] commands again: the one by that name, or all of them.
    Module(Option<String>),
    Timer(std::time::Duration, Option<String>),
    StopTimers,
    Nudge(Nudge),
    // From the lock screen once it's up.
    Locked,
}

impl Command {
    pub fn parse(line: &str) -> Result<Self, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["switch"] => Ok(Command::Switcher),
            ["launcher"] => Ok(Command::Launcher),
            ["panel"] => Ok(Command::Panel),
            ["clipboard"] => Ok(Command::Clipboard),
            ["keys"] => Ok(Command::Keys),
            ["replay"] => Ok(Command::Replay),
            ["terminal"] => Ok(Command::Terminal),
            ["machine"] => Ok(Command::Machine),
            ["updates"] => Ok(Command::Updates),
            ["locked"] => Ok(Command::Locked),
            ["timer", "stop"] => Ok(Command::StopTimers),
            ["timer", ..] => crate::timers::parse(line)
                .map(|(length, label)| Command::Timer(length, label))
                .ok_or_else(|| "how long? like timer 25m tea, or timer 1h30m".to_owned()),
            ["module"] => Ok(Command::Module(None)),
            ["module", name @ ..] => Ok(Command::Module(Some(name.join(" ")))),
            words @ ["volume" | "mic" | "brightness", _] => Nudge::parse(words)
                .map(Command::Nudge)
                .ok_or_else(|| format!("unknown command “{}”", line.trim())),
            ["mode", "next"] => Ok(Command::NextMode),
            ["mode", "none" | "off"] => Ok(Command::NoMode),
            ["mode", name @ ..] if !name.is_empty() => Ok(Command::Mode(name.join(" "))),
            ["mode"] => Err("which mode? give its name, next or none".into()),
            _ => Err(format!("unknown command “{}”", line.trim())),
        }
    }
}

// Never the shared /tmp: another user could take the socket or the lock's guard there first.
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(own_dir)
}

// Where logind makes the runtime dir anyway, private to the user.
#[cfg(target_os = "linux")]
fn own_dir() -> PathBuf {
    // SAFETY: getuid can't fail and touches nothing.
    PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() }))
}

// The temp dir on a Mac is already one per user.
#[cfg(not(target_os = "linux"))]
fn own_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn socket() -> PathBuf {
    runtime_dir().join("apolline.sock")
}

pub fn me() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.to_str()
                .map(|p| format!("'{}'", p.replace('\'', r"'\''")))
        })
        .unwrap_or_else(|| "apolline".to_owned())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shared {
    pub mode: Option<String>,
    pub waiting: usize,
    pub reason: Option<String>,
    pub back: Option<String>,
    pub wallpaper: Option<String>,
    // For the lock screen: the weather code and a line about it, and what's on soon.
    pub weather: Option<(u8, String)>,
    pub next: Option<String>,
}

impl Shared {
    fn text(&self) -> String {
        let mode = self.mode.as_deref().unwrap_or_default();
        let mut text = format!("mode {mode}\nwaiting {}\n", self.waiting);
        if let Some(wallpaper) = &self.wallpaper {
            text.push_str(&format!("wallpaper {wallpaper}\n"));
        }
        if let Some((code, line)) = &self.weather {
            text.push_str(&format!("weather {code} {line}\n"));
        }
        if let Some(next) = &self.next {
            text.push_str(&format!("next {next}\n"));
        }
        if let Some(reason) = &self.reason {
            let back = self.back.as_deref().unwrap_or_default();
            text.push_str(&format!("reason {reason}\nback {back}\n"));
        }
        text
    }

    fn parse(text: &str) -> Self {
        let mut shared = Self::default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("mode", key)) if !key.is_empty() => shared.mode = Some(key.to_owned()),
                Some(("waiting", count)) => shared.waiting = count.parse().unwrap_or(0),
                Some(("reason", why)) => shared.reason = Some(why.to_owned()),
                Some(("back", key)) if !key.is_empty() => shared.back = Some(key.to_owned()),
                Some(("wallpaper", name)) => shared.wallpaper = Some(name.to_owned()),
                Some(("weather", rest)) => {
                    shared.weather = rest
                        .split_once(' ')
                        .and_then(|(code, line)| Some((code.parse().ok()?, line.to_owned())))
                }
                Some(("next", what)) => shared.next = Some(what.to_owned()),
                _ => {}
            }
        }
        shared
    }
}

pub fn publish(shared: &Shared) {
    let path = runtime_dir().join("apolline.state");
    let staged = path.with_extension("state.new");
    if std::fs::write(&staged, shared.text())
        .and_then(|()| std::fs::rename(&staged, &path))
        .is_err()
    {
        eprintln!("apolline: cannot write {}", path.display());
    }
}

pub fn shared() -> Shared {
    std::fs::read_to_string(runtime_dir().join("apolline.state"))
        .map(|text| Shared::parse(&text))
        .unwrap_or_default()
}

pub fn listen() -> Subscription<Message> {
    Subscription::run(stream)
}

// A command is one short line; a client that sends more, or nothing, is let go.
const LONGEST: u64 = 4096;
const PATIENCE: Duration = Duration::from_secs(2);

pub fn send(line: &str) -> Result<(), String> {
    let path = socket();
    let mut stream = StdStream::connect(&path).map_err(|_| "not running".to_owned())?;
    // A shell that's stuck shouldn't hang whoever asked, the lock screen least of all.
    let wait = Some(PATIENCE * 2);
    stream
        .set_read_timeout(wait)
        .and_then(|()| stream.set_write_timeout(wait))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    match reply.trim() {
        "ok" => Ok(()),
        other => Err(other.strip_prefix("error: ").unwrap_or(other).to_owned()),
    }
}

fn stream() -> impl Stream<Item = Message> {
    iced::stream::channel(16, async |output| {
        let path = socket();
        if StdStream::connect(&path).is_ok() {
            eprintln!(
                "apolline: another instance is listening on {}",
                path.display()
            );
            return;
        }
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("apolline: cannot listen on {}: {err}", path.display());
                return;
            }
        };
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream, output.clone()));
        }
    })
}

// Each client on its own, so a slow one holds up nobody else.
async fn serve(stream: UnixStream, mut output: Sender<Message>) {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    let mut read = tokio::io::BufReader::new(read.take(LONGEST));
    let line_read = read.read_line(&mut line);
    if !matches!(tokio::time::timeout(PATIENCE, line_read).await, Ok(Ok(_))) {
        return;
    }
    let reply = if line.len() as u64 >= LONGEST {
        // Whatever is left unread would make closing a reset, and the client would miss the answer.
        let mut rest = read.into_inner().into_inner().take(LONGEST * 16);
        let mut sink = tokio::io::sink();
        let _ = tokio::time::timeout(PATIENCE, tokio::io::copy(&mut rest, &mut sink)).await;
        "error: too long\n".to_owned()
    } else {
        match Command::parse(&line) {
            Ok(Command::Locked) => {
                crate::logind::may_sleep();
                "ok\n".to_owned()
            }
            Ok(command) => {
                if output.send(Message::Control(command)).await.is_err() {
                    return;
                }
                "ok\n".to_owned()
            }
            Err(err) => format!("error: {err}\n"),
        }
    };
    let _ = tokio::time::timeout(PATIENCE, write.write_all(reply.as_bytes())).await;
    let _ = write.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_state_round_trips() {
        let shared = Shared {
            mode: Some("night-owl".into()),
            waiting: 2,
            reason: Some("20:00-07:00".into()),
            back: Some("work".into()),
            wallpaper: Some("~/Pictures/Wallpapers/two words.jpg".into()),
            weather: Some((2, "17° Partly cloudy · Lisbon".into())),
            next: Some("Design review · in 10 min".into()),
        };
        assert_eq!(Shared::parse(&shared.text()), shared);
        assert_eq!(Shared::parse("garbage"), Shared::default());
    }

    fn served(sent: &[u8]) -> (String, Option<Message>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (mut client, server) = UnixStream::pair().unwrap();
            let (output, mut messages) = iced::futures::channel::mpsc::channel(1);
            let serving = tokio::spawn(serve(server, output));
            client.write_all(sent).await.unwrap();
            let mut reply = String::new();
            client.read_to_string(&mut reply).await.unwrap();
            serving.await.unwrap();
            (reply, messages.try_recv().ok())
        })
    }

    #[test]
    fn the_socket_answers_a_line_and_turns_down_a_flood() {
        let (reply, message) = served(b"launcher\n");
        assert_eq!(reply, "ok\n");
        assert!(matches!(message, Some(Message::Control(Command::Launcher))));
        let (reply, message) = served(&[b'a'; 5000]);
        assert_eq!(reply, "error: too long\n");
        assert!(message.is_none());
    }

    #[test]
    fn parses_mode_commands() {
        assert_eq!(
            Command::parse("mode Night Owl\n"),
            Ok(Command::Mode("Night Owl".into()))
        );
        assert_eq!(Command::parse("mode none"), Ok(Command::NoMode));
        assert_eq!(Command::parse("  mode   next "), Ok(Command::NextMode));
        assert!(Command::parse("mode").is_err());
        assert_eq!(Command::parse("module"), Ok(Command::Module(None)));
        assert_eq!(Command::parse("locked\n"), Ok(Command::Locked));
        assert_eq!(
            Command::parse("timer 5m eggs"),
            Ok(Command::Timer(
                std::time::Duration::from_secs(300),
                Some("eggs".into())
            ))
        );
        assert!(Command::parse("timer soon").is_err());
        assert_eq!(
            Command::parse("module weather"),
            Ok(Command::Module(Some("weather".into())))
        );
        assert!(
            Command::parse("reboot")
                .unwrap_err()
                .contains("unknown command")
        );
    }
}

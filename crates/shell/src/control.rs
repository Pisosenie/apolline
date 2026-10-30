use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream as StdStream;
use std::path::PathBuf;

use apolline_theme::Mode;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use crate::shell::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Mode(Mode),
    NextMode,
}

impl Command {
    pub fn parse(line: &str) -> Result<Self, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["mode", "next"] => Ok(Command::NextMode),
            ["mode", name] => Mode::from_key(name)
                .map(Command::Mode)
                .ok_or_else(|| format!("unknown mode “{name}”, use work, play, evening or next")),
            ["mode"] => Err("which mode? use work, play, evening or next".into()),
            _ => Err(format!("unknown command “{}”", line.trim())),
        }
    }
}

pub fn socket() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("apolline.sock")
}

pub fn listen() -> Subscription<Message> {
    Subscription::run(stream)
}

pub fn send(line: &str) -> Result<(), String> {
    let path = socket();
    let mut stream = StdStream::connect(&path).map_err(|_| "not running".to_owned())?;
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
    iced::stream::channel(16, async |mut output| {
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
            let (read, mut write) = stream.into_split();
            let mut line = String::new();
            if tokio::io::BufReader::new(read)
                .read_line(&mut line)
                .await
                .is_err()
            {
                continue;
            }
            let reply = match Command::parse(&line) {
                Ok(command) => {
                    if output.send(Message::Control(command)).await.is_err() {
                        return;
                    }
                    "ok\n".to_owned()
                }
                Err(err) => format!("error: {err}\n"),
            };
            let _ = write.write_all(reply.as_bytes()).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mode_commands() {
        assert_eq!(Command::parse("mode play\n"), Ok(Command::Mode(Mode::Play)));
        assert_eq!(Command::parse("  mode   next "), Ok(Command::NextMode));
        assert!(
            Command::parse("mode party")
                .unwrap_err()
                .contains("unknown mode “party”")
        );
        assert!(Command::parse("mode").is_err());
        assert!(
            Command::parse("reboot")
                .unwrap_err()
                .contains("unknown command")
        );
    }
}

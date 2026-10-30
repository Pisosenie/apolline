use std::path::PathBuf;
use std::time::Duration;

use apolline_niri::{Event, Request};
use iced::futures::{SinkExt, Stream};
use iced::{Subscription, Task};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::shell::Message;

pub fn outputs() -> Vec<apolline_niri::Output> {
    if std::env::var_os("NIRI_SOCKET").is_none() {
        return Vec::new();
    }
    std::process::Command::new("niri")
        .args(["msg", "-j", "outputs"])
        .output()
        .map(|out| apolline_niri::outputs(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

pub fn connected() -> bool {
    apolline_niri::socket().is_some()
}

pub fn events() -> Subscription<Message> {
    if connected() {
        Subscription::run(stream)
    } else {
        Subscription::none()
    }
}

pub fn send(request: Request) -> Task<Message> {
    match apolline_niri::socket() {
        Some(path) => Task::perform(ask(path, request), Message::NiriReply),
        None => Task::none(),
    }
}

fn stream() -> impl Stream<Item = Message> {
    iced::stream::channel(64, async |mut output| {
        let Some(path) = apolline_niri::socket() else {
            return;
        };
        loop {
            if let Ok(stream) = UnixStream::connect(&path).await {
                let (read, mut write) = stream.into_split();
                if write
                    .write_all(Request::EventStream.line().as_bytes())
                    .await
                    .is_ok()
                {
                    let mut lines = BufReader::new(read).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Some(event) = Event::parse(&line)
                            && output.send(Message::Niri(event)).await.is_err()
                        {
                            return;
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

async fn ask(path: PathBuf, request: Request) -> Result<(), String> {
    let stream = UnixStream::connect(&path)
        .await
        .map_err(|e| e.to_string())?;
    let (read, mut write) = stream.into_split();
    write
        .write_all(request.line().as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(read)
        .read_line(&mut reply)
        .await
        .map_err(|e| e.to_string())?;
    if apolline_niri::is_ok(&reply) {
        Ok(())
    } else {
        Err(reply.trim().to_owned())
    }
}

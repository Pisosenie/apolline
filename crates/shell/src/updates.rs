// Updates waiting for the system: pacman's through checkupdates, which syncs a copy of the
// databases and needs no root, and the AUR's through paru or yay when one is there. Looked at a
// minute and a half after start, then every hour, and again once an update run finishes.

use std::time::Duration;

use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::launcher::on_path;
use crate::shell::Message;

const FIRST: Duration = Duration::from_secs(90);
const EVERY: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Updates {
    pub system: usize,
    pub aur: usize,
}

impl Updates {
    pub fn total(self) -> usize {
        self.system + self.aur
    }
}

pub fn subscription(on: bool) -> Subscription<Message> {
    if on {
        Subscription::run(ticks)
    } else {
        Subscription::none()
    }
}

fn ticks() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| {
        tokio::time::sleep(FIRST).await;
        loop {
            if output.send(Message::CheckUpdates).await.is_err() {
                return;
            }
            tokio::time::sleep(EVERY).await;
        }
    })
}

fn helper() -> Option<&'static str> {
    ["paru", "yay"].into_iter().find(|helper| on_path(helper))
}

// None when there's no checkupdates (pacman-contrib) to ask, or it couldn't reach a mirror.
pub async fn check() -> Option<Updates> {
    let system = tokio::process::Command::new("checkupdates")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    // 2 is checkupdates saying there's nothing to update.
    let system = match system.status.code() {
        Some(0) => lines(&system.stdout),
        Some(2) => 0,
        _ => return None,
    };
    let aur = match helper() {
        Some(helper) => tokio::process::Command::new(helper)
            .arg("-Qua")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .map_or(0, |out| lines(&out.stdout)),
        None => 0,
    };
    Some(Updates { system, aur })
}

// Package lines, leaving out what's held back with [ignored].
fn lines(out: &[u8]) -> usize {
    String::from_utf8_lossy(out)
        .lines()
        .filter(|line| line.contains(" -> ") && !line.contains("[ignored]"))
        .count()
}

// The run that updates everything, in the terminal; when it's over the bar looks again, and the
// window waits so what happened can be read.
pub fn command() -> String {
    let update = helper().map_or_else(|| "sudo pacman -Syu".to_owned(), |h| format!("{h} -Syu"));
    format!("{update}; apolline updates; printf '\\n%s' 'Done. Enter closes this.'; read _")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_package_lines_count() {
        let out = b"linux 6.17.1-1 -> 6.17.2-1\nmesa 25.2.3-1 -> 25.2.4-1\nfirefox 1 -> 2 [ignored]\n:: warning\n";
        assert_eq!(lines(out), 2);
        assert_eq!(Updates { system: 2, aur: 3 }.total(), 5);
    }
}

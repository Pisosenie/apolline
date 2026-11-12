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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Updates {
    pub system: usize,
    pub aur: usize,
    // A newer Apolline release than this one, by its version.
    pub apolline: Option<String>,
}

impl Updates {
    pub fn total(&self) -> usize {
        self.system + self.aur + usize::from(self.apolline.is_some())
    }
}

const TAGS: &str = "https://api.github.com/repos/Pisosenie/apolline/tags?per_page=30";
// Releases are looked for once a day at most, whatever the bar asks.
const RELEASES_EVERY: Duration = Duration::from_secs(24 * 3600);

// The newest release's version when it's newer than this build; the tags are all there is to
// ask, and only their names are read.
async fn newer_release() -> Option<String> {
    let looked =
        apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")?.join("apolline/release-looked");
    let fresh = std::fs::metadata(&looked)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|age| age < RELEASES_EVERY);
    let cached = std::fs::read_to_string(&looked).ok();
    let newest = if fresh {
        cached
    } else {
        let fetched = tokio::task::spawn_blocking(|| {
            let body = ureq::get(TAGS)
                .header("User-Agent", "apolline")
                .config()
                .timeout_global(Some(Duration::from_secs(10)))
                .build()
                .call()
                .ok()?
                .body_mut()
                .with_config()
                .limit(256 * 1024)
                .read_to_string()
                .ok()?;
            newest(&body)
        })
        .await
        .ok()
        .flatten();
        if let Some(newest) = &fetched {
            let _ = std::fs::write(&looked, newest);
        }
        fetched.or(cached)
    }?;
    (version(&newest)? > version(env!("CARGO_PKG_VERSION"))?).then_some(newest)
}

// The highest vX.Y.Z among the tags.
fn newest(tags: &str) -> Option<String> {
    let tags: Vec<serde_json::Value> = serde_json::from_str(tags).ok()?;
    tags.iter()
        .filter_map(|tag| tag.get("name")?.as_str())
        .filter_map(|name| name.strip_prefix('v'))
        .filter_map(|v| version(v).map(|parts| (parts, v.to_owned())))
        .max()
        .map(|(_, v)| v)
}

fn version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().split('.').map(|part| part.parse().ok());
    let found = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(found)
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
pub async fn check(releases: bool) -> Option<Updates> {
    let apolline = if releases {
        newer_release().await
    } else {
        None
    };
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
    Some(Updates {
        system,
        aur,
        apolline,
    })
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
pub fn command(apolline: bool) -> String {
    let update = helper().map_or_else(|| "sudo pacman -Syu".to_owned(), |h| format!("{h} -Syu"));
    // Apolline isn't in the repositories: it's built again from its own.
    let own = if apolline { "; apolline update" } else { "" };
    format!("{update}{own}; apolline updates; printf '\\n%s' 'Done. Enter closes this.'; read _")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_package_lines_count() {
        let out = b"linux 6.17.1-1 -> 6.17.2-1\nmesa 25.2.3-1 -> 25.2.4-1\nfirefox 1 -> 2 [ignored]\n:: warning\n";
        assert_eq!(lines(out), 2);
        let updates = Updates {
            system: 2,
            aur: 3,
            apolline: Some("0.4.0".into()),
        };
        assert_eq!(updates.total(), 6);
    }

    #[test]
    fn the_newest_release_is_read_from_the_tag_names() {
        let tags = r#"[{"name":"v0.2.0"},{"name":"v0.10.1"},{"name":"v0.9.0"},{"name":"nightly"},{"name":"v1.0"}]"#;
        assert_eq!(newest(tags).as_deref(), Some("0.10.1"));
        assert!(version("0.10.1") > version("0.9.9"));
        assert_eq!(version("1.0"), None);
        assert_eq!(newest("not json"), None);
    }
}

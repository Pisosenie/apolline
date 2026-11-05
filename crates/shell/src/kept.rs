// The notification history on disk, so it outlives the shell and the session. Only what a card
// said is kept, never pictures or buttons: after a restart no app answers them anyway.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::notify::{self, Heard, Notification, Urgency};

const MOST: usize = 50;
const FOR: Duration = Duration::from_secs(7 * 24 * 3600);

pub fn file() -> Option<PathBuf> {
    apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")
        .map(|state| state.join("apolline/notifications.json"))
}

// An app's cards; not Apolline's own, nor what the app said isn't worth keeping.
// A card with a level (volume, brightness, a download) is a live gauge, not something to read later,
// and keeping it would write the file on every step.
fn worth(notification: &Notification) -> bool {
    !notification.transient
        && notification.progress.is_none()
        && (notification.id < notify::CONFIG_CARD || notify::is_past(notification.id))
}

// What's worth keeping, told apart: a card replaced in place comes with a new time.
pub fn marks(history: &[Notification]) -> Vec<(u32, SystemTime)> {
    history
        .iter()
        .filter(|n| worth(n))
        .map(|n| (n.id, n.received))
        .collect()
}

fn to_json(history: &[Notification]) -> Option<String> {
    let cards: Vec<Value> = history
        .iter()
        .filter(|n| worth(n))
        .take(MOST)
        .map(|n| {
            json!({
                "app": n.app,
                "desktop": n.desktop,
                "summary": n.summary,
                "body": n.body,
                "urgency": n.urgency as u8,
                "at": n.received.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
            })
        })
        .collect();
    (!cards.is_empty()).then(|| serde_json::to_string_pretty(&cards).unwrap_or_default())
}

fn from_json(text: &str, now: SystemTime) -> Vec<Notification> {
    let Ok(Value::Array(cards)) = serde_json::from_str(text) else {
        return Vec::new();
    };
    let words = |card: &Value, key: &str| card[key].as_str().unwrap_or_default().to_owned();
    cards
        .iter()
        .filter_map(|card| {
            let at = UNIX_EPOCH + Duration::from_secs(card["at"].as_u64()?);
            let fresh = now.duration_since(at).map_or(true, |age| age <= FOR);
            (fresh && card["app"].is_string()).then_some((card, at))
        })
        .take(MOST)
        .zip(notify::PAST_CARD..)
        .map(|((card, at), id)| Notification {
            id,
            app: words(card, "app"),
            desktop: card["desktop"].as_str().map(str::to_owned),
            actions: Vec::new(),
            image: None,
            progress: None,
            transient: false,
            resident: false,
            reply: None,
            summary: words(card, "summary"),
            body: words(card, "body"),
            urgency: match card["urgency"].as_u64() {
                Some(0) => Urgency::Low,
                Some(2) => Urgency::Critical,
                _ => Urgency::Normal,
            },
            timeout: None,
            received: at,
            sound: Heard::Nothing,
        })
        .collect()
}

pub fn load() -> Vec<Notification> {
    file()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .map(|text| from_json(&text, SystemTime::now()))
        .unwrap_or_default()
}

// Readable by you alone, since what people write you is in it; nothing left once it's cleared.
pub fn save(history: &[Notification]) -> std::io::Result<()> {
    let Some(file) = file() else {
        return Ok(());
    };
    let Some(text) = to_json(history) else {
        return forget();
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let next = file.with_extension("json.new");
    let mut out = std::fs::OpenOptions::new();
    out.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut out, 0o600);
    out.open(&next)?.write_all(text.as_bytes())?;
    std::fs::rename(next, file)
}

pub fn forget() -> std::io::Result<()> {
    match file().map(std::fs::remove_file) {
        Some(Err(err)) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: u32, app: &str, age: u64) -> Notification {
        Notification {
            id,
            app: app.into(),
            desktop: Some(format!("org.{app}")),
            actions: vec![("default".into(), "Open".into())],
            image: None,
            progress: None,
            transient: false,
            resident: false,
            reply: Some("Reply…".into()),
            summary: format!("{app} says"),
            body: "hi".into(),
            urgency: Urgency::Critical,
            timeout: None,
            received: SystemTime::now() - Duration::from_secs(age),
            sound: Heard::Nothing,
        }
    }

    #[test]
    fn what_a_card_said_comes_back_without_what_only_the_app_could_answer() {
        let shot = Notification {
            transient: false,
            ..crate::notify::shot_card("/tmp/a.png")
        };
        let fleeting = Notification {
            transient: true,
            ..card(3, "volume", 0)
        };
        let gauge = Notification {
            progress: Some(40),
            ..card(4, "brightness", 0)
        };
        let history = [
            card(7, "telegram", 120),
            shot,
            fleeting,
            gauge,
            card(2, "mail", 60),
        ];
        let text = to_json(&history).unwrap();
        let back = from_json(&text, SystemTime::now());
        let apps: Vec<&str> = back.iter().map(|n| n.app.as_str()).collect();
        assert_eq!(apps, ["telegram", "mail"]);
        let first = &back[0];
        assert!(notify::is_past(first.id) && notify::is_past(back[1].id));
        assert_ne!(first.id, back[1].id);
        assert_eq!(first.desktop.as_deref(), Some("org.telegram"));
        assert_eq!(
            (first.summary.as_str(), first.urgency),
            ("telegram says", Urgency::Critical)
        );
        assert!(first.actions.is_empty() && first.reply.is_none() && first.progress.is_none());
        assert_eq!(crate::popup::ago(first.age()), "2 min");
    }

    #[test]
    fn a_week_old_card_and_a_broken_file_are_let_go() {
        let text = to_json(&[card(1, "old", 8 * 24 * 3600), card(2, "new", 0)]).unwrap();
        let back = from_json(&text, SystemTime::now());
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].app, "new");
        assert!(from_json("{not json", SystemTime::now()).is_empty());
        assert!(from_json(r#"[{"summary": "no app"}]"#, SystemTime::now()).is_empty());
        assert_eq!(to_json(&[]), None);
    }
}

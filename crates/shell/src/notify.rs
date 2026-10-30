use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use std::path::Path;

use apolline_config::{Diagnostic, Notifications};
use iced::futures::channel::mpsc::Sender;
use iced::futures::{SinkExt, Stream};
use iced::{Subscription, Task};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, OwnedVariant, connection};

use crate::shell::Message;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
pub const SHOWN_FOR: Duration = Duration::from_secs(5);
const CONFIG_SHOWN_FOR: Duration = Duration::from_secs(12);

pub const CONFIG_CARD: u32 = 1 << 31;

static CONNECTION: OnceLock<Connection> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closed {
    Expired = 1,
    Dismissed = 2,
    Requested = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout: Option<Duration>,
    pub received: Instant,
}

pub fn lets_through(setting: Notifications, urgency: Urgency) -> bool {
    match setting {
        Notifications::All => true,
        Notifications::Mentions => urgency >= Urgency::Normal,
        Notifications::Hold => urgency == Urgency::Critical,
    }
}

fn urgency(level: Option<u8>) -> Urgency {
    match level {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

fn timeout(expire_ms: i32, urgency: Urgency) -> Option<Duration> {
    match expire_ms {
        0 => None,
        ms if ms > 0 => Some(Duration::from_millis(ms as u64)),
        _ if urgency == Urgency::Critical => None,
        _ => Some(SHOWN_FOR),
    }
}

pub async fn send(summary: String, body: String) -> Result<(), String> {
    let connection = Connection::session().await.map_err(|e| e.to_string())?;
    let hints: HashMap<&str, zbus::Variant> = HashMap::new();
    let actions: Vec<&str> = Vec::new();
    connection
        .call_method(
            Some(NAME),
            PATH,
            Some(NAME),
            "Notify",
            &("Apolline", 0u32, "", summary, body, actions, hints, -1i32),
        )
        .await
        .map(drop)
        .map_err(|e| e.to_string())
}

pub fn subscription() -> Subscription<Message> {
    Subscription::run(serve)
}

pub fn config_card(path: &Path, diagnostics: &[Diagnostic]) -> Option<Notification> {
    let first = diagnostics.first()?;
    let file = path.file_name().map_or_else(
        || path.display().to_string(),
        |f| f.to_string_lossy().into_owned(),
    );
    let app = if first.line > 0 {
        format!("{file} · line {}", first.line)
    } else {
        file
    };
    let advice = first
        .hint
        .as_deref()
        .map(capitalize)
        .unwrap_or_else(|| "The rest is applied.".into());
    let body = match diagnostics.len() {
        1 => advice,
        n => format!("{advice} +{} more: apolline check", n - 1),
    };
    Some(Notification {
        id: CONFIG_CARD,
        app,
        summary: capitalize(&first.message),
        body,
        urgency: Urgency::Normal,
        timeout: Some(CONFIG_SHOWN_FOR),
        received: Instant::now(),
    })
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

pub fn closed(id: u32, reason: Closed) -> Task<Message> {
    if id >= CONFIG_CARD {
        return Task::none();
    }
    let Some(connection) = CONNECTION.get().cloned() else {
        return Task::none();
    };
    Task::future(async move {
        let _ = connection
            .emit_signal(
                None::<&str>,
                PATH,
                NAME,
                "NotificationClosed",
                &(id, reason as u32),
            )
            .await;
    })
    .discard()
}

struct Server {
    events: Sender<Message>,
    last: u32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&str> {
        vec!["body"]
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id == 0 {
            self.last += 1;
            self.last
        } else {
            replaces_id
        };
        let level = hints
            .get("urgency")
            .and_then(|v| v.downcast_ref::<u8>().ok());
        let urgency = urgency(level);
        let notification = Notification {
            id,
            app: app_name,
            summary,
            body,
            urgency,
            timeout: timeout(expire_timeout, urgency),
            received: Instant::now(),
        };
        let _ = self.events.send(Message::Notify(notification)).await;
        id
    }

    async fn close_notification(&mut self, id: u32) {
        let _ = self.events.send(Message::Unnotify(id)).await;
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("Apolline", "Apolline", env!("CARGO_PKG_VERSION"), "1.2")
    }
}

fn serve() -> impl Stream<Item = Message> {
    iced::stream::channel(32, async |events| {
        let server = Server { events, last: 0 };
        let Ok(connection) = connection::Builder::session()
            .serve_at(PATH, server)
            .build()
            .await
        else {
            return;
        };
        let flags = RequestNameFlags::DO_NOT_QUEUE | RequestNameFlags::ALLOW_REPLACEMENT;
        match connection.request_name_with_flags(NAME, flags).await {
            Ok(RequestNameReply::PrimaryOwner) => {
                let _ = CONNECTION.set(connection);
                std::future::pending::<()>().await;
            }
            Ok(_) => eprintln!("apolline: another notification daemon is running, leaving it be"),
            Err(err) => eprintln!("apolline: notifications: {err}"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_decide_what_gets_through() {
        use Notifications::*;
        use Urgency::*;
        assert!(lets_through(All, Low));
        assert!(!lets_through(Mentions, Low));
        assert!(lets_through(Mentions, Normal));
        assert!(!lets_through(Hold, Normal));
        assert!(lets_through(Hold, Critical));
    }

    #[test]
    fn config_problems_become_a_card() {
        let (_, problems) = apolline_config::Config::parse("[layout]\ngaps_in = 4\nradius = 99\n");
        let card = config_card(
            Path::new("/home/me/.config/apolline/config.toml"),
            &problems,
        )
        .unwrap();
        assert_eq!(card.id, CONFIG_CARD);
        assert_eq!(card.app, "config.toml · line 2");
        assert_eq!(card.summary, "Unknown option “gaps_in”");
        assert_eq!(card.body, "Did you mean “gaps”? +1 more: apolline check");
        assert!(config_card(Path::new("config.toml"), &[]).is_none());
    }

    #[test]
    fn urgency_and_timeout_follow_the_spec() {
        assert_eq!(urgency(Some(0)), Urgency::Low);
        assert_eq!(urgency(None), Urgency::Normal);
        assert_eq!(urgency(Some(2)), Urgency::Critical);
        assert_eq!(timeout(-1, Urgency::Normal), Some(SHOWN_FOR));
        assert_eq!(timeout(-1, Urgency::Critical), None);
        assert_eq!(timeout(0, Urgency::Normal), None);
        assert_eq!(
            timeout(2500, Urgency::Low),
            Some(Duration::from_millis(2500))
        );
    }
}

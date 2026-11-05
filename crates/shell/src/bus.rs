use std::sync::Mutex;
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{StreamExt, stream};
use zbus::match_rule::Builder;
use zbus::message::Type;
use zbus::{Connection, MatchRule, MessageStream};

// Signals come in bursts: a scan touches every access point at once.
const SETTLE: Duration = Duration::from_millis(300);

// Properties changing and objects coming and going on one service. A task of its own empties the
// signal queues as they fill, so a busy service can't stall replies on the same connection.
pub struct Changes {
    seen: mpsc::Receiver<()>,
    drain: tokio::task::JoinHandle<()>,
}

impl Drop for Changes {
    fn drop(&mut self) {
        self.drain.abort();
    }
}

impl Changes {
    // Waits for a change and the rest of its burst. Under a steady stream it still comes back every
    // SETTLE, so the reader keeps up. None once the bus is gone.
    pub async fn settled(&mut self) -> Option<()> {
        self.seen.next().await?;
        tokio::time::sleep(SETTLE).await;
        let _ = self.seen.try_recv();
        Some(())
    }
}

// One connection to the system bus, shared by the watchers and what they switch.
pub async fn system() -> Option<Connection> {
    static SYSTEM: Mutex<Option<Connection>> = Mutex::new(None);
    let kept = SYSTEM.lock().ok().and_then(|kept| kept.clone());
    if kept.is_some() {
        return kept;
    }
    let connection = Connection::system().await.ok()?;
    let mut kept = SYSTEM.lock().ok()?;
    Some(kept.get_or_insert(connection).clone())
}

// Every signal of one interface on a service.
pub fn signals(sender: &'static str, interface: &'static str) -> Builder<'static> {
    MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender)
        .interface(interface)
}

// The properties of one interface changing, on any of the service's objects.
pub fn properties(sender: &'static str, interface: &'static str) -> Builder<'static> {
    signals(sender, "org.freedesktop.DBus.Properties")
        .member("PropertiesChanged")
        .arg(0, interface)
}

// Everything on a service: its properties, and its objects coming and going.
pub async fn changes(connection: &Connection, sender: &'static str) -> Option<Changes> {
    let rules = [
        signals(sender, "org.freedesktop.DBus.Properties"),
        signals(sender, "org.freedesktop.DBus.ObjectManager"),
    ];
    heard(connection, rules).await
}

// Only what the rules ask for: a busy service wakes no one for the rest.
pub async fn heard(
    connection: &Connection,
    rules: impl IntoIterator<Item = Builder<'static>>,
) -> Option<Changes> {
    let mut streams = Vec::new();
    for rule in rules {
        let rule = rule.build().ok()?;
        streams.push(
            MessageStream::for_match_rule(rule, connection, None)
                .await
                .ok()?,
        );
    }
    let (mut tell, seen) = mpsc::channel(1);
    let mut all = stream::select_all(streams);
    let drain = tokio::spawn(async move {
        while all.next().await.is_some() {
            if tell.try_send(()).is_err_and(|err| err.is_disconnected()) {
                return;
            }
        }
    });
    Some(Changes { seen, drain })
}

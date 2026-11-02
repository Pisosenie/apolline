use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{StreamExt, stream};
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

pub async fn changes(connection: &Connection, sender: &'static str) -> Option<Changes> {
    let mut streams = Vec::new();
    for interface in [
        "org.freedesktop.DBus.Properties",
        "org.freedesktop.DBus.ObjectManager",
    ] {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(sender)
            .interface(interface)
            .build()
            .ok()?;
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

use std::time::Duration;

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt};
use zbus::{Connection, OwnedFd, OwnedObjectPath};

use crate::shell::Message;

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn get_session_by_pid(&self, pid: u32) -> zbus::Result<OwnedObjectPath>;
    fn get_session(&self, id: &str) -> zbus::Result<OwnedObjectPath>;
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait Session {
    #[zbus(signal)]
    fn lock(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

pub async fn session_id(connection: &Connection) -> Option<String> {
    let manager = ManagerProxy::new(connection).await.ok()?;
    session(connection, &manager).await?.id().await.ok()
}

enum Event {
    Lock,
    Sleep(bool),
}

pub fn subscription() -> Subscription<Message> {
    Subscription::run(watch)
}

fn watch() -> impl Stream<Item = Message> {
    iced::stream::channel(4, async |mut output| {
        let Ok(connection) = Connection::system().await else {
            return;
        };
        let Ok(manager) = ManagerProxy::new(&connection).await else {
            return;
        };
        let Ok(sleep) = manager.receive_prepare_for_sleep().await else {
            return;
        };
        let sleep = sleep.filter_map(|signal| async move {
            signal.args().ok().map(|args| Event::Sleep(args.start))
        });
        let locks = match session(&connection, &manager).await {
            Some(session) => match session.receive_lock().await {
                Ok(locks) => locks.map(|_| Event::Lock).boxed(),
                Err(_) => iced::futures::stream::pending().boxed(),
            },
            None => iced::futures::stream::pending().boxed(),
        };
        let mut events = iced::futures::stream::select(sleep.boxed(), locks);

        let mut delay = hold(&manager).await;
        while let Some(event) = events.next().await {
            let sent = match event {
                Event::Lock => output.send(Message::LockNow).await,
                Event::Sleep(true) => {
                    let sent = output.send(Message::Sleeping).await;
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    delay = None;
                    sent
                }
                Event::Sleep(false) => {
                    delay = delay.or(hold(&manager).await);
                    Ok(())
                }
            };
            if sent.is_err() {
                return;
            }
        }
    })
}

async fn session<'a>(
    connection: &Connection,
    manager: &ManagerProxy<'a>,
) -> Option<SessionProxy<'a>> {
    let path = match manager.get_session_by_pid(std::process::id()).await {
        Ok(path) => path,
        Err(_) => {
            let id = std::env::var("XDG_SESSION_ID").ok()?;
            manager.get_session(&id).await.ok()?
        }
    };
    SessionProxy::builder(connection)
        .path(path)
        .build()
        .await
        .ok()
}

async fn hold(manager: &ManagerProxy<'_>) -> Option<OwnedFd> {
    manager
        .inhibit("sleep", "Apolline", "Lock the screen before sleep", "delay")
        .await
        .ok()
}

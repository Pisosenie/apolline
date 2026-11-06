use std::time::Duration;

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt};
use zbus::fdo::DBusProxy;
use zbus::names::{BusName, WellKnownName};
use zbus::proxy::CacheProperties;
use zbus::{Connection, OwnedObjectPath};

const FPRINT: &str = "net.reactivated.Fprint";
// Misses before the reader is let go and only the password is left.
const TRIES: usize = 5;
// How often a reader someone else holds is asked again.
const BUSY: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finger {
    // Waiting for a finger; true for a reader you swipe rather than touch.
    Ready(bool),
    Again,
    Matched,
    Gone(&'static str),
}

#[zbus::proxy(
    interface = "net.reactivated.Fprint.Manager",
    default_service = "net.reactivated.Fprint",
    default_path = "/net/reactivated/Fprint/Manager"
)]
trait Manager {
    fn get_default_device(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "net.reactivated.Fprint.Device",
    default_service = "net.reactivated.Fprint"
)]
trait Reader {
    fn list_enrolled_fingers(&self, username: &str) -> zbus::Result<Vec<String>>;
    fn claim(&self, username: &str) -> zbus::Result<()>;
    fn release(&self) -> zbus::Result<()>;
    fn verify_start(&self, finger: &str) -> zbus::Result<()>;
    fn verify_stop(&self) -> zbus::Result<()>;
    #[zbus(property, name = "scan-type")]
    fn scan_type(&self) -> zbus::Result<String>;
    #[zbus(signal)]
    fn verify_status(&self, result: &str, done: bool) -> zbus::Result<()>;
}

// fprintd next to the password: nothing shows unless there is a reader with your fingers on it.
pub fn subscription(on: bool) -> Subscription<Finger> {
    if on {
        Subscription::run(watch)
    } else {
        Subscription::none()
    }
}

fn watch() -> impl Stream<Item = Finger> {
    iced::stream::channel(4, async |mut output| {
        let Ok(connection) = Connection::system().await else {
            return;
        };
        if !trusted(&connection).await {
            return;
        }
        let reader = loop {
            match claim(&connection).await {
                Claim::Ours(reader) => break reader,
                Claim::Busy => tokio::time::sleep(BUSY).await,
                Claim::None => return,
            }
        };
        let swipe = reader.scan_type().await.is_ok_and(|kind| kind == "swipe");
        let Ok(mut statuses) = reader.receive_verify_status().await else {
            return;
        };
        let mut misses = 0;
        let mut said = Finger::Ready(swipe);
        loop {
            if reader.verify_start("any").await.is_err() {
                let _ = output
                    .send(Finger::Gone("The fingerprint reader is busy"))
                    .await;
                return;
            }
            if output.send(said.clone()).await.is_err() {
                return;
            }
            // Retries within one scan (too short, off centre) aren't done; a match or a miss is.
            let result = loop {
                let Some(status) = statuses.next().await else {
                    return;
                };
                match status.args() {
                    Ok(args) if args.done => break args.result.to_owned(),
                    Ok(_) => {
                        let _ = output.send(Finger::Again).await;
                    }
                    Err(_) => {}
                }
            };
            let _ = reader.verify_stop().await;
            said = match result.as_str() {
                "verify-match" => Finger::Matched,
                "verify-no-match" => {
                    misses += 1;
                    if misses < TRIES {
                        Finger::Again
                    } else {
                        Finger::Gone("Too many tries; type your password")
                    }
                }
                _ => Finger::Gone("The fingerprint reader stopped; type your password"),
            };
            if matches!(said, Finger::Matched | Finger::Gone(_)) {
                let _ = reader.release().await;
                let _ = output.send(said).await;
                return;
            }
        }
    })
}

// Only fprintd running as root is believed, not one a test bus in the environment brought along.
async fn trusted(connection: &Connection) -> bool {
    let Ok(bus) = DBusProxy::new(connection).await else {
        return false;
    };
    let name = BusName::from(WellKnownName::from_static_str_unchecked(FPRINT));
    match bus.get_name_owner(name).await {
        Ok(owner) => bus
            .get_connection_unix_user(owner.into())
            .await
            .is_ok_and(|uid| uid == 0),
        Err(_) => false,
    }
}

enum Claim<'a> {
    Ours(ReaderProxy<'a>),
    Busy,
    None,
}

// The default reader, if it knows a finger of yours and nothing else holds it.
async fn claim(connection: &Connection) -> Claim<'_> {
    let reader = async {
        let path = ManagerProxy::new(connection)
            .await
            .ok()?
            .get_default_device()
            .await
            .ok()?;
        ReaderProxy::builder(connection)
            .path(path)
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .ok()
    };
    let Some(reader) = reader.await else {
        return Claim::None;
    };
    // An empty name means whoever is asking.
    match reader.list_enrolled_fingers("").await {
        Ok(fingers) if !fingers.is_empty() => {}
        _ => return Claim::None,
    }
    match reader.claim("").await {
        Ok(()) => Claim::Ours(reader),
        Err(zbus::Error::MethodError(name, _, _)) if name.ends_with(".AlreadyInUse") => Claim::Busy,
        Err(_) => Claim::None,
    }
}

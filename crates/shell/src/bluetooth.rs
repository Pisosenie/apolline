use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use iced::Subscription;
use iced::futures::channel::{mpsc, oneshot};
use iced::futures::{SinkExt, Stream};
use zbus::fdo::{DBusProxy, ManagedObjects, ObjectManagerProxy};
use zbus::message::Header;
use zbus::names::{BusName, WellKnownName};
use zbus::proxy::CacheProperties;
use zbus::{Connection, ObjectPath, OwnedObjectPath, OwnedVariant};

use crate::controls::Radio;
use crate::shell::Message;

const BLUEZ: &str = "org.bluez";
const AGENT: &str = "/org/apolline/bluetooth";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub radio: Option<Radio>,
    pub devices: Vec<Device>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub kind: String,
    pub paired: bool,
    pub connected: bool,
    pub battery: Option<u8>,
    path: String,
}

// What the device being paired wants from you.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    id: u64,
    device: String,
    pub what: Asking,
}

impl Ask {
    pub fn about(&self, device: &Device) -> bool {
        self.device == device.path
    }

    pub fn is(&self, id: u64) -> bool {
        self.id == id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asking {
    // Both sides show this code; say whether they match.
    Confirm(String),
    // Type this code on the device.
    Type(String),
    // The device has no screen; it only asks to be let in.
    Allow,
    // An older device: type the PIN it came with.
    Pin,
    // Type the code the device shows.
    Passkey,
}

#[zbus::proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
trait Adapter {
    fn start_discovery(&self) -> zbus::Result<()>;
    fn remove_device(&self, device: &ObjectPath<'_>) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_powered(&self, on: bool) -> zbus::Result<()>;
}

#[zbus::proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
trait Gadget {
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
    fn pair(&self) -> zbus::Result<()>;
    fn cancel_pairing(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn adapter(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn set_trusted(&self, on: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.bluez.AgentManager1",
    default_service = "org.bluez",
    default_path = "/org/bluez"
)]
trait Agents {
    fn register_agent(&self, agent: &ObjectPath<'_>, capability: &str) -> zbus::Result<()>;
    fn request_default_agent(&self, agent: &ObjectPath<'_>) -> zbus::Result<()>;
}

// While the list is open it also looks for new devices and answers pairing; otherwise it only
// follows the radio and what's connected.
pub fn subscription(listing: bool) -> Subscription<Message> {
    Subscription::run_with(listing, |listing| watch(*listing))
}

fn watch(listing: bool) -> impl Stream<Item = Message> {
    iced::stream::channel(8, async move |mut output| {
        let mut builder = zbus::connection::Builder::system();
        if listing {
            builder = builder.serve_at(
                AGENT,
                Agent {
                    output: output.clone(),
                },
            );
        }
        let Ok(connection) = builder.build().await else {
            return;
        };
        let (Ok(objects), Some(mut changes)) = (
            objects(&connection).await,
            crate::bus::changes(&connection, BLUEZ).await,
        ) else {
            return;
        };
        let Ok(bus) = DBusProxy::new(&connection).await else {
            return;
        };
        let mut shown = None;
        let (mut owner, mut agent, mut looking) = (None, false, false);
        loop {
            let found = objects.get_managed_objects().await.ok();
            let seen = found
                .as_ref()
                .map(|found| seen(found, listing))
                .unwrap_or_default();
            // Each is tried once, not on every change: bluetoothd forgets both when it restarts,
            // and stops looking when the radio goes off.
            if listing {
                let now = bus.get_name_owner(bluez()).await.ok();
                if now != owner {
                    (owner, agent, looking) = (now, false, false);
                }
                if !agent && owner.is_some() {
                    agent = true;
                    if let Err(err) = serve(&connection).await {
                        eprintln!("apolline: no Bluetooth pairing agent: {err}");
                    }
                }
                let on = seen.radio.as_ref().is_some_and(|radio| radio.on);
                looking = looking && on;
                if !looking
                    && on
                    && let Some(adapter) = found.as_ref().and_then(adapter)
                {
                    looking = true;
                    if let Err(err) = look(&connection, adapter).await {
                        eprintln!("apolline: Bluetooth can't look for devices: {err}");
                    }
                }
            }
            if shown.as_ref() != Some(&seen) {
                if output.send(Message::Bluetooth(seen.clone())).await.is_err() {
                    return;
                }
                shown = Some(seen);
            }
            if changes.settled().await.is_none() {
                return;
            }
        }
    })
}

fn bluez() -> BusName<'static> {
    WellKnownName::from_static_str_unchecked(BLUEZ).into()
}

async fn objects(connection: &Connection) -> zbus::Result<ObjectManagerProxy<'_>> {
    ObjectManagerProxy::builder(connection)
        .destination(BLUEZ)
        .path("/")
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn serve(connection: &Connection) -> zbus::Result<()> {
    let agents = AgentsProxy::new(connection).await?;
    let path = ObjectPath::from_static_str_unchecked(AGENT);
    agents.register_agent(&path, "KeyboardDisplay").await?;
    agents.request_default_agent(&path).await
}

// Discovery belongs to this connection: BlueZ stops it when the list closes and the connection goes.
async fn look(connection: &Connection, adapter: &OwnedObjectPath) -> zbus::Result<()> {
    gadget::<AdapterProxy>(connection, adapter)
        .await?
        .start_discovery()
        .await
}

type Props = HashMap<String, OwnedVariant>;

fn adapter(objects: &ManagedObjects) -> Option<&OwnedObjectPath> {
    objects
        .iter()
        .filter(|(_, interfaces)| interfaces.contains_key("org.bluez.Adapter1"))
        .map(|(path, _)| path)
        .min_by_key(|path| path.as_str())
}

fn seen(objects: &ManagedObjects, listing: bool) -> Seen {
    let Some(adapter) = adapter(objects) else {
        return Seen::default();
    };
    let flag = |props: &Props, key: &str| {
        props
            .get(key)
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    };
    let word = |props: &Props, key: &str| {
        props
            .get(key)
            .and_then(|v| v.downcast_ref::<&str>().ok())
            .map(str::to_owned)
    };
    let on = flag(&objects[adapter]["org.bluez.Adapter1"], "Powered");
    let mut devices: Vec<Device> = objects
        .iter()
        .filter_map(|(path, interfaces)| {
            let props = interfaces.get("org.bluez.Device1")?;
            if props
                .get("Adapter")
                .and_then(|v| v.downcast_ref::<ObjectPath<'_>>().ok())
                .is_none_or(|owner| owner.as_str() != adapter.as_str())
            {
                return None;
            }
            let paired = flag(props, "Paired");
            // Unnamed ones nearby are beacons and other people's things, not something to pair with.
            word(props, "Name").or_else(|| paired.then(|| word(props, "Alias")).flatten())?;
            Some(Device {
                name: word(props, "Alias")?
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect(),
                kind: word(props, "Icon").unwrap_or_default(),
                paired,
                connected: flag(props, "Connected"),
                battery: interfaces
                    .get("org.bluez.Battery1")
                    .and_then(|battery| battery.get("Percentage"))
                    .and_then(|v| u8::try_from(v).ok()),
                path: path.to_string(),
            })
        })
        .collect();
    devices.sort_by(|a, b| {
        (!a.connected, !a.paired, a.name.to_lowercase()).cmp(&(
            !b.connected,
            !b.paired,
            b.name.to_lowercase(),
        ))
    });
    let connected = on
        .then(|| devices.iter().find(|d| d.connected))
        .flatten()
        .map(|d| d.name.clone());
    if !listing || !on {
        devices.clear();
    }
    Seen {
        radio: Some(Radio { on, connected }),
        devices,
    }
}

async fn gadget<'a, P>(connection: &Connection, path: &'a str) -> zbus::Result<P>
where
    P: From<zbus::Proxy<'a>> + zbus::proxy::Defaults,
{
    zbus::proxy::Builder::<P>::new(connection)
        .path(path)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

pub async fn power(on: bool) -> Result<(), String> {
    let connection = Connection::system().await.map_err(said)?;
    let found = objects(&connection)
        .await
        .map_err(said)?
        .get_managed_objects()
        .await
        .map_err(|err| said(err.into()))?;
    let adapter = adapter(&found).ok_or("There's no Bluetooth here")?;
    gadget::<AdapterProxy>(&connection, adapter)
        .await
        .map_err(said)?
        .set_powered(on)
        .await
        .map_err(said)
}

// The device you chose to pair: the agent answers for no other.
static PAIRING: Mutex<Option<String>> = Mutex::new(None);

struct Pairing;

impl Pairing {
    fn of(device: &Device) -> Self {
        if let Ok(mut pairing) = PAIRING.lock() {
            *pairing = Some(device.path.clone());
        }
        Pairing
    }
}

impl Drop for Pairing {
    fn drop(&mut self) {
        if let Ok(mut pairing) = PAIRING.lock() {
            *pairing = None;
        }
    }
}

// Connects or disconnects one you have, and pairs a new one first.
pub async fn toggle(device: Device) -> Result<(), String> {
    let connection = Connection::system().await.map_err(said)?;
    let gadget = gadget::<GadgetProxy>(&connection, &device.path)
        .await
        .map_err(said)?;
    let work = async {
        if device.connected {
            return gadget.disconnect().await;
        }
        if !device.paired {
            let pairing = Pairing::of(&device);
            gadget.pair().await?;
            drop(pairing);
            let _ = gadget.set_trusted(true).await;
            // A phone pairs but has nothing to connect to; paired is what was asked for.
            let _ = gadget.connect().await;
            return Ok(());
        }
        gadget.connect().await
    };
    tokio::time::timeout(Duration::from_secs(90), work)
        .await
        .map_err(|_| "It didn't answer in time".to_owned())?
        .map_err(said)
}

pub async fn forget(device: Device) -> Result<(), String> {
    let connection = Connection::system().await.map_err(said)?;
    let adapter = gadget::<GadgetProxy>(&connection, &device.path)
        .await
        .map_err(said)?
        .adapter()
        .await
        .map_err(said)?;
    gadget::<AdapterProxy>(&connection, &adapter)
        .await
        .map_err(said)?
        .remove_device(&ObjectPath::try_from(device.path.as_str()).map_err(said)?)
        .await
        .map_err(said)
}

fn said(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, text, _) => match name.as_str() {
            "org.bluez.Error.AuthenticationCanceled" | "org.bluez.Error.AuthenticationRejected" => {
                "Pairing was turned down".into()
            }
            "org.bluez.Error.AuthenticationFailed" => {
                "Pairing failed: the codes didn't match or it gave up".into()
            }
            "org.bluez.Error.AuthenticationTimeout" | "org.bluez.Error.ConnectionAttemptFailed" => {
                "It didn't answer. Put it in pairing mode and try again".into()
            }
            "org.bluez.Error.InProgress" => "Already busy with it".into(),
            _ if text.as_deref().is_some_and(|text| text.contains("rfkill")) => {
                "Bluetooth is blocked by a key or airplane mode".into()
            }
            _ if text.as_deref().is_some_and(|text| {
                ["page-timeout", "Page Timeout", "Host is down"]
                    .iter()
                    .any(|gone| text.contains(gone))
            }) =>
            {
                "It didn't answer. Is it on and nearby?".into()
            }
            _ => text.unwrap_or_else(|| name.to_string()),
        },
        err => err.to_string(),
    }
}

// One question at a time: BlueZ asks its agent one at a time.
static WAITING: Mutex<Option<(u64, oneshot::Sender<Option<String>>)>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn settle(id: Option<u64>, reply: Option<String>) {
    let Ok(mut waiting) = WAITING.lock() else {
        return;
    };
    if waiting
        .as_ref()
        .is_some_and(|(at, _)| id.is_none_or(|id| id == *at))
        && let Some((_, done)) = waiting.take()
    {
        let _ = done.send(reply);
    }
}

// The panel's answer: None turns it down. A code shown for typing has nothing waiting, so
// turning it down stops the pairing itself.
pub async fn reply(ask: Ask, reply: Option<String>) {
    if let (Asking::Type(_), None) = (&ask.what, &reply)
        && let Ok(connection) = Connection::system().await
        && let Ok(gadget) = gadget::<GadgetProxy>(&connection, &ask.device).await
    {
        let _ = gadget.cancel_pairing().await;
    }
    settle(Some(ask.id), reply);
}

// The list closed: whatever is still asked is turned down.
pub fn refuse() {
    settle(None, None);
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum Refused {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected,
}

struct Agent {
    output: mpsc::Sender<Message>,
}

async fn from_bluez(connection: &Connection, header: &Header<'_>) -> bool {
    let (Some(sender), Ok(bus)) = (header.sender(), DBusProxy::new(connection).await) else {
        return false;
    };
    bus.get_name_owner(bluez())
        .await
        .is_ok_and(|owner| owner.as_str() == sender.as_str())
}

impl Agent {
    // Only bluetoothd may ask, and only about the device you chose to pair.
    async fn allowed(
        connection: &Connection,
        header: &Header<'_>,
        device: &ObjectPath<'_>,
    ) -> bool {
        PAIRING
            .lock()
            .is_ok_and(|pairing| pairing.as_deref() == Some(device.as_str()))
            && from_bluez(connection, header).await
    }

    fn question(device: &ObjectPath<'_>, what: Asking) -> Ask {
        Ask {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            device: device.to_string(),
            what,
        }
    }

    async fn show(&self, device: &ObjectPath<'_>, what: Asking) {
        let ask = Self::question(device, what);
        let _ = self.output.clone().send(Message::BluetoothAsk(ask)).await;
    }

    async fn ask(&self, device: &ObjectPath<'_>, what: Asking) -> Result<String, Refused> {
        let ask = Self::question(device, what);
        let id = ask.id;
        let (done, wait) = oneshot::channel();
        if let Ok(mut waiting) = WAITING.lock() {
            *waiting = Some((id, done));
        }
        let _ = self.output.clone().send(Message::BluetoothAsk(ask)).await;
        let reply = wait.await.ok().flatten();
        let _ = self.output.clone().send(Message::BluetoothDone(id)).await;
        reply.ok_or(Refused::Rejected)
    }
}

// Calls nothing back on BlueZ while it waits for an answer; the panel names the device from its list.
#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn release(&self) {}

    async fn request_pin_code(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
    ) -> Result<String, Refused> {
        if !Self::allowed(connection, &header, &device).await {
            return Err(Refused::Rejected);
        }
        self.ask(&device, Asking::Pin).await
    }

    async fn display_pin_code(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
        pincode: String,
    ) -> Result<(), Refused> {
        if !Self::allowed(connection, &header, &device).await {
            return Err(Refused::Rejected);
        }
        self.show(&device, Asking::Type(pincode)).await;
        Ok(())
    }

    async fn request_passkey(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
    ) -> Result<u32, Refused> {
        if !Self::allowed(connection, &header, &device).await {
            return Err(Refused::Rejected);
        }
        let typed = self.ask(&device, Asking::Passkey).await?;
        typed.trim().parse().map_err(|_| Refused::Rejected)
    }

    async fn display_passkey(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
        passkey: u32,
        _entered: u16,
    ) {
        if Self::allowed(connection, &header, &device).await {
            self.show(&device, Asking::Type(format!("{passkey:06}")))
                .await;
        }
    }

    async fn request_confirmation(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
        passkey: u32,
    ) -> Result<(), Refused> {
        if !Self::allowed(connection, &header, &device).await {
            return Err(Refused::Rejected);
        }
        let code = format!("{passkey:06}");
        self.ask(&device, Asking::Confirm(code)).await.map(drop)
    }

    async fn request_authorization(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
    ) -> Result<(), Refused> {
        if !Self::allowed(connection, &header, &device).await {
            return Err(Refused::Rejected);
        }
        self.ask(&device, Asking::Allow).await.map(drop)
    }

    // The device being paired brings its services with it; nothing else is let in.
    async fn authorize_service(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        device: ObjectPath<'_>,
        _uuid: String,
    ) -> Result<(), Refused> {
        if Self::allowed(connection, &header, &device).await {
            Ok(())
        } else {
            Err(Refused::Rejected)
        }
    }

    async fn cancel(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) {
        if from_bluez(connection, &header).await {
            refuse();
        }
    }
}

pub fn sample() -> Vec<Device> {
    [
        ("WH-1000XM5", "audio-headset", true, true, Some(80)),
        ("MX Master 3S", "input-mouse", true, false, None),
        ("Pixel 8", "phone", false, false, None),
        ("Keychron K2", "input-keyboard", false, false, None),
    ]
    .into_iter()
    .map(|(name, kind, paired, connected, battery)| Device {
        name: name.into(),
        kind: kind.into(),
        paired,
        connected,
        battery,
        path: String::new(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant;
    use zbus::names::OwnedInterfaceName;

    fn object(
        interfaces: &[(&str, &[(&str, Variant<'static>)])],
    ) -> HashMap<OwnedInterfaceName, Props> {
        interfaces
            .iter()
            .map(|(name, props)| {
                (
                    OwnedInterfaceName::try_from(*name).unwrap(),
                    props
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), OwnedVariant::try_from(v.clone()).unwrap()))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn the_list_has_yours_first_and_skips_nameless_beacons() {
        let adapter = ObjectPath::from_static_str_unchecked("/org/bluez/hci0");
        let device = |name: Option<&'static str>, alias: &'static str, paired, connected| {
            let mut props = vec![
                ("Adapter", Variant::from(adapter.clone())),
                ("Alias", Variant::from(alias)),
                ("Paired", Variant::from(paired)),
                ("Connected", Variant::from(connected)),
            ];
            if let Some(name) = name {
                props.push(("Name", Variant::from(name)));
            }
            props
        };
        let headset = device(Some("WH-1000XM5"), "Headphones", true, true);
        let phone = device(Some("Pixel 8"), "Pixel 8", false, false);
        let beacon = device(None, "4C-11-AE-00-00-01", false, false);
        let mouse = device(None, "MX Master", true, false);
        let objects: ManagedObjects = [
            (
                "/org/bluez/hci0",
                object(&[("org.bluez.Adapter1", &[("Powered", Variant::from(true))])]),
            ),
            (
                "/org/bluez/hci0/dev_1",
                object(&[
                    ("org.bluez.Device1", &headset),
                    ("org.bluez.Battery1", &[("Percentage", Variant::from(80u8))]),
                ]),
            ),
            (
                "/org/bluez/hci0/dev_2",
                object(&[("org.bluez.Device1", &phone)]),
            ),
            (
                "/org/bluez/hci0/dev_3",
                object(&[("org.bluez.Device1", &beacon)]),
            ),
            (
                "/org/bluez/hci0/dev_4",
                object(&[("org.bluez.Device1", &mouse)]),
            ),
        ]
        .into_iter()
        .map(|(path, interfaces)| (OwnedObjectPath::try_from(path).unwrap(), interfaces))
        .collect();

        let listed = seen(&objects, true);
        let names: Vec<&str> = listed.devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Headphones", "MX Master", "Pixel 8"]);
        assert_eq!(listed.devices[0].battery, Some(80));
        assert_eq!(
            listed.radio,
            Some(Radio {
                on: true,
                connected: Some("Headphones".into())
            })
        );
        assert!(
            seen(&objects, false).devices.is_empty(),
            "the bar needs no list"
        );
        assert_eq!(seen(&ManagedObjects::new(), true), Seen::default());
    }
}

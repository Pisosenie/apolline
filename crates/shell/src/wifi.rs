use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt, future, stream};
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::proxy::CacheProperties;
use zbus::{Connection, ObjectPath, OwnedObjectPath, OwnedVariant, Variant};

use crate::controls::Radio;
use crate::shell::Message;

const NM: &str = "org.freedesktop.NetworkManager";
const POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
// NetworkManager turns down scans asked for more often than this.
const RESCAN: Duration = Duration::from_secs(20);

const PRIVACY: u32 = 0x1;
const PSK: u32 = 0x100;
const EAP: u32 = 0x200;
const SAE: u32 = 0x400;
const SUITE_B: u32 = 0x2000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub radio: Option<Radio>,
    pub networks: Vec<Network>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub lock: Lock,
    pub active: bool,
    // A saved profile fits how this network is secured.
    pub known: bool,
    // Saved under this name but secured differently, so it may be someone else's.
    pub stranger: bool,
    raw: Vec<u8>,
    flags: u32,
    keys: u32,
    point: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    Open,
    Password,
    Wep,
    Enterprise,
}

impl Network {
    pub fn secure(&self) -> bool {
        self.lock != Lock::Open
    }

    pub fn asks_password(&self) -> bool {
        matches!(self.lock, Lock::Password | Lock::Wep) && !self.known && !self.stranger
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    pub said: String,
    pub password: bool,
}

impl Failed {
    fn because(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            password: false,
        }
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait Manager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, Variant<'_>>>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wireless_enabled(&self, on: bool) -> zbus::Result<()>;
}

// Its own module: both it and the active connection have a StateChanged signal.
mod device {
    #[zbus::proxy(
        interface = "org.freedesktop.NetworkManager.Device",
        default_service = "org.freedesktop.NetworkManager"
    )]
    pub trait Device {
        #[zbus(property)]
        fn device_type(&self) -> zbus::Result<u32>;
        #[zbus(signal, name = "StateChanged")]
        fn moved(&self, new: u32, old: u32, reason: u32) -> zbus::Result<()>;
    }
}
use device::DeviceProxy;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wireless {
    fn request_scan(&self, options: HashMap<&str, Variant<'_>>) -> zbus::Result<()>;
    #[zbus(property)]
    fn access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait Settings {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

type Profile = HashMap<String, HashMap<String, OwnedVariant>>;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Saved {
    fn get_settings(&self) -> zbus::Result<Profile>;
    fn delete(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Active {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(signal, name = "StateChanged")]
    fn turned(&self, state: u32, reason: u32) -> zbus::Result<()>;
}

// While the list is open it also scans; otherwise it only follows the radio and the network in use.
pub fn subscription(listing: bool) -> Subscription<Message> {
    Subscription::run_with(listing, |listing| watch(*listing))
}

fn watch(listing: bool) -> impl Stream<Item = Message> {
    iced::stream::channel(4, async move |mut output| {
        let Ok(connection) = Connection::system().await else {
            return;
        };
        let Some(mut changes) = crate::bus::changes(&connection, NM).await else {
            return;
        };
        let mut shown = None;
        let mut scanned: Option<Instant> = None;
        loop {
            let seen = read(&connection, listing).await;
            if listing
                && seen.radio.as_ref().is_some_and(|radio| radio.on)
                && scanned.is_none_or(|at| at.elapsed() >= RESCAN)
            {
                scan(&connection).await;
                scanned = Some(Instant::now());
            }
            if shown.as_ref() != Some(&seen) {
                if output.send(Message::Wifi(seen.clone())).await.is_err() {
                    return;
                }
                shown = Some(seen);
            }
            // While listing, the next scan comes due even if nothing changes.
            let changed = changes.settled();
            let changed = if listing {
                tokio::time::timeout(RESCAN, changed)
                    .await
                    .unwrap_or(Some(()))
            } else {
                changed.await
            };
            if changed.is_none() {
                return;
            }
        }
    })
}

pub async fn connected() -> Option<String> {
    let connection = Connection::system().await.ok()?;
    read(&connection, false)
        .await
        .radio
        .filter(|radio| radio.on)?
        .connected
}

async fn read(connection: &Connection, listing: bool) -> Seen {
    let Some((manager, device)) = wireless(connection).await else {
        return Seen::default();
    };
    let Ok(wireless) = proxy(WirelessProxy::builder(connection), &device).await else {
        return Seen::default();
    };
    let on = manager.wireless_enabled().await.unwrap_or(false);
    let active = wireless
        .active_access_point()
        .await
        .ok()
        .filter(|path| on && path.as_str() != "/");
    let connected = match &active {
        Some(path) => point(connection, path).await.map(|p| p.ssid),
        None => None,
    };
    let networks = if listing && on {
        networks(connection, &wireless, active.as_ref()).await
    } else {
        Vec::new()
    };
    Seen {
        radio: Some(Radio { on, connected }),
        networks,
    }
}

async fn networks(
    connection: &Connection,
    wireless: &WirelessProxy<'_>,
    active: Option<&OwnedObjectPath>,
) -> Vec<Network> {
    let saved = saved(connection).await;
    let mut networks: Vec<Network> = Vec::new();
    for path in wireless.access_points().await.unwrap_or_default() {
        let Some(mut network) = point(connection, &path).await else {
            continue;
        };
        if network.raw.is_empty() {
            continue;
        }
        network.active = active == Some(&path);
        let mine = saved.iter().filter(|s| s.raw == network.raw);
        network.known = mine.clone().any(|s| network.fits(&s.security));
        network.stranger = !network.known && mine.count() > 0;
        // One row per network: the one in use, else one that fits what you saved, else the strongest.
        let rank = |n: &Network| (n.active, n.known, n.signal);
        match networks.iter_mut().find(|n| n.raw == network.raw) {
            Some(seen) if rank(seen) >= rank(&network) => {}
            Some(seen) => *seen = network,
            None => networks.push(network),
        }
    }
    networks.sort_by_key(|n| (!n.active, std::cmp::Reverse(n.signal)));
    networks
}

async fn point(connection: &Connection, path: &ObjectPath<'_>) -> Option<Network> {
    let props = proxy(PropertiesProxy::builder(connection), path)
        .await
        .ok()?
        .get_all(InterfaceName::from_static_str_unchecked(POINT))
        .await
        .ok()?;
    let number = |key: &str| props.get(key).and_then(|v| u32::try_from(v).ok());
    let raw = props
        .get("Ssid")?
        .try_clone()
        .ok()
        .and_then(|v| Vec::<u8>::try_from(v).ok())?;
    let flags = number("Flags").unwrap_or(0);
    let keys = number("WpaFlags").unwrap_or(0) | number("RsnFlags").unwrap_or(0);
    Some(Network {
        ssid: shown(&raw),
        signal: props
            .get("Strength")
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(0),
        lock: lock(flags, keys),
        active: false,
        known: false,
        stranger: false,
        raw,
        flags,
        keys,
        point: path.to_string(),
    })
}

// Network names are bytes; what can't be shown as text becomes a placeholder.
fn shown(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

// The privacy bit, then the key management NetworkManager reads from the beacon.
fn lock(flags: u32, keys: u32) -> Lock {
    if keys & (EAP | SUITE_B) != 0 {
        Lock::Enterprise
    } else if keys & (PSK | SAE) != 0 {
        Lock::Password
    } else if keys == 0 && flags & PRIVACY != 0 {
        Lock::Wep
    } else {
        Lock::Open
    }
}

impl Network {
    // Whether a profile saved with this key management can join this access point.
    fn fits(&self, security: &str) -> bool {
        match security {
            "wpa-psk" => self.keys & PSK != 0,
            "sae" => self.keys & SAE != 0,
            "wpa-eap" | "wpa-eap-suite-b-192" => self.keys & (EAP | SUITE_B) != 0,
            "none" | "ieee8021x" => lock(self.flags, self.keys) == Lock::Wep,
            _ => lock(self.flags, self.keys) == Lock::Open,
        }
    }
}

async fn wireless(connection: &Connection) -> Option<(ManagerProxy<'_>, OwnedObjectPath)> {
    let manager = ManagerProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()?;
    for path in manager.get_devices().await.ok()? {
        let Ok(device) = proxy(DeviceProxy::builder(connection), &path).await else {
            continue;
        };
        if device.device_type().await == Ok(2) {
            return Some((manager, path));
        }
    }
    None
}

struct Kept {
    raw: Vec<u8>,
    // Key management, empty for an open network.
    security: String,
    path: OwnedObjectPath,
}

async fn saved(connection: &Connection) -> Vec<Kept> {
    let Ok(settings) = SettingsProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
    else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for path in settings.list_connections().await.unwrap_or_default() {
        let Ok(profile) = proxy(SavedProxy::builder(connection), &path).await else {
            continue;
        };
        if let Some((raw, security)) = profile.get_settings().await.ok().as_ref().and_then(wifi) {
            found.push(Kept {
                raw,
                security,
                path,
            });
        }
    }
    found
}

fn wifi(profile: &Profile) -> Option<(Vec<u8>, String)> {
    let kind = profile.get("connection")?.get("type")?;
    if kind.downcast_ref::<&str>().ok()? != "802-11-wireless" {
        return None;
    }
    let raw = profile
        .get("802-11-wireless")?
        .get("ssid")?
        .try_clone()
        .ok()
        .and_then(|v| Vec::<u8>::try_from(v).ok())?;
    let security = profile
        .get("802-11-wireless-security")
        .and_then(|s| s.get("key-mgmt"))
        .and_then(|v| v.downcast_ref::<&str>().ok())
        .unwrap_or_default()
        .to_owned();
    Some((raw, security))
}

async fn proxy<'a, P>(
    builder: zbus::proxy::Builder<'a, P>,
    path: &ObjectPath<'_>,
) -> zbus::Result<P>
where
    P: From<zbus::Proxy<'a>> + zbus::proxy::Defaults,
{
    builder
        .destination(NM)
        .path(path.to_owned())
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn delete(connection: &Connection, path: &ObjectPath<'_>) -> zbus::Result<()> {
    proxy(SavedProxy::builder(connection), path)
        .await?
        .delete()
        .await
}

async fn scan(connection: &Connection) {
    if let Some((_, device)) = wireless(connection).await
        && let Ok(wireless) = proxy(WirelessProxy::builder(connection), &device).await
    {
        let _ = wireless.request_scan(HashMap::new()).await;
    }
}

pub async fn power(on: bool) -> Result<(), String> {
    let connection = Connection::system().await.map_err(said)?;
    ManagerProxy::new(&connection)
        .await
        .map_err(said)?
        .set_wireless_enabled(on)
        .await
        .map_err(said)
}

pub async fn join(network: Network, password: Option<String>) -> Result<(), Failed> {
    if network.stranger {
        return Err(Failed::because(
            "Secured differently from the network you saved under this name, so it may not be yours. Forget that one to join this.",
        ));
    }
    if network.lock == Lock::Enterprise && !network.known {
        return Err(Failed::because(
            "This network signs in with a username. Set it up once in nmtui; after that it's here.",
        ));
    }
    let connection = Connection::system().await.map_err(failed)?;
    let (manager, device) = wireless(&connection)
        .await
        .ok_or(Failed::because("There's no Wi-Fi here"))?;
    let point = ObjectPath::try_from(network.point.as_str()).map_err(failed)?;
    let fitting: Vec<OwnedObjectPath> = saved(&connection)
        .await
        .into_iter()
        .filter(|s| s.raw == network.raw && network.fits(&s.security))
        .map(|s| s.path)
        .collect();
    if let (None, Some(profile)) = (&password, fitting.first()) {
        let active = manager
            .activate_connection(profile, &device, &point)
            .await
            .map_err(failed)?;
        return settle(&connection, &device, active).await;
    }
    let mut security: HashMap<&str, Variant<'_>> = HashMap::new();
    match (network.lock, password.as_deref()) {
        (Lock::Password, Some(password)) => {
            security.insert("psk", password.into());
        }
        (Lock::Wep, Some(password)) => {
            // A key of hex digits or 5 or 13 letters is the key itself; anything else is a passphrase.
            let key = matches!(password.len(), 5 | 13)
                || (matches!(password.len(), 10 | 26)
                    && password.chars().all(|c| c.is_ascii_hexdigit()));
            security.insert("wep-key0", password.into());
            security.insert("wep-key-type", Variant::from(if key { 1u32 } else { 2 }));
        }
        _ => {}
    }
    let mut settings = HashMap::new();
    if !security.is_empty() {
        settings.insert("802-11-wireless-security", security);
    }
    let (profile, active) = manager
        .add_and_activate_connection(settings, &device, &point)
        .await
        .map_err(failed)?;
    let joined = settle(&connection, &device, active).await;
    // The new password replaces the old one only once it works; a wrong one isn't kept.
    let stale = if joined.is_ok() {
        fitting
    } else {
        vec![profile]
    };
    for path in stale {
        let _ = delete(&connection, &path).await;
    }
    joined
}

pub async fn forget(network: Network) -> Result<(), String> {
    let connection = Connection::system().await.map_err(said)?;
    for kept in saved(&connection).await {
        if kept.raw == network.raw {
            delete(&connection, &kept.path).await.map_err(said)?;
        }
    }
    Ok(())
}

enum Turn {
    Active(u32),
    Device(u32, u32),
}

// Waits until NetworkManager says the network is up, or why it isn't. The reason comes with the
// device's failure; the active connection only says it went down.
async fn settle(
    connection: &Connection,
    device: &OwnedObjectPath,
    active: OwnedObjectPath,
) -> Result<(), Failed> {
    let device = proxy(DeviceProxy::builder(connection), device)
        .await
        .map_err(failed)?;
    let active = proxy(ActiveProxy::builder(connection), &active)
        .await
        .map_err(failed)?;
    let moves = device
        .receive_moved()
        .await
        .map_err(failed)?
        .filter_map(|s| future::ready(s.args().ok().map(|a| Turn::Device(a.new, a.reason))));
    let turns = active
        .receive_turned()
        .await
        .map_err(failed)?
        .filter_map(|s| future::ready(s.args().ok().map(|a| Turn::Active(a.state))));
    let mut turns = stream::select(moves, turns);
    let wait = async {
        match active.state().await {
            Ok(2) => return Ok(()),
            Ok(4) | Err(_) => return Err(why(0)),
            Ok(_) => {}
        }
        let mut down = false;
        loop {
            let turn = if down {
                tokio::time::timeout(Duration::from_millis(500), turns.next())
                    .await
                    .ok()
                    .flatten()
            } else {
                turns.next().await
            };
            match turn {
                Some(Turn::Active(2)) if !down => return Ok(()),
                Some(Turn::Device(120, reason)) => return Err(why(reason)),
                Some(Turn::Active(4)) => down = true,
                Some(_) => {}
                None => return Err(why(0)),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(60), wait)
        .await
        .unwrap_or_else(|_| Err(Failed::because("It took too long to join")))
}

// NetworkManager's reasons for a device to fail.
fn why(reason: u32) -> Failed {
    let (said, password) = match reason {
        7 | 8 => ("The password didn't fit", true),
        11 | 53 => ("The network didn't answer. Is it in range?", false),
        5 | 6 => ("Joined, but the network gave no address", false),
        _ => ("Couldn't join this network", false),
    };
    Failed {
        said: said.into(),
        password,
    }
}

fn said(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(text), _) => text,
        err => err.to_string(),
    }
}

fn failed(err: zbus::Error) -> Failed {
    Failed::because(said(err))
}

pub fn sample() -> Vec<Network> {
    [
        ("Polina", 82, PSK, true, true),
        ("Neighbours", 64, PSK, false, false),
        ("Office", 52, EAP, false, false),
        ("Cafe: Free", 41, 0, false, false),
    ]
    .into_iter()
    .map(|(ssid, signal, keys, active, known)| Network {
        ssid: ssid.into(),
        signal,
        lock: lock(PRIVACY, keys),
        active,
        known,
        stranger: false,
        raw: ssid.as_bytes().to_vec(),
        flags: if keys == 0 { 0 } else { PRIVACY },
        keys,
        point: String::new(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_flags_say_what_a_network_asks_for() {
        assert_eq!(lock(0, 0), Lock::Open);
        assert_eq!(lock(1, 0), Lock::Wep);
        assert_eq!(lock(1, 0x188), Lock::Password, "WPA2 personal");
        assert_eq!(lock(1, 0x400 | 0x100), Lock::Password, "WPA3 transition");
        assert_eq!(lock(1, 0x400), Lock::Password, "WPA3 only");
        assert_eq!(lock(1, 0x288), Lock::Enterprise);
        assert_eq!(
            lock(0, 0x800),
            Lock::Open,
            "enhanced open needs no password"
        );
    }

    #[test]
    fn a_saved_profile_fits_only_the_security_it_was_made_for() {
        let mut network = sample().remove(0);
        assert!(network.fits("wpa-psk"));
        assert!(!network.fits("sae"), "WPA3 isn't offered");
        assert!(
            !network.fits("wpa-eap"),
            "a twin can't take an office login"
        );
        assert!(!network.fits(""), "nor pass for an open network");
        network.keys = PSK | SAE;
        assert!(network.fits("sae") && network.fits("wpa-psk"));
        network.keys = SAE;
        assert!(!network.fits("wpa-psk"), "no downgrade from WPA3");
    }

    #[test]
    fn only_wifi_profiles_have_a_network_name() {
        let entry = |pairs: &[(&str, Variant<'static>)]| -> HashMap<String, OwnedVariant> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), OwnedVariant::try_from(v.clone()).unwrap()))
                .collect()
        };
        let wifi_profile: Profile = HashMap::from([
            (
                "connection".to_owned(),
                entry(&[("type", Variant::from("802-11-wireless"))]),
            ),
            (
                "802-11-wireless".to_owned(),
                entry(&[("ssid", Variant::from(b"Polina".to_vec()))]),
            ),
            (
                "802-11-wireless-security".to_owned(),
                entry(&[("key-mgmt", Variant::from("sae"))]),
            ),
        ]);
        assert_eq!(
            wifi(&wifi_profile),
            Some((b"Polina".to_vec(), "sae".to_owned()))
        );
        let wired: Profile = HashMap::from([(
            "connection".to_owned(),
            entry(&[("type", Variant::from("802-3-ethernet"))]),
        )]);
        assert_eq!(wifi(&wired), None);
        assert_eq!(shown(b"Caf\xe9\n"), "Caf\u{fffd}", "no control characters");
    }
}

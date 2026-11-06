use std::collections::HashMap;

use iced::futures::SinkExt;
use zbus::proxy::CacheProperties;
use zbus::{Connection, ObjectPath, OwnedObjectPath, OwnedVariant};

// VPNs and WireGuard tunnels saved in NetworkManager, switched on and off from quick settings.
const NM: &str = "org.freedesktop.NetworkManager";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tunnel {
    pub name: String,
    pub uuid: String,
    // Where NetworkManager keeps it, to switch it on.
    pub path: String,
    // While it's on, its active connection, to switch it off.
    pub active: Option<String>,
    // Coming up, not up yet.
    pub busy: bool,
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait Manager {
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn deactivate_connection(&self, active: &ObjectPath<'_>) -> zbus::Result<()>;
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait Settings {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Saved {
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedVariant>>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Active {
    #[zbus(property)]
    fn uuid(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

pub fn subscription() -> iced::Subscription<Vec<Tunnel>> {
    iced::Subscription::run(watch)
}

fn watch() -> impl iced::futures::Stream<Item = Vec<Tunnel>> {
    iced::stream::channel(1, async |mut output| {
        let Some(connection) = crate::bus::system().await else {
            return;
        };
        let Some(mut changes) = crate::bus::heard(&connection, rules()).await else {
            return;
        };
        let mut shown: Option<Vec<Tunnel>> = None;
        loop {
            let now = read(&connection).await;
            if shown.as_ref() != Some(&now) {
                shown = Some(now.clone());
                if output.send(now).await.is_err() {
                    return;
                }
            }
            if changes.settled().await.is_none() {
                return;
            }
        }
    })
}

// Saved connections changing, and connections going up and down: not the stream of strengths
// and bitrates the rest of NetworkManager sends.
fn rules() -> [zbus::match_rule::Builder<'static>; 4] {
    [
        crate::bus::signals(NM, "org.freedesktop.NetworkManager.Settings"),
        crate::bus::signals(NM, "org.freedesktop.NetworkManager.Settings.Connection"),
        crate::bus::properties(NM, NM),
        crate::bus::signals(NM, "org.freedesktop.NetworkManager.Connection.Active")
            .member("StateChanged"),
    ]
}

async fn read(connection: &Connection) -> Vec<Tunnel> {
    let Ok(settings) = SettingsProxy::new(connection).await else {
        return Vec::new();
    };
    let mut tunnels = Vec::new();
    for path in settings.list_connections().await.unwrap_or_default() {
        let Ok(saved) = proxy(SavedProxy::builder(connection), &path).await else {
            continue;
        };
        let Ok(profile) = saved.get_settings().await else {
            continue;
        };
        if let Some((name, uuid)) = tunnel(&profile) {
            tunnels.push(Tunnel {
                name,
                uuid,
                path: path.to_string(),
                active: None,
                busy: false,
            });
        }
    }
    let manager = ManagerProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await;
    if let Ok(manager) = manager {
        for active in manager.active_connections().await.unwrap_or_default() {
            let Ok(proxy) = proxy(ActiveProxy::builder(connection), &active).await else {
                continue;
            };
            let (Ok(uuid), Ok(state)) = (proxy.uuid().await, proxy.state().await) else {
                continue;
            };
            if let Some(tunnel) = tunnels.iter_mut().find(|t| t.uuid == uuid) {
                // 1 activating, 2 activated.
                tunnel.busy = state == 1;
                tunnel.active = matches!(state, 1 | 2).then(|| active.to_string());
            }
        }
    }
    tunnels.sort_by_key(|t| t.name.to_lowercase());
    tunnels
}

// A saved connection that's a VPN or a WireGuard tunnel: its name and uuid.
fn tunnel(profile: &HashMap<String, HashMap<String, OwnedVariant>>) -> Option<(String, String)> {
    let section = profile.get("connection")?;
    let text = |key: &str| {
        section
            .get(key)
            .and_then(|v| String::try_from(v.clone()).ok())
    };
    let kind = text("type")?;
    if !matches!(kind.as_str(), "vpn" | "wireguard") {
        return None;
    }
    Some((text("id")?, text("uuid")?))
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

pub async fn switch(tunnel: Tunnel, on: bool) -> Result<(), String> {
    let connection = crate::bus::system().await.ok_or("no system bus")?;
    let manager = ManagerProxy::new(&connection)
        .await
        .map_err(|e| e.to_string())?;
    let root = ObjectPath::from_static_str_unchecked("/");
    let result = match (on, &tunnel.active) {
        (true, _) => {
            let path = ObjectPath::try_from(tunnel.path.as_str()).map_err(|e| e.to_string())?;
            manager
                .activate_connection(&path, &root, &root)
                .await
                .map(|_| ())
        }
        (false, Some(active)) => {
            let path = ObjectPath::try_from(active.as_str()).map_err(|e| e.to_string())?;
            manager.deactivate_connection(&path).await
        }
        (false, None) => Ok(()),
    };
    result.map_err(|err| match err {
        zbus::Error::MethodError(_, Some(text), _) => text,
        err => err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(kind: &str) -> HashMap<String, HashMap<String, OwnedVariant>> {
        let value = |s: &str| OwnedVariant::try_from(zbus::Variant::from(s)).unwrap();
        let section: HashMap<String, OwnedVariant> = [
            ("type".to_owned(), value(kind)),
            ("id".to_owned(), value("Office")),
            ("uuid".to_owned(), value("1234")),
        ]
        .into_iter()
        .collect();
        [("connection".to_owned(), section)].into_iter().collect()
    }

    #[test]
    fn only_vpns_and_wireguard_count() {
        assert_eq!(
            tunnel(&profile("vpn")),
            Some(("Office".into(), "1234".into()))
        );
        assert_eq!(
            tunnel(&profile("wireguard")),
            Some(("Office".into(), "1234".into()))
        );
        assert_eq!(tunnel(&profile("802-11-wireless")), None);
    }
}

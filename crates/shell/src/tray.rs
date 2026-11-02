use std::collections::HashMap;
use std::path::PathBuf;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, StreamExt};
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};
use zbus::message::Type;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, MatchRule, MessageStream, connection};
use zbus::{OwnedObjectPath, OwnedVariant as OwnedValue, Variant as Value};

// The icons apps put in the tray: Steam, Discord, Telegram, VPN clients and the like, over
// StatusNotifierItem. The shell is the watcher apps register with, unless another one already is,
// and the host that shows them.
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const DEFAULT_ITEM_PATH: &str = "/StatusNotifierItem";

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    // Where it lives: a bus name and an object path.
    pub service: String,
    pub path: String,
    pub title: String,
    pub icon: Icon,
    pub attention: bool,
    // Some apps only offer a menu, no action of their own.
    pub menu_only: bool,
    pub menu: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    // A named icon, and the app's own folder to look for it in.
    Named(String, Option<PathBuf>),
    // Pixels, ARGB as the protocol sends them, turned into RGBA.
    Pixels(u32, u32, Vec<u8>),
    None,
}

#[derive(Clone, Debug)]
pub enum Event {
    Items(Vec<Item>),
}

// What a click on an icon asks of its app.
#[derive(Clone, Debug, PartialEq)]
pub enum Press {
    Activate,
    Secondary,
    // A menu entry, by its id in the app's DBusMenu.
    Menu(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub toggled: Option<bool>,
}

pub fn subscription() -> iced::Subscription<Event> {
    iced::Subscription::run(host)
}

struct Watcher {
    items: Vec<String>,
    hosts: Vec<String>,
    changed: mpsc::Sender<()>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &mut self,
        service: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        // Apps send a bus name, or only an object path on their own connection.
        let sender = header.sender().map(|s| s.to_string()).unwrap_or_default();
        let full = if service.starts_with('/') {
            format!("{sender}{service}")
        } else if service.contains('/') {
            service.to_owned()
        } else {
            format!("{service}{DEFAULT_ITEM_PATH}")
        };
        if !self.items.contains(&full) {
            self.items.push(full.clone());
            let _ = Self::status_notifier_item_registered(&emitter, &full).await;
            let _ = self
                .registered_status_notifier_items_changed(&emitter)
                .await;
            let _ = self.changed.try_send(());
        }
    }

    fn register_status_notifier_host(&mut self, service: &str) {
        if !self.hosts.iter().any(|h| h == service) {
            self.hosts.push(service.to_owned());
        }
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;
}

fn host() -> impl iced::futures::Stream<Item = Event> {
    iced::stream::channel(8, async |mut output| {
        let (changed, mut heard) = mpsc::channel::<()>(1);
        let watcher = Watcher {
            items: Vec::new(),
            hosts: Vec::new(),
            changed: changed.clone(),
        };
        let Ok(connection) = connection::Builder::session()
            .serve_at(WATCHER_PATH, watcher)
            .build()
            .await
        else {
            return;
        };
        let flags = RequestNameFlags::DO_NOT_QUEUE;
        let ours = matches!(
            connection.request_name_with_flags(WATCHER, flags).await,
            Ok(RequestNameReply::PrimaryOwner)
        );
        let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
        let _ = connection.request_name(host.as_str()).await;
        if !ours {
            let _ = register_host(&connection, &host).await;
        }
        // Items come and go with their apps, and change their icons and titles.
        let watch = watch_signals(connection.clone(), changed.clone());
        tokio::spawn(watch);
        let mut shown: Option<Vec<Item>> = None;
        loop {
            let names = if ours {
                ours_items(&connection, changed.clone()).await
            } else {
                their_items(&connection).await
            };
            let items = read_all(&connection, &names).await;
            if shown.as_ref() != Some(&items) {
                shown = Some(items.clone());
                if output.send(Event::Items(items)).await.is_err() {
                    return;
                }
            }
            if heard.next().await.is_none() {
                return;
            }
            // The rest of the burst: an app usually changes several things at once.
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            while heard.try_recv().is_ok() {}
        }
    })
}

async fn register_host(connection: &Connection, host: &str) -> zbus::Result<()> {
    connection
        .call_method(
            Some(WATCHER),
            WATCHER_PATH,
            Some(WATCHER),
            "RegisterStatusNotifierHost",
            &(host,),
        )
        .await
        .map(|_| ())
}

// Our own watcher's list, with the ones whose apps went away dropped.
async fn ours_items(connection: &Connection, changed: mpsc::Sender<()>) -> Vec<String> {
    let server = connection.object_server();
    let Ok(watcher) = server.interface::<_, Watcher>(WATCHER_PATH).await else {
        return Vec::new();
    };
    let names = watcher.get().await.items.clone();
    let mut alive = Vec::new();
    let Ok(bus) = DBusProxy::new(connection).await else {
        return names;
    };
    for name in names {
        let (service, _) = split(&name);
        let has = match zbus::names::BusName::try_from(service) {
            Ok(bus_name) => bus.name_has_owner(bus_name).await.unwrap_or(false),
            Err(_) => false,
        };
        if has {
            alive.push(name);
        } else {
            let mut guard = watcher.get_mut().await;
            guard.items.retain(|n| *n != name);
            let emitter = watcher.signal_emitter();
            let _ = Watcher::status_notifier_item_unregistered(emitter, &name).await;
            let _ = guard
                .registered_status_notifier_items_changed(emitter)
                .await;
            drop(guard);
            let _ = changed.clone().try_send(());
        }
    }
    alive
}

async fn their_items(connection: &Connection) -> Vec<String> {
    let reply = connection
        .call_method(
            Some(WATCHER),
            WATCHER_PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(WATCHER, "RegisteredStatusNotifierItems"),
        )
        .await;
    reply
        .ok()
        .and_then(|reply| reply.body().deserialize::<OwnedValue>().ok())
        .and_then(|value| Vec::<String>::try_from(value).ok())
        .unwrap_or_default()
}

async fn watch_signals(connection: Connection, mut changed: mpsc::Sender<()>) {
    let rules = [
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(ITEM)
            .build(),
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(WATCHER)
            .build(),
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.freedesktop.DBus")
            .member("NameOwnerChanged")
            .build(),
    ];
    let mut streams = Vec::new();
    for rule in rules.into_iter().flatten() {
        if let Ok(stream) = MessageStream::for_match_rule(rule, &connection, None).await {
            streams.push(stream);
        }
    }
    let mut all = iced::futures::stream::select_all(streams);
    while all.next().await.is_some() {
        if changed.try_send(()).is_err_and(|err| err.is_disconnected()) {
            return;
        }
    }
}

fn split(name: &str) -> (&str, &str) {
    match name.find('/') {
        Some(at) => (&name[..at], &name[at..]),
        None => (name, DEFAULT_ITEM_PATH),
    }
}

async fn read_all(connection: &Connection, names: &[String]) -> Vec<Item> {
    let mut items = Vec::new();
    for name in names {
        let (service, path) = split(name);
        if let Some(item) = read(connection, service, path).await {
            items.push(item);
        }
    }
    items
}

async fn read(connection: &Connection, service: &str, path: &str) -> Option<Item> {
    let reply = connection
        .call_method(
            Some(service),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &(ITEM,),
        )
        .await
        .ok()?;
    let props: HashMap<String, OwnedValue> = reply.body().deserialize().ok()?;
    let text = |key: &str| -> Option<String> {
        props
            .get(key)
            .and_then(|v| String::try_from(v.clone()).ok())
            .filter(|s| !s.is_empty())
    };
    let status = text("Status").unwrap_or_default();
    if status == "Passive" {
        return None;
    }
    let attention = status == "NeedsAttention";
    let theme_path = text("IconThemePath").map(PathBuf::from);
    let named = if attention {
        text("AttentionIconName").or_else(|| text("IconName"))
    } else {
        text("IconName")
    };
    let pixels = || {
        let key = if attention {
            "AttentionIconPixmap"
        } else {
            "IconPixmap"
        };
        props.get(key).and_then(|v| pixmap(v.clone()))
    };
    let icon = match (named, pixels()) {
        (Some(name), _) if name.starts_with('/') => Icon::Named(name, None),
        (Some(name), Some(pixels))
            if crate::apps::icon_named(&name, None, theme_path.as_deref()).is_none() =>
        {
            pixels
        }
        (Some(name), _) => Icon::Named(name, theme_path),
        (None, Some(pixels)) => pixels,
        (None, None) => Icon::None,
    };
    let tooltip = props
        .get("ToolTip")
        .and_then(|v| {
            <(String, Vec<(i32, i32, Vec<u8>)>, String, String)>::try_from(v.clone()).ok()
        })
        .map(|(_, _, title, _)| title)
        .filter(|t| !t.is_empty());
    let title = tooltip
        .or_else(|| text("Title"))
        .or_else(|| text("Id"))
        .unwrap_or_else(|| service.to_owned());
    let menu = props
        .get("Menu")
        .and_then(|v| OwnedObjectPath::try_from(v.clone()).ok())
        .map(|p| p.to_string())
        .filter(|p| p != "/");
    let menu_only = props
        .get("ItemIsMenu")
        .and_then(|v| bool::try_from(v.clone()).ok())
        .unwrap_or(false);
    Some(Item {
        service: service.to_owned(),
        path: path.to_owned(),
        title,
        icon,
        attention,
        menu_only,
        menu,
    })
}

// The biggest picture of the set, ARGB in network order, as RGBA.
fn pixmap(value: OwnedValue) -> Option<Icon> {
    let sizes = Vec::<(i32, i32, Vec<u8>)>::try_from(value).ok()?;
    let (w, h, data) = sizes
        .into_iter()
        .filter(|(w, h, data)| *w > 0 && *h > 0 && data.len() >= (*w * *h * 4) as usize)
        .max_by_key(|(w, ..)| *w)?;
    Some(Icon::Pixels(w as u32, h as u32, argb_to_rgba(&data)))
}

fn argb_to_rgba(data: &[u8]) -> Vec<u8> {
    data.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[a, r, g, b]| [r, g, b, a])
        .collect()
}

pub async fn press(item: Item, press: Press, at: (i32, i32)) {
    let Ok(connection) = Connection::session().await else {
        return;
    };
    let result = match press {
        Press::Activate | Press::Secondary => {
            let method = if press == Press::Activate {
                "Activate"
            } else {
                "SecondaryActivate"
            };
            connection
                .call_method(
                    Some(item.service.as_str()),
                    item.path.as_str(),
                    Some(ITEM),
                    method,
                    &at,
                )
                .await
                .map(|_| ())
        }
        Press::Menu(id) => match &item.menu {
            Some(menu) => {
                let data = Value::from(0i32);
                let stamp = 0u32;
                connection
                    .call_method(
                        Some(item.service.as_str()),
                        menu.as_str(),
                        Some("com.canonical.dbusmenu"),
                        "Event",
                        &(id, "clicked", data, stamp),
                    )
                    .await
                    .map(|_| ())
            }
            None => Ok(()),
        },
    };
    if let Err(err) = result {
        eprintln!("apolline: tray: {}: {err}", item.title);
    }
}

// The top of an app's menu, as DBusMenu describes it.
pub async fn menu(item: Item) -> Vec<MenuEntry> {
    let Some(path) = item.menu.clone() else {
        return Vec::new();
    };
    let Ok(connection) = Connection::session().await else {
        return Vec::new();
    };
    let _ = connection
        .call_method(
            Some(item.service.as_str()),
            path.as_str(),
            Some("com.canonical.dbusmenu"),
            "AboutToShow",
            &(0i32,),
        )
        .await;
    let names: Vec<&str> = vec![
        "type",
        "label",
        "enabled",
        "visible",
        "toggle-type",
        "toggle-state",
    ];
    let reply = connection
        .call_method(
            Some(item.service.as_str()),
            path.as_str(),
            Some("com.canonical.dbusmenu"),
            "GetLayout",
            &(0i32, 1i32, names),
        )
        .await;
    let Ok(reply) = reply else {
        return Vec::new();
    };
    type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let Ok((_, (_, _, children))) = reply.body().deserialize::<(u32, Layout)>() else {
        return Vec::new();
    };
    children
        .into_iter()
        .filter_map(|child| Layout::try_from(child).ok())
        .filter_map(|(id, props, _)| entry(id, &props))
        .collect()
}

fn entry(id: i32, props: &HashMap<String, OwnedValue>) -> Option<MenuEntry> {
    let get_bool = |key: &str, default: bool| {
        props
            .get(key)
            .and_then(|v| bool::try_from(v.clone()).ok())
            .unwrap_or(default)
    };
    if !get_bool("visible", true) {
        return None;
    }
    let kind = props
        .get("type")
        .and_then(|v| String::try_from(v.clone()).ok())
        .unwrap_or_default();
    let label = props
        .get("label")
        .and_then(|v| String::try_from(v.clone()).ok())
        .unwrap_or_default()
        // Mnemonics: a lone underscore marks the access key, a doubled one is a real underscore.
        .replace("__", "\u{0}")
        .replace('_', "")
        .replace('\u{0}', "_");
    let toggled = props
        .get("toggle-type")
        .and_then(|v| String::try_from(v.clone()).ok())
        .filter(|t| !t.is_empty())
        .map(|_| {
            props
                .get("toggle-state")
                .and_then(|v| i32::try_from(v.clone()).ok())
                == Some(1)
        });
    Some(MenuEntry {
        id,
        label,
        enabled: get_bool("enabled", true),
        separator: kind == "separator",
        toggled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_their_object_path_or_get_the_usual_one() {
        assert_eq!(
            split(":1.42/org/ayatana/NotificationItem/steam"),
            (":1.42", "/org/ayatana/NotificationItem/steam")
        );
        assert_eq!(
            split("org.kde.StatusNotifierItem-1234-1"),
            ("org.kde.StatusNotifierItem-1234-1", "/StatusNotifierItem")
        );
    }

    #[test]
    fn pixels_come_as_argb_and_go_out_as_rgba() {
        assert_eq!(argb_to_rgba(&[255, 10, 20, 30]), [10, 20, 30, 255]);
        let sizes: Vec<(i32, i32, Vec<u8>)> = vec![
            (1, 1, vec![255, 1, 2, 3]),
            (2, 1, vec![128, 4, 5, 6, 255, 7, 8, 9]),
        ];
        let value = OwnedValue::try_from(Value::from(sizes)).unwrap();
        assert_eq!(
            pixmap(value),
            Some(Icon::Pixels(2, 1, vec![4, 5, 6, 128, 7, 8, 9, 255])),
            "the biggest one"
        );
    }

    #[test]
    fn menu_labels_lose_their_access_keys() {
        let props: HashMap<String, OwnedValue> = [
            (
                "label".to_owned(),
                OwnedValue::try_from(Value::from("_Quit Steam")).unwrap(),
            ),
            (
                "toggle-type".to_owned(),
                OwnedValue::try_from(Value::from("checkmark")).unwrap(),
            ),
            (
                "toggle-state".to_owned(),
                OwnedValue::try_from(Value::from(1i32)).unwrap(),
            ),
        ]
        .into_iter()
        .collect();
        let entry = entry(7, &props).unwrap();
        assert_eq!(entry.label, "Quit Steam");
        assert_eq!(entry.toggled, Some(true));
        let hidden: HashMap<String, OwnedValue> = [(
            "visible".to_owned(),
            OwnedValue::try_from(Value::from(false)).unwrap(),
        )]
        .into_iter()
        .collect();
        assert_eq!(super::entry(8, &hidden), None);
    }
}

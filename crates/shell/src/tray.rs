use std::collections::HashMap;
use std::path::PathBuf;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, StreamExt};
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};
use zbus::message::Type;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, MatchRule, MessageStream, connection};
use zbus::{OwnedObjectPath, OwnedVariant as OwnedValue, Variant as Value};

use iced::widget::{image, svg};

use crate::apps::Picture;

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
    // The icon ready to draw, found once by the shell rather than on every frame.
    pub glyph: Option<Glyph>,
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

#[derive(Clone, Debug, PartialEq)]
pub enum Glyph {
    Raster(image::Handle),
    Vector(svg::Handle),
}

// Pixels are kept no bigger than this: a 16 point icon on a screen at twice the scale.
const PIXELS: u32 = 32;

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
    changed: mpsc::Sender<Heard>,
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
            let _ = self.changed.try_send(Heard::List);
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
        let (tell, mut heard) = mpsc::channel::<Heard>(16);
        let watcher = Watcher {
            items: Vec::new(),
            hosts: Vec::new(),
            changed: tell.clone(),
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
        tokio::spawn(watch_signals(connection.clone(), tell.clone()));
        let mut tray = Tray::default();
        let mut news = vec![Heard::List];
        let mut shown: Option<Vec<Item>> = None;
        loop {
            tray.hear(&connection, ours, &news, &tell).await;
            let items = tray.items();
            if shown.as_ref() != Some(&items) {
                shown = Some(items.clone());
                if output.send(Event::Items(items)).await.is_err() {
                    return;
                }
            }
            let Some(first) = heard.next().await else {
                return;
            };
            // The rest of the burst: an app usually changes several things at once.
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            news = vec![first];
            while let Ok(more) = heard.try_recv() {
                if !news.contains(&more) {
                    news.push(more);
                }
            }
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Heard {
    // Items registered or went away.
    List,
    // One item changed something of its own: its sender and object path.
    Item(String, String),
    // An app left the bus; if it's back already, its items are read anew.
    Gone(String),
}

// The items as last read, in the order they registered.
#[derive(Default)]
struct Tray {
    names: Vec<String>,
    read: HashMap<String, Kept>,
}

struct Kept {
    // The app's own connection, which its signals come from.
    owner: Option<String>,
    // None while it asks not to be shown.
    item: Option<Item>,
    gone: tokio::task::JoinHandle<()>,
}

impl Drop for Kept {
    fn drop(&mut self) {
        self.gone.abort();
    }
}

impl Tray {
    async fn hear(
        &mut self,
        connection: &Connection,
        ours: bool,
        news: &[Heard],
        tell: &mpsc::Sender<Heard>,
    ) {
        for news in news {
            if let Heard::Gone(service) = news {
                self.read.retain(|name, _| split(name).0 != service);
            }
        }
        if news.iter().any(|news| !matches!(news, Heard::Item(..))) {
            let names = if ours {
                ours_items(connection).await
            } else {
                their_items(connection).await
            };
            self.read.retain(|name, _| names.contains(name));
            for name in &names {
                if !self.read.contains_key(name) {
                    let kept = Kept::read(connection, name, tell.clone()).await;
                    self.read.insert(name.clone(), kept);
                }
            }
            self.names = names;
        }
        // Only the item that changed is read again.
        for news in news {
            let Heard::Item(sender, path) = news else {
                continue;
            };
            for (name, kept) in &mut self.read {
                let (service, at) = split(name);
                if at == path && (service == sender || kept.owner.as_ref() == Some(sender)) {
                    kept.item = read(connection, service, at).await;
                }
            }
        }
    }

    fn items(&self) -> Vec<Item> {
        self.names
            .iter()
            .filter_map(|name| self.read.get(name)?.item.clone())
            .collect()
    }
}

impl Kept {
    async fn read(connection: &Connection, name: &str, tell: mpsc::Sender<Heard>) -> Self {
        let (service, path) = split(name);
        let owner = if service.starts_with(':') {
            Some(service.to_owned())
        } else {
            match (DBusProxy::new(connection).await, BusName::try_from(service)) {
                (Ok(bus), Ok(service)) => bus.get_name_owner(service).await.ok(),
                _ => None,
            }
            .map(|owner| owner.to_string())
        };
        Self {
            owner,
            item: read(connection, service, path).await,
            gone: tokio::spawn(gone(connection.clone(), service.to_owned(), tell)),
        }
    }
}

// Waits for an item's app to leave the bus. Only its own name is listened for: every other
// program that comes and goes on the session bus is none of the tray's business.
async fn gone(connection: Connection, service: String, mut tell: mpsc::Sender<Heard>) {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")
        .interface("org.freedesktop.DBus")
        .member("NameOwnerChanged")
        .arg(0, service.clone())
        .arg(2, "")
        .build();
    let Ok(rule) = rule else {
        return;
    };
    let Ok(mut left) = MessageStream::for_match_rule(rule, &connection, None).await else {
        return;
    };
    // It may have left before anyone listened.
    if here(&connection, &service).await {
        left.next().await;
    }
    let _ = tell.send(Heard::Gone(service)).await;
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
async fn ours_items(connection: &Connection) -> Vec<String> {
    let server = connection.object_server();
    let Ok(watcher) = server.interface::<_, Watcher>(WATCHER_PATH).await else {
        return Vec::new();
    };
    let names = watcher.get().await.items.clone();
    let mut alive = Vec::new();
    for name in names {
        if here(connection, split(&name).0).await {
            alive.push(name);
        } else {
            let mut guard = watcher.get_mut().await;
            guard.items.retain(|n| *n != name);
            let emitter = watcher.signal_emitter();
            let _ = Watcher::status_notifier_item_unregistered(emitter, &name).await;
            let _ = guard
                .registered_status_notifier_items_changed(emitter)
                .await;
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
    let names = reply
        .ok()
        .and_then(|reply| reply.body().deserialize::<OwnedValue>().ok())
        .and_then(|value| Vec::<String>::try_from(value).ok())
        .unwrap_or_default();
    // Another watcher may keep an app that left; it'd be read and lost over and over.
    let mut alive = Vec::new();
    for name in names {
        if here(connection, split(&name).0).await {
            alive.push(name);
        }
    }
    alive
}

async fn here(connection: &Connection, service: &str) -> bool {
    match (DBusProxy::new(connection).await, BusName::try_from(service)) {
        (Ok(bus), Ok(service)) => bus.name_has_owner(service).await.unwrap_or(false),
        _ => false,
    }
}

async fn watch_signals(connection: Connection, mut changed: mpsc::Sender<Heard>) {
    let rules = [
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(ITEM)
            .build(),
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(WATCHER)
            .build(),
    ];
    let mut streams = Vec::new();
    for rule in rules.into_iter().flatten() {
        if let Ok(stream) = MessageStream::for_match_rule(rule, &connection, None).await {
            streams.push(stream);
        }
    }
    let mut all = iced::futures::stream::select_all(streams);
    while let Some(message) = all.next().await {
        let Ok(message) = message else {
            continue;
        };
        let header = message.header();
        let heard = match (header.interface(), header.sender(), header.path()) {
            (Some(interface), Some(sender), Some(path)) if interface == ITEM => {
                Heard::Item(sender.to_string(), path.to_string())
            }
            _ => Heard::List,
        };
        if changed
            .try_send(heard)
            .is_err_and(|err| err.is_disconnected())
        {
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
        glyph: None,
        attention,
        menu_only,
        menu,
    })
}

// The smallest picture of the set that's big enough, else the biggest, ARGB in network order, as
// RGBA and no bigger than it's drawn.
fn pixmap(value: OwnedValue) -> Option<Icon> {
    let sizes: Vec<(i32, i32, Vec<u8>)> = Vec::<(i32, i32, Vec<u8>)>::try_from(value)
        .ok()?
        .into_iter()
        .filter(|(w, h, data)| *w > 0 && *h > 0 && data.len() >= (*w * *h * 4) as usize)
        .collect();
    let (w, h, data) = sizes
        .iter()
        .filter(|(w, h, _)| (*w).min(*h) as u32 >= PIXELS)
        .min_by_key(|(w, ..)| *w)
        .or_else(|| sizes.iter().max_by_key(|(w, ..)| *w))?;
    let (w, h) = (*w as u32, *h as u32);
    let rgba = argb_to_rgba(&data[..(w * h * 4) as usize]);
    Some(fit(w, h, rgba))
}

fn fit(w: u32, h: u32, rgba: Vec<u8>) -> Icon {
    if w.max(h) <= PIXELS {
        return Icon::Pixels(w, h, rgba);
    }
    let Some(picture) = ::image::RgbaImage::from_raw(w, h, rgba) else {
        return Icon::None;
    };
    let scale = |side: u32| (side * PIXELS / w.max(h)).max(1);
    let small = ::image::imageops::resize(
        &picture,
        scale(w),
        scale(h),
        ::image::imageops::FilterType::Triangle,
    );
    Icon::Pixels(small.width(), small.height(), small.into_raw())
}

// Each item's icon made ready to draw: kept from before when it's the same icon, since a theme
// search is many file checks and a new picture is uploaded again.
pub fn draw(items: Vec<Item>, before: &[Item], theme: Option<&str>) -> Vec<Item> {
    items
        .into_iter()
        .map(|item| {
            let glyph = match before.iter().find(|b| b.icon == item.icon) {
                Some(known) => known.glyph.clone(),
                None => glyph(&item.icon, theme),
            };
            Item { glyph, ..item }
        })
        .collect()
}

fn glyph(icon: &Icon, theme: Option<&str>) -> Option<Glyph> {
    match icon {
        Icon::Pixels(w, h, rgba) => Some(Glyph::Raster(image::Handle::from_rgba(
            *w,
            *h,
            rgba.clone(),
        ))),
        Icon::Named(name, folder) => {
            match crate::apps::icon_named(name, theme, folder.as_deref())? {
                Picture::Vector(path) => Some(Glyph::Vector(svg::Handle::from_path(path))),
                Picture::Raster(path) => Some(Glyph::Raster(image::Handle::from_path(path))),
            }
        }
        Icon::None => None,
    }
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
    fn big_pixmaps_are_brought_down_to_the_size_drawn() {
        let square = |side: i32| (side, side, vec![255u8; (side * side * 4) as usize]);
        let sizes = vec![square(16), square(48), square(256)];
        let value = OwnedValue::try_from(Value::from(sizes)).unwrap();
        let Some(Icon::Pixels(w, h, rgba)) = pixmap(value) else {
            panic!("no pixels");
        };
        assert_eq!((w, h), (32, 32), "from the 48, not the 256");
        assert_eq!(rgba.len(), 32 * 32 * 4);
        let Icon::Pixels(w, h, _) = fit(64, 16, vec![0; 64 * 16 * 4]) else {
            panic!("no pixels");
        };
        assert_eq!((w, h), (32, 8), "the shape is kept");
    }

    #[test]
    fn an_icon_seen_before_keeps_its_picture() {
        let item = |icon: Icon| Item {
            service: ":1.7".into(),
            path: DEFAULT_ITEM_PATH.into(),
            title: "Telegram".into(),
            icon,
            glyph: None,
            attention: false,
            menu_only: false,
            menu: None,
        };
        let dot = Icon::Pixels(1, 1, vec![1, 2, 3, 255]);
        let first = draw(vec![item(dot.clone())], &[], None);
        let again = draw(vec![item(dot)], &first, None);
        assert!(first[0].glyph.is_some());
        assert_eq!(
            again[0].glyph, first[0].glyph,
            "the same picture, not a new upload"
        );
        let other = draw(
            vec![item(Icon::Pixels(1, 1, vec![9, 9, 9, 255]))],
            &first,
            None,
        );
        assert_ne!(other[0].glyph, first[0].glyph);
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

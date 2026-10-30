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
pub const SHOT_CARD: u32 = CONFIG_CARD + 1;

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
    // The app's desktop entry, which is usually its window's app id too.
    pub desktop: Option<String>,
    // (key, label) pairs; "default" is what a click on the card does.
    pub actions: Vec<(String, String)>,
    // A picture the app sent along, such as who wrote.
    pub image: Option<iced::widget::image::Handle>,
    // How far along something is, 0 to 100, for downloads and the like.
    pub progress: Option<u8>,
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
        desktop: None,
        actions: Vec::new(),
        image: None,
        progress: None,
        summary: capitalize(&first.message),
        body,
        urgency: Urgency::Normal,
        timeout: Some(CONFIG_SHOWN_FOR),
        received: Instant::now(),
    })
}

pub fn shot_card(path: &str) -> Notification {
    let name = std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |f| f.to_string_lossy().into_owned());
    Notification {
        id: SHOT_CARD,
        app: name,
        desktop: None,
        actions: Vec::new(),
        image: None,
        progress: None,
        summary: "Screenshot copied".into(),
        body: "Click to add arrows, boxes or hide a part".into(),
        urgency: Urgency::Normal,
        timeout: Some(Duration::from_secs(6)),
        received: Instant::now(),
    }
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

// Tells the app which of its buttons was pressed, then that the card is gone.
pub fn invoke(id: u32, key: String) -> Task<Message> {
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
                "ActionInvoked",
                &(id, key.as_str()),
            )
            .await;
        let _ = connection
            .emit_signal(
                None::<&str>,
                PATH,
                NAME,
                "NotificationClosed",
                &(id, Closed::Dismissed as u32),
            )
            .await;
    })
    .discard()
}

// A card shows pictures this big at most, so nothing larger is kept, in the card or the history.
const PICTURE: u32 = 144;

enum Picture {
    Pixels(u32, u32, Vec<u8>),
    File(std::path::PathBuf),
}

// The picture a notification carries: raw pixels, a file, or an icon given as a path.
fn picture(hints: &HashMap<String, OwnedVariant>, app_icon: &str) -> Option<Picture> {
    let pixels = ["image-data", "image_data", "icon_data"]
        .iter()
        .find_map(|key| hints.get(*key))
        .and_then(|value| {
            let value = value.try_clone().ok()?;
            <(i32, i32, i32, bool, i32, i32, Vec<u8>)>::try_from(zbus::Variant::from(value)).ok()
        })
        .and_then(|(width, height, stride, _, bits, channels, data)| {
            rgba(width, height, stride, bits, channels, &data)
        });
    if let Some((width, height, rgba)) = pixels {
        return Some(Picture::Pixels(width, height, rgba));
    }
    let path = ["image-path", "image_path"]
        .iter()
        .find_map(|key| hints.get(*key))
        .and_then(|v| v.downcast_ref::<String>().ok())
        .unwrap_or_else(|| app_icon.to_owned());
    let path = path.strip_prefix("file://").unwrap_or(&path);
    let file = Path::new(path);
    let small = std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.len() < 32 << 20);
    (file.is_absolute() && small).then(|| Picture::File(file.to_path_buf()))
}

fn shrink(picture: Picture) -> Option<iced::widget::image::Handle> {
    let image = match picture {
        Picture::Pixels(width, height, rgba) => ::image::RgbaImage::from_raw(width, height, rgba)?,
        Picture::File(file) => ::image::open(file).ok()?.into_rgba8(),
    };
    let (width, height) = image.dimensions();
    let image = if width.max(height) > PICTURE {
        let scale = PICTURE as f32 / width.max(height) as f32;
        ::image::imageops::thumbnail(
            &image,
            ((width as f32 * scale).round() as u32).max(1),
            ((height as f32 * scale).round() as u32).max(1),
        )
    } else {
        image
    };
    let (width, height) = image.dimensions();
    Some(iced::widget::image::Handle::from_rgba(
        width,
        height,
        image.into_raw(),
    ))
}

fn rgba(
    width: i32,
    height: i32,
    stride: i32,
    bits: i32,
    channels: i32,
    data: &[u8],
) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h, stride) = (
        usize::try_from(width).ok()?,
        usize::try_from(height).ok()?,
        usize::try_from(stride).ok()?,
    );
    let channels = usize::try_from(channels)
        .ok()
        .filter(|c| matches!(c, 3 | 4))?;
    if bits != 8 || w == 0 || h == 0 || stride < w * channels {
        return None;
    }
    if data.len() < stride * (h - 1) + w * channels {
        return None;
    }
    let mut out = Vec::with_capacity(w * h * 4);
    for row in data.chunks(stride).take(h) {
        for px in row[..w * channels].chunks(channels) {
            out.extend_from_slice(&px[..3]);
            out.push(if channels == 4 { px[3] } else { 255 });
        }
    }
    Some((w as u32, h as u32, out))
}

impl Notification {
    pub fn default_action(&self) -> bool {
        self.actions.iter().any(|(key, _)| key == "default")
    }

    pub fn buttons(&self) -> impl Iterator<Item = &(String, String)> {
        self.actions
            .iter()
            .filter(|(key, label)| key != "default" && !label.trim().is_empty())
            .take(3)
    }
}

struct Server {
    events: Sender<Message>,
    last: u32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&str> {
        vec!["actions", "body"]
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        expire_timeout: i32,
    ) -> u32 {
        // Apolline's own cards aren't anyone else's to replace.
        // Only an id handed out since this start can be replaced; anything else is new.
        let id = if replaces_id == 0 || replaces_id > self.last {
            self.last += 1;
            self.last
        } else {
            replaces_id
        };
        let level = hints
            .get("urgency")
            .and_then(|v| v.downcast_ref::<u8>().ok());
        let urgency = urgency(level);
        let desktop = hints
            .get("desktop-entry")
            .and_then(|v| v.downcast_ref::<String>().ok())
            .filter(|entry| !entry.is_empty());
        let actions = actions
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[key, label]| (key.clone(), label.clone()))
            .collect();
        let image = match picture(&hints, &app_icon) {
            Some(picture) => tokio::task::spawn_blocking(move || shrink(picture))
                .await
                .ok()
                .flatten(),
            None => None,
        };
        let progress = hints
            .get("value")
            .and_then(|v| v.downcast_ref::<i32>().ok())
            .map(|value| value.clamp(0, 100) as u8);
        let notification = Notification {
            id,
            app: app_name,
            desktop,
            actions,
            image,
            progress,
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
    fn raw_pictures_become_rgba() {
        // Two RGB pixels a row, rows padded to 8 bytes.
        let data = [1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0];
        let (w, h, out) = rgba(2, 2, 8, 8, 3, &data).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(
            out,
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
        assert_eq!(rgba(2, 2, 8, 8, 3, &data[..10]), None, "too short");
        assert_eq!(rgba(2, 2, 8, 16, 3, &data), None, "16 bits a channel");
        assert_eq!(rgba(2, 2, 4, 8, 3, &data), None, "rows shorter than pixels");
    }

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

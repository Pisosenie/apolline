use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

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
pub const BATTERY_CARD: u32 = CONFIG_CARD + 2;
// Calendar reminders each get their own card from here on, so one doesn't push out another.
pub const REMINDER_CARD: u32 = CONFIG_CARD + 3;
const REMINDERS: u32 = 1000;
// Past the reminders and the timers.
pub const SAFE_CARD: u32 = CONFIG_CARD + 3000;

pub fn is_reminder(id: u32) -> bool {
    (REMINDER_CARD..REMINDER_CARD + REMINDERS).contains(&id)
}

// What the history kept from before this start: no app is left to answer for these ids.
pub const PAST_CARD: u32 = CONFIG_CARD + 4096;
pub const PASTS: u32 = 64;

pub fn is_past(id: u32) -> bool {
    (PAST_CARD..PAST_CARD + PASTS).contains(&id)
}

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
    // Not worth keeping once it's gone, like a volume level another tool shows.
    pub transient: bool,
    // Stays after one of its buttons is pressed, for a player's controls and the like.
    pub resident: bool,
    // A chat app takes an answer typed right on the card; this is what the field says before.
    pub reply: Option<String>,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout: Option<Duration>,
    pub received: SystemTime,
    // What the app asked to be heard, if anything.
    pub sound: Heard,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Heard {
    #[default]
    Nothing,
    // suppress-sound: the app plays its own, or wants none.
    Quiet,
    File(std::path::PathBuf),
    // A name from the sound theme, like message-new-instant.
    Named(String),
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
        transient: false,
        resident: false,
        reply: None,
        summary: capitalize(&first.message),
        body,
        urgency: Urgency::Normal,
        timeout: Some(CONFIG_SHOWN_FOR),
        received: SystemTime::now(),
        sound: Heard::Nothing,
    })
}

// Running low on battery; it stays until it's dismissed or the charger goes in.
pub fn battery_card(mark: u8, battery: crate::power::Battery) -> Notification {
    let percent = battery.percent;
    let body = match battery.time_left() {
        Some(left) => crate::i18n::fill(
            "{percent}% left, about {left}. Time to plug in.",
            &[("percent", &percent), ("left", &left)],
        ),
        None => crate::i18n::fill(
            "{percent}% left. Time to plug in.",
            &[("percent", &percent)],
        ),
    };
    Notification {
        id: BATTERY_CARD,
        app: "Battery".into(),
        desktop: None,
        actions: Vec::new(),
        image: None,
        progress: None,
        transient: false,
        resident: false,
        reply: None,
        summary: crate::i18n::t(if mark <= 5 {
            "Battery almost empty"
        } else {
            "Battery low"
        })
        .into(),
        body,
        urgency: Urgency::Critical,
        timeout: None,
        received: SystemTime::now(),
        sound: Heard::Nothing,
    }
}

// Something from the calendar about to start; a Join button when it has a call. The button's key
// is the link itself.
pub fn reminder_card(
    occurrence: &crate::events::Occurrence,
    now: chrono::NaiveDateTime,
    count: u32,
) -> Notification {
    let begins = occurrence.begins().unwrap_or(now);
    let wait = ((begins - now).num_seconds() + 59) / 60;
    let mut said = vec![if wait > 0 {
        crate::i18n::fill("In {wait} min", &[("wait", &wait)])
    } else {
        crate::i18n::t("Now").to_owned()
    }];
    if let (Some(start), Some(end)) = (occurrence.start, occurrence.end) {
        said.push(format!(
            "{} – {}",
            start.format("%H:%M"),
            end.format("%H:%M")
        ));
    }
    said.extend(occurrence.location.as_deref().map(str::to_owned));
    // Gone ten minutes into the meeting, by when it's been answered or missed.
    let shown_for = (begins - now + chrono::TimeDelta::minutes(10))
        .to_std()
        .unwrap_or(Duration::from_secs(60));
    Notification {
        id: REMINDER_CARD + count % REMINDERS,
        app: "Calendar".into(),
        desktop: None,
        actions: occurrence
            .call
            .iter()
            .map(|link| (link.to_string(), crate::i18n::t("Join").to_owned()))
            .collect(),
        image: None,
        progress: None,
        transient: false,
        resident: false,
        reply: None,
        summary: occurrence.summary.to_string(),
        body: said.join(" · "),
        urgency: Urgency::Normal,
        timeout: Some(shown_for),
        received: SystemTime::now(),
        sound: Heard::Nothing,
    }
}

// The shell kept crashing and runs safe; a click starts it again as usual. Why is in the crash log.
pub fn safe_card() -> Notification {
    Notification {
        id: SAFE_CARD,
        app: "Apolline".into(),
        desktop: None,
        actions: Vec::new(),
        image: None,
        progress: None,
        transient: false,
        resident: false,
        reply: None,
        summary: crate::i18n::t("Safe mode after crashes").into(),
        body: crate::i18n::t("Settings untouched; click to start normally").into(),
        urgency: Urgency::Critical,
        timeout: None,
        received: SystemTime::now(),
        sound: Heard::Nothing,
    }
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
        transient: false,
        resident: false,
        reply: None,
        summary: crate::i18n::t("Screenshot copied").into(),
        body: crate::i18n::t("Click to add arrows, boxes or hide a part").into(),
        urgency: Urgency::Normal,
        timeout: Some(Duration::from_secs(6)),
        received: SystemTime::now(),
        sound: Heard::Nothing,
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

// Tells the app which of its buttons was pressed, and nothing more: the card stays.
pub fn act(id: u32, key: String) -> Task<Message> {
    let Some(connection) = CONNECTION.get().cloned().filter(|_| id < CONFIG_CARD) else {
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

// What was typed on the card goes to the app, then the card is gone, as after a button.
pub fn replied(id: u32, text: String) -> Task<Message> {
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
                "NotificationReplied",
                &(id, text.as_str()),
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
fn heard(hints: &HashMap<String, OwnedVariant>) -> Heard {
    let text = |key: &str| {
        hints
            .get(key)
            .and_then(|v| v.downcast_ref::<&str>().ok())
            .filter(|t| !t.trim().is_empty())
            .map(str::to_owned)
    };
    let quiet = hints
        .get("suppress-sound")
        .and_then(|v| v.downcast_ref::<bool>().ok())
        .unwrap_or(false);
    if quiet {
        Heard::Quiet
    } else if let Some(file) = text("sound-file") {
        Heard::File(file.into())
    } else if let Some(name) = text("sound-name") {
        Heard::Named(name)
    } else {
        Heard::Nothing
    }
}

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

// Any app names the file, so a small one that unpacks into gigabytes is turned down.
fn decode(file: &Path) -> Option<::image::RgbaImage> {
    let mut reader = ::image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 << 20);
    reader.limits(limits);
    Some(reader.decode().ok()?.into_rgba8())
}

fn shrink(picture: Picture) -> Option<iced::widget::image::Handle> {
    let image = match picture {
        Picture::Pixels(width, height, rgba) => ::image::RgbaImage::from_raw(width, height, rgba)?,
        Picture::File(file) => decode(&file)?,
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
    // By the wall clock, so a card kept from yesterday says so.
    pub fn age(&self) -> Duration {
        self.received.elapsed().unwrap_or_default()
    }

    pub fn default_action(&self) -> bool {
        self.actions.iter().any(|(key, _)| key == "default")
    }

    pub fn buttons(&self) -> impl Iterator<Item = &(String, String)> {
        self.actions
            .iter()
            .filter(|(key, label)| {
                !matches!(key.as_str(), "default" | "inline-reply") && !label.trim().is_empty()
            })
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
        vec!["actions", "body", "inline-reply"]
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
        let actions: Vec<(String, String)> = actions
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
        let flag = |key: &str| {
            hints
                .get(key)
                .and_then(|v| v.downcast_ref::<bool>().ok())
                .unwrap_or(false)
        };
        // KDE's extension, which Telegram and other chat apps use when it's offered.
        let reply = actions
            .iter()
            .any(|(key, _)| key == "inline-reply")
            .then(|| {
                hints
                    .get("x-kde-reply-placeholder-text")
                    .and_then(|v| v.downcast_ref::<&str>().ok())
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or("Reply…")
                    .to_owned()
            });
        let notification = Notification {
            id,
            app: app_name,
            desktop,
            actions,
            image,
            progress,
            transient: flag("transient"),
            resident: flag("resident"),
            reply,
            summary,
            body,
            urgency,
            timeout: timeout(expire_timeout, urgency),
            received: SystemTime::now(),
            sound: heard(&hints),
        };
        let _ = self.events.send(Message::Notify(notification)).await;
        id
    }

    async fn close_notification(&mut self, id: u32) {
        // Only what an app was handed; the shell's own cards aren't anyone's to close.
        if id == 0 || id > self.last {
            return;
        }
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
    fn a_picture_too_big_to_unpack_is_left_out() {
        let dir = std::env::temp_dir().join(format!("apolline-picture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (small, wide) = (dir.join("small.png"), dir.join("wide.png"));
        ::image::RgbaImage::new(2, 2).save(&small).unwrap();
        ::image::GrayImage::new(5000, 1).save(&wide).unwrap();
        assert_eq!(decode(&small).map(|i| i.dimensions()), Some((2, 2)));
        assert!(decode(&wide).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

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

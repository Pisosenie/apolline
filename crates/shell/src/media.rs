use std::collections::HashMap;

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt, stream};
use zbus::OwnedVariant;
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::proxy::CacheProperties;
use zbus::{Connection, MatchRule, MessageStream};

use crate::shell::Message;

const PLAYERS: &str = "org.mpris.MediaPlayer2";
const LONGEST: usize = 48;

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub player: String,
    pub title: String,
    pub artist: Option<String>,
    pub playing: bool,
}

impl Track {
    pub fn sample(title: &str, artist: &str) -> Self {
        Self {
            player: String::new(),
            title: title.to_owned(),
            artist: Some(artist.to_owned()),
            playing: true,
        }
    }

    pub fn line(&self) -> String {
        let line = match &self.artist {
            Some(artist) => format!("{} — {artist}", self.title),
            None => self.title.clone(),
        };
        shorten(&line, LONGEST)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Toggle,
    Next,
    Previous,
}

pub async fn control(player: String, action: Action) {
    let call = async {
        let connection = Connection::session().await?;
        let proxy = PlayerProxy::builder(&connection)
            .destination(player.as_str())
            .build()
            .await?;
        match action {
            Action::Toggle => proxy.play_pause().await,
            Action::Next => proxy.next().await,
            Action::Previous => proxy.previous().await,
        }
    };
    if let Err(err) = call.await {
        eprintln!("apolline: media: {err}");
    }
}

pub fn subscription() -> Subscription<Message> {
    Subscription::run(watch)
}

fn watch() -> impl Stream<Item = Message> {
    iced::stream::channel(4, async |mut output| {
        let Ok(connection) = Connection::session().await else {
            return;
        };
        let Ok(dbus) = DBusProxy::new(&connection).await else {
            return;
        };
        let changed = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.freedesktop.DBus.Properties")
            .member("PropertiesChanged")
            .path("/org/mpris/MediaPlayer2")
            .build();
        let owners = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.DBus")
            .interface("org.freedesktop.DBus")
            .member("NameOwnerChanged")
            .arg0ns(PLAYERS)
            .build();
        let (Ok(changed), Ok(owners)) = (changed, owners) else {
            return;
        };
        let (Ok(changed), Ok(owners)) = (
            MessageStream::for_match_rule(changed, &connection, None).await,
            MessageStream::for_match_rule(owners, &connection, None).await,
        ) else {
            return;
        };
        let mut events = stream::select(changed, owners);

        let mut shown = None;
        loop {
            let now = playing(&connection, &dbus).await;
            if now != shown {
                if output.send(Message::Media(now.clone())).await.is_err() {
                    return;
                }
                shown = now;
            }
            if events.next().await.is_none() {
                return;
            }
        }
    })
}

async fn playing(connection: &Connection, dbus: &DBusProxy<'_>) -> Option<Track> {
    let mut paused = None;
    for name in dbus.list_names().await.ok()? {
        if !name.starts_with(PLAYERS) {
            continue;
        }
        let Ok(player) = PlayerProxy::builder(connection)
            .destination(name.as_str())
            .cache_properties(CacheProperties::No)
            .build()
            .await
        else {
            continue;
        };
        let playing = match player.playback_status().await.ok().as_deref() {
            Some("Playing") => true,
            Some("Paused") => false,
            _ => continue,
        };
        let metadata = player.metadata().await.unwrap_or_default();
        let found = track(
            name.to_string(),
            title(&metadata),
            artists(&metadata),
            playing,
        );
        if playing && found.is_some() {
            return found;
        }
        paused = paused.or(found);
    }
    paused
}

fn title(metadata: &HashMap<String, OwnedVariant>) -> Option<String> {
    metadata
        .get("xesam:title")?
        .downcast_ref::<&str>()
        .ok()
        .map(str::to_owned)
}

fn artists(metadata: &HashMap<String, OwnedVariant>) -> Vec<String> {
    metadata
        .get("xesam:artist")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| Vec::<String>::try_from(value).ok())
        .unwrap_or_default()
}

fn track(
    player: String,
    title: Option<String>,
    artists: Vec<String>,
    playing: bool,
) -> Option<Track> {
    let title = title.filter(|t| !t.trim().is_empty())?;
    let artist = artists.into_iter().find(|a| !a.trim().is_empty());
    Some(Track {
        player,
        title,
        artist,
        playing,
    })
}

pub fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max - 1).collect();
    short.truncate(short.trim_end().len());
    short + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reads_title_and_first_artist() {
        let at = |title: Option<&str>, artists: &[&str]| {
            let artists = artists.iter().map(|a| a.to_string()).collect();
            track("p".into(), title.map(Into::into), artists, true).map(|t| t.line())
        };
        assert_eq!(
            at(Some("Night Drive"), &["Low Tide", "Guest"]).as_deref(),
            Some("Night Drive — Low Tide")
        );
        assert_eq!(at(Some("Night Drive"), &[]).as_deref(), Some("Night Drive"));
        assert_eq!(at(Some("  "), &["Low Tide"]), None);
        assert_eq!(at(None, &[]), None);
    }

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        assert_eq!(shorten("short", 10), "short");
        assert_eq!(shorten("a very long title indeed", 10), "a very lo…");
        assert_eq!(shorten("ночной трек без конца", 8).chars().count(), 7);
    }

    #[test]
    fn metadata_values_are_read_from_variants() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "xesam:title".to_owned(),
            OwnedVariant::try_from(zbus::Variant::from("Ember Run")).unwrap(),
        );
        let list = zbus::Variant::from(vec!["Low Tide".to_owned()]);
        metadata.insert(
            "xesam:artist".to_owned(),
            OwnedVariant::try_from(list).unwrap(),
        );
        assert_eq!(title(&metadata).as_deref(), Some("Ember Run"));
        assert_eq!(artists(&metadata), ["Low Tide"]);
    }
}

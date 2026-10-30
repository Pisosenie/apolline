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
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
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

async fn playing(connection: &Connection, dbus: &DBusProxy<'_>) -> Option<String> {
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
        if player.playback_status().await.ok().as_deref() != Some("Playing") {
            continue;
        }
        let metadata = player.metadata().await.unwrap_or_default();
        return track(title(&metadata), artists(&metadata));
    }
    None
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

fn track(title: Option<String>, artists: Vec<String>) -> Option<String> {
    let title = title.filter(|t| !t.trim().is_empty())?;
    let line = match artists.first().filter(|a| !a.trim().is_empty()) {
        Some(artist) => format!("{title} — {artist}"),
        None => title,
    };
    Some(shorten(&line, LONGEST))
}

fn shorten(text: &str, max: usize) -> String {
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
        let artists = vec!["Low Tide".to_owned(), "Guest".to_owned()];
        assert_eq!(
            track(Some("Night Drive".into()), artists),
            Some("Night Drive — Low Tide".into())
        );
        assert_eq!(
            track(Some("Night Drive".into()), vec![]),
            Some("Night Drive".into())
        );
        assert_eq!(track(Some("  ".into()), vec!["Low Tide".into()]), None);
        assert_eq!(track(None, vec![]), None);
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

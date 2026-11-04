use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt};
use zbus::message::Type;
use zbus::proxy::CacheProperties;
use zbus::{Connection, MatchRule, MessageStream};

use crate::shell::Message;

const DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    // Minutes until empty, or until full while charging, when UPower knows.
    pub left: Option<u32>,
}

impl Battery {
    // "3 h 10 min", "45 min".
    pub fn time_left(&self) -> Option<String> {
        let minutes = self.left?;
        Some(match (minutes / 60, minutes % 60) {
            (0, m) => crate::i18n::fill("{m} min", &[("m", &m)]),
            (h, 0) => crate::i18n::fill("{h} h", &[("h", &h)]),
            (h, m) => crate::i18n::fill("{h} h {m} min", &[("h", &h), ("m", &m)]),
        })
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait Device {
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;
}

pub fn subscription() -> Subscription<Message> {
    Subscription::run(stream)
}

fn stream() -> impl Stream<Item = Message> {
    iced::stream::channel(4, async |mut output| {
        let Ok(connection) = Connection::system().await else {
            return;
        };
        let Ok(device) = DeviceProxy::builder(&connection)
            .cache_properties(CacheProperties::No)
            .build()
            .await
        else {
            return;
        };
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.UPower")
            .interface("org.freedesktop.DBus.Properties")
            .member("PropertiesChanged")
            .path(DEVICE)
            .build();
        let Ok(rule) = rule else {
            return;
        };
        let Ok(mut changes) = MessageStream::for_match_rule(rule, &connection, None).await else {
            return;
        };

        // UPower works the time out again every little while; only what shows is sent on.
        let mut shown = None;
        loop {
            let now = read(&device).await;
            if shown != Some(now) {
                shown = Some(now);
                if output.send(Message::Battery(now)).await.is_err() {
                    return;
                }
            }
            if changes.next().await.is_none() {
                return;
            }
        }
    })
}

async fn read(device: &DeviceProxy<'_>) -> Option<Battery> {
    let mut battery = battery(
        device.is_present().await.ok()?,
        device.percentage().await.ok()?,
        device.state().await.ok()?,
    )?;
    let seconds = if battery.charging {
        device.time_to_full().await
    } else {
        device.time_to_empty().await
    };
    battery.left = seconds.ok().and_then(minutes);
    Some(battery)
}

fn battery(present: bool, percentage: f64, state: u32) -> Option<Battery> {
    present.then(|| Battery {
        percent: percentage.round().clamp(0.0, 100.0) as u8,
        charging: matches!(state, 1 | 4 | 5),
        left: None,
    })
}

// To the minute when it's nearly over, else in fives, so it doesn't change every few seconds.
fn minutes(seconds: i64) -> Option<u32> {
    let minutes = u32::try_from(seconds / 60).ok().filter(|m| *m > 0)?;
    Some(if minutes < 15 {
        minutes
    } else {
        (minutes + 2) / 5 * 5
    })
}

// The lowest mark the battery has gone under while running on it, to warn once at each.
pub fn low(battery: Option<Battery>) -> Option<u8> {
    let battery = battery.filter(|b| !b.charging)?;
    [5, 10].into_iter().find(|mark| battery.percent <= *mark)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_without_a_battery_shows_nothing() {
        assert_eq!(battery(false, 0.0, 0), None);
    }

    #[test]
    fn the_time_left_reads_like_a_person_would_say_it() {
        assert_eq!(minutes(0), None, "UPower doesn't know yet");
        assert_eq!(minutes(7 * 60 + 40), Some(7));
        assert_eq!(minutes(3 * 3600 + 11 * 60), Some(190));
        let left = |minutes| Battery {
            percent: 50,
            charging: false,
            left: Some(minutes),
        };
        assert_eq!(left(190).time_left().as_deref(), Some("3 h 10 min"));
        assert_eq!(left(120).time_left().as_deref(), Some("2 h"));
        assert_eq!(left(7).time_left().as_deref(), Some("7 min"));
    }

    #[test]
    fn it_warns_under_ten_and_five_percent_unplugged() {
        let at = |percent, charging| {
            Some(Battery {
                percent,
                charging,
                left: None,
            })
        };
        assert_eq!(low(at(11, false)), None);
        assert_eq!(low(at(10, false)), Some(10));
        assert_eq!(low(at(4, false)), Some(5));
        assert_eq!(low(at(4, true)), None, "plugged in");
        assert_eq!(low(None), None);
    }

    #[test]
    fn plugged_in_states_count_as_charging() {
        let at = |state| battery(true, 81.6, state).unwrap();
        assert_eq!(
            at(2),
            Battery {
                percent: 82,
                charging: false,
                left: None,
            }
        );
        assert!(at(1).charging);
        assert!(at(4).charging);
        assert!(at(5).charging);
        assert!(!at(6).charging);
    }
}

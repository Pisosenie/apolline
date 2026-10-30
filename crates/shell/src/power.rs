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

        loop {
            if output
                .send(Message::Battery(read(&device).await))
                .await
                .is_err()
            {
                return;
            }
            if changes.next().await.is_none() {
                return;
            }
        }
    })
}

async fn read(device: &DeviceProxy<'_>) -> Option<Battery> {
    battery(
        device.is_present().await.ok()?,
        device.percentage().await.ok()?,
        device.state().await.ok()?,
    )
}

fn battery(present: bool, percentage: f64, state: u32) -> Option<Battery> {
    present.then(|| Battery {
        percent: percentage.round().clamp(0.0, 100.0) as u8,
        charging: matches!(state, 1 | 4 | 5),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_without_a_battery_shows_nothing() {
        assert_eq!(battery(false, 0.0, 0), None);
    }

    #[test]
    fn plugged_in_states_count_as_charging() {
        let at = |state| battery(true, 81.6, state).unwrap();
        assert_eq!(
            at(2),
            Battery {
                percent: 82,
                charging: false
            }
        );
        assert!(at(1).charging);
        assert!(at(4).charging);
        assert!(at(5).charging);
        assert!(!at(6).charging);
    }
}

use std::collections::HashMap;

use zbus::proxy::CacheProperties;
use zbus::{Connection, OwnedVariant};

const PERFORMANCE: &str = "performance";

#[zbus::proxy(
    interface = "org.freedesktop.UPower.PowerProfiles",
    default_service = "org.freedesktop.UPower.PowerProfiles",
    default_path = "/org/freedesktop/UPower/PowerProfiles"
)]
trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, profile: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn profiles(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profiles {
    pub active: String,
    pub available: Vec<String>,
}

impl Profiles {
    // The next one round: power saver, balanced, performance.
    pub fn next(&self) -> Option<String> {
        let at = self.available.iter().position(|p| *p == self.active)?;
        let next = &self.available[(at + 1) % self.available.len()];
        (*next != self.active).then(|| next.clone())
    }
}

pub fn subscription() -> iced::Subscription<Option<Profiles>> {
    iced::Subscription::run(watch)
}

fn watch() -> impl iced::futures::Stream<Item = Option<Profiles>> {
    use iced::futures::SinkExt;
    iced::stream::channel(1, async |mut output| {
        let Ok(connection) = Connection::system().await else {
            return;
        };
        let Some(mut changes) =
            crate::bus::changes(&connection, "org.freedesktop.UPower.PowerProfiles").await
        else {
            return;
        };
        let mut shown = None;
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

async fn read(connection: &Connection) -> Option<Profiles> {
    let profiles = PowerProfilesProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()?;
    let mut available = names(&profiles.profiles().await.ok()?);
    let order = |p: &String| {
        ["power-saver", "balanced", PERFORMANCE]
            .iter()
            .position(|o| o == p)
    };
    available.sort_by_key(order);
    Some(Profiles {
        active: profiles.active_profile().await.ok()?,
        available,
    })
}

fn names(profiles: &[HashMap<String, OwnedVariant>]) -> Vec<String> {
    profiles
        .iter()
        .filter_map(|p| {
            p.get("Profile")?
                .downcast_ref::<&str>()
                .ok()
                .map(str::to_owned)
        })
        .collect()
}

pub async fn choose(profile: String) -> Result<(), String> {
    let profiles = proxy().await.ok_or("power-profiles-daemon isn't running")?;
    profiles
        .set_active_profile(&profile)
        .await
        .map_err(|err| match err {
            zbus::Error::MethodError(_, Some(text), _) => text,
            err => err.to_string(),
        })
}

pub async fn boost() -> Option<String> {
    let profiles = proxy().await?;
    let available = names(&profiles.profiles().await.ok()?);
    let previous = plan(&profiles.active_profile().await.ok()?, &available)?;
    profiles.set_active_profile(PERFORMANCE).await.ok()?;
    Some(previous)
}

pub async fn restore(previous: String) {
    let Some(profiles) = proxy().await else {
        return;
    };
    if profiles.active_profile().await.ok().as_deref() == Some(PERFORMANCE) {
        let _ = profiles.set_active_profile(&previous).await;
    }
}

async fn proxy() -> Option<PowerProfilesProxy<'static>> {
    let connection = Connection::system().await.ok()?;
    PowerProfilesProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()
}

fn plan(current: &str, available: &[String]) -> Option<String> {
    let can_boost = available.iter().any(|p| p == PERFORMANCE);
    (can_boost && current != PERFORMANCE).then(|| current.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_tile_goes_round_what_the_machine_has() {
        let at = |active: &str, available: &[&str]| Profiles {
            active: active.into(),
            available: names(available),
        };
        let all = ["power-saver", "balanced", "performance"];
        assert_eq!(at("balanced", &all).next().as_deref(), Some("performance"));
        assert_eq!(
            at("performance", &all).next().as_deref(),
            Some("power-saver")
        );
        assert_eq!(
            at("balanced", &["power-saver", "balanced"])
                .next()
                .as_deref(),
            Some("power-saver")
        );
        assert_eq!(at("balanced", &["balanced"]).next(), None);
    }

    #[test]
    fn boosts_only_when_performance_exists_and_is_not_already_on() {
        let all = names(&["power-saver", "balanced", "performance"]);
        assert_eq!(plan("balanced", &all), Some("balanced".into()));
        assert_eq!(plan("performance", &all), None);
        assert_eq!(plan("balanced", &names(&["power-saver", "balanced"])), None);
    }
}

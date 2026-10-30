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

pub async fn boost() -> Option<String> {
    let profiles = proxy().await?;
    let available: Vec<String> = profiles
        .profiles()
        .await
        .ok()?
        .iter()
        .filter_map(|p| {
            p.get("Profile")?
                .downcast_ref::<&str>()
                .ok()
                .map(str::to_owned)
        })
        .collect();
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
    fn boosts_only_when_performance_exists_and_is_not_already_on() {
        let all = names(&["power-saver", "balanced", "performance"]);
        assert_eq!(plan("balanced", &all), Some("balanced".into()));
        assert_eq!(plan("performance", &all), None);
        assert_eq!(plan("balanced", &names(&["power-saver", "balanced"])), None);
    }
}

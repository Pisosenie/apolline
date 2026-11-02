use std::collections::HashMap;
use std::path::PathBuf;

// How often and how lately each command was started from the launcher, so the ones in use come
// first. Kept in ~/.local/state/apolline/launches.json as { "command": [times, unix seconds] }.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage(HashMap<String, (u32, u64)>);

// Past this many commands the least used are forgotten.
const KEEP: usize = 100;
const HOUR: u64 = 3600;

impl Usage {
    pub fn load() -> Usage {
        let read = || -> Option<Usage> {
            let text = std::fs::read_to_string(path()?).ok()?;
            let map: HashMap<String, (u32, u64)> = serde_json::from_str(&text).ok()?;
            Some(Usage(map))
        };
        read().unwrap_or_default()
    }

    pub fn note(&mut self, command: &str, now: u64) {
        let (times, last) = self.0.entry(command.to_owned()).or_default();
        *times = times.saturating_add(1);
        *last = now;
        if self.0.len() > KEEP {
            let least = self
                .0
                .iter()
                .filter(|(name, _)| *name != command)
                .min_by_key(|(_, (times, last))| (*times, *last))
                .map(|(command, _)| command.clone());
            if let Some(command) = least {
                self.0.remove(&command);
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| {
                std::fs::write(&path, serde_json::to_string(&self.0).unwrap_or_default())
            });
        if let Err(err) = written {
            eprintln!("apolline: {}: {err}", path.display());
        }
    }

    // More for more starts, and much more for recent ones.
    pub fn weight(&self, command: &str, now: u64) -> u32 {
        let Some((times, last)) = self.0.get(command) else {
            return 0;
        };
        let age = now.saturating_sub(*last);
        let lately = match age {
            a if a < HOUR => 8,
            a if a < 24 * HOUR => 4,
            a if a < 7 * 24 * HOUR => 2,
            _ => 1,
        };
        times.min(&50) * lately
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("apolline/launches.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_and_frequent_weigh_more() {
        let now = 100_000_000;
        let mut usage = Usage::default();
        usage.note("firefox", now - 10);
        usage.note("steam", now - 30 * 24 * HOUR);
        usage.note("steam", now - 30 * 24 * HOUR);
        assert!(usage.weight("firefox", now) > usage.weight("steam", now));
        usage.note("steam", now);
        assert!(usage.weight("steam", now) > usage.weight("firefox", now));
        assert_eq!(usage.weight("never", now), 0);
    }

    #[test]
    fn the_least_used_are_forgotten_past_the_limit() {
        let mut usage = Usage::default();
        for i in 0..KEEP {
            usage.note(&format!("app{i}"), 100);
            usage.note(&format!("app{i}"), 100);
        }
        usage.note("once", 50);
        usage.note("new", 200);
        assert_eq!(usage.0.len(), KEEP);
        assert!(!usage.0.contains_key("once"));
        assert!(usage.0.contains_key("new"), "what was just started stays");
    }
}

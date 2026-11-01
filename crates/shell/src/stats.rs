use std::path::Path;
use std::time::Duration;

use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::shell::Message;

const EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub cpu: Option<u8>,
    pub memory: Option<u8>,
    pub temperature: Option<i16>,
}

pub fn subscription(on: bool) -> Subscription<Message> {
    if on {
        Subscription::run(watch)
    } else {
        Subscription::none()
    }
}

// Sends only what changed, so a quiet machine doesn't repaint the bar every few seconds.
fn watch() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| {
        let mut before: Option<(u64, u64)> = None;
        let mut shown = Stats::default();
        loop {
            let read = tokio::task::spawn_blocking(move || {
                let times = std::fs::read_to_string("/proc/stat")
                    .ok()
                    .and_then(|text| cpu_times(&text));
                let memory = std::fs::read_to_string("/proc/meminfo")
                    .ok()
                    .and_then(|text| memory(&text));
                (times, memory, temperature())
            })
            .await;
            if let Ok((times, memory, temperature)) = read {
                let cpu = match (before, times) {
                    (Some((busy, total)), Some((busy_now, total_now))) if total_now > total => {
                        let used = busy_now.saturating_sub(busy) as f64;
                        // In steps of 5, so a busy-ish machine doesn't repaint the bar every time.
                        Some(((used / (total_now - total) as f64 * 20.0).round() * 5.0) as u8)
                    }
                    _ => None,
                };
                before = times;
                let stats = Stats {
                    cpu,
                    memory,
                    temperature,
                };
                if stats != shown && (stats.cpu.is_some() || before.is_none()) {
                    shown = stats;
                    if output.send(Message::Stats(stats)).await.is_err() {
                        return;
                    }
                }
            }
            tokio::time::sleep(EVERY).await;
        }
    })
}

// Busy and total jiffies from the first line of /proc/stat.
fn cpu_times(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().find(|line| line.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|n| n.parse().ok())
        .collect();
    let total: u64 = fields.iter().take(8).sum();
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    Some((total.saturating_sub(idle), total))
}

// The share of memory in use, counting what the kernel could hand back as free.
fn memory(meminfo: &str) -> Option<u8> {
    let field = |name: &str| -> Option<u64> {
        meminfo
            .lines()
            .find(|line| line.starts_with(name))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:")?;
    (total > 0).then(|| ((total.saturating_sub(available)) * 100 / total) as u8)
}

// The CPU's own sensor when there's one, else the first thermal zone that says it's the CPU.
fn temperature() -> Option<i16> {
    const CPUS: [&str; 6] = [
        "k10temp",
        "coretemp",
        "zenpower",
        "cpu_thermal",
        "soc_thermal",
        "acpitz",
    ];
    let read = |path: &Path| -> Option<i16> {
        let milli: i64 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
        i16::try_from(milli / 2000 * 2).ok()
    };
    let hwmon = std::fs::read_dir("/sys/class/hwmon").ok()?;
    let mut sensors: Vec<(usize, std::path::PathBuf)> = hwmon
        .flatten()
        .filter_map(|entry| {
            let name = std::fs::read_to_string(entry.path().join("name")).ok()?;
            let rank = CPUS.iter().position(|cpu| *cpu == name.trim())?;
            Some((rank, entry.path().join("temp1_input")))
        })
        .collect();
    sensors.sort();
    if let Some(found) = sensors.iter().find_map(|(_, path)| read(path)) {
        return Some(found);
    }
    std::fs::read_dir("/sys/class/thermal")
        .ok()?
        .flatten()
        .filter(|entry| {
            std::fs::read_to_string(entry.path().join("type"))
                .is_ok_and(|kind| kind.contains("x86_pkg_temp") || kind.contains("cpu"))
        })
        .find_map(|entry| read(&entry.path().join("temp")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_files_read_into_shares() {
        let stat =
            "cpu  4705 356 584 3699176 23 23 0 0 0 0\ncpu0 1393 280 157 924834 4 4 0 0 0 0\n";
        assert_eq!(
            cpu_times(stat),
            Some((5668, 3704867)),
            "idle and iowait aren't busy"
        );
        let meminfo = "MemTotal:       16000000 kB\nMemFree:         1000000 kB\nMemAvailable:    6000000 kB\n";
        assert_eq!(memory(meminfo), Some(62));
        assert_eq!(memory("garbage"), None);
        assert_eq!(cpu_times("intr 1 2 3"), None);
    }
}

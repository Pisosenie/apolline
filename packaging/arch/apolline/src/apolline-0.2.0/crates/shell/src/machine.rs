// What the machine is doing, read twice a second or so while the monitor is open and never
// otherwise: load and memory over the last minute and a half, the network, the disk, and the
// processes doing the most.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::shell::Message;
use crate::stats;

const EVERY: Duration = Duration::from_millis(1500);
pub const KEPT: usize = 60;
const LISTED: usize = 8;
// What the session stands on: ending one of these from here would take the desktop with it.
const HELD: [&str; 7] = [
    "niri",
    "apolline",
    "systemd",
    "dbus-broker",
    "dbus-daemon",
    "pipewire",
    "wireplumber",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    // Of one core, as top counts it.
    pub cpu: f32,
    pub memory: u64,
    // Only your own can be ended from here, and not what the session stands on.
    pub yours: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Machine {
    pub cpu: VecDeque<f32>,
    pub memory: VecDeque<f32>,
    pub down: VecDeque<f32>,
    pub up: VecDeque<f32>,
    // Bytes.
    pub used: u64,
    pub total: u64,
    pub swap: (u64, u64),
    pub disk: Option<(u64, u64)>,
    pub temperature: Option<i16>,
    pub cores: usize,
    pub busiest: Vec<Process>,
    pub biggest: Vec<Process>,
}

fn push(line: &mut VecDeque<f32>, value: f32) {
    if line.len() == KEPT {
        line.pop_front();
    }
    line.push_back(value);
}

pub fn subscription(open: bool) -> Subscription<Message> {
    if open {
        Subscription::run(watch)
    } else {
        Subscription::none()
    }
}

struct Seen {
    cpu: (u64, u64),
    ticks: HashMap<u32, u64>,
    net: (u64, u64),
    at: Instant,
}

struct Raw {
    cpu: Option<(u64, u64)>,
    cores: usize,
    meminfo: String,
    net: Option<(u64, u64)>,
    disk: Option<(u64, u64)>,
    temperature: Option<i16>,
    // pid, name, CPU ticks so far, resident bytes, yours.
    processes: Vec<(u32, String, u64, u64, bool)>,
}

fn watch() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| {
        let mut machine = Machine::default();
        let mut seen: Option<Seen> = None;
        // The first reading only sets the baseline, so the second comes soon after.
        let mut wait = Duration::from_millis(400);
        loop {
            let Ok(raw) = tokio::task::spawn_blocking(read).await else {
                return;
            };
            seen = Some(take(&mut machine, seen, raw));
            if !machine.cpu.is_empty()
                && output
                    .send(Message::Machine(Box::new(machine.clone())))
                    .await
                    .is_err()
            {
                return;
            }
            tokio::time::sleep(wait).await;
            wait = EVERY;
        }
    })
}

fn read() -> Raw {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    // SAFETY: getuid can't fail and touches nothing.
    let me = unsafe { libc::getuid() };
    // SAFETY: sysconf only reads a constant.
    let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).unwrap_or(4096);
    let processes = std::fs::read_dir("/proc")
        .map(|dir| {
            dir.flatten()
                .filter_map(|entry| {
                    let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
                    let text = std::fs::read_to_string(entry.path().join("stat")).ok()?;
                    let (name, ticks, pages) = process(&text)?;
                    let owner = std::os::unix::fs::MetadataExt::uid(&entry.metadata().ok()?);
                    let yours = owner == me && !HELD.contains(&name.as_str());
                    // Kernel threads hold no memory of their own.
                    (pages > 0).then_some((pid, name, ticks, pages * page, yours))
                })
                .collect()
        })
        .unwrap_or_default();
    Raw {
        cpu: stats::cpu_times(&stat),
        cores: stat
            .lines()
            .filter(|l| l.starts_with("cpu") && !l.starts_with("cpu "))
            .count()
            .max(1),
        meminfo: std::fs::read_to_string("/proc/meminfo").unwrap_or_default(),
        net: std::fs::read_to_string("/proc/net/dev")
            .ok()
            .map(|text| network(&text)),
        disk: disk("/"),
        temperature: stats::temperature(),
        processes,
    }
}

// The new reading folded into what's shown, against the one before it.
fn take(machine: &mut Machine, seen: Option<Seen>, raw: Raw) -> Seen {
    let now = Instant::now();
    let kb = |name| stats::kb(&raw.meminfo, name).unwrap_or(0) * 1024;
    machine.total = kb("MemTotal:");
    machine.used = machine.total.saturating_sub(kb("MemAvailable:"));
    machine.swap = (
        kb("SwapTotal:").saturating_sub(kb("SwapFree:")),
        kb("SwapTotal:"),
    );
    machine.disk = raw.disk;
    machine.temperature = raw.temperature;
    machine.cores = raw.cores;
    let cpu = raw.cpu.unwrap_or_default();
    let net = raw.net.unwrap_or_default();
    let ticks: HashMap<u32, u64> = raw.processes.iter().map(|p| (p.0, p.2)).collect();
    if let Some(before) = seen {
        let spent = cpu.1.saturating_sub(before.cpu.1).max(1) as f32;
        push(
            &mut machine.cpu,
            cpu.0.saturating_sub(before.cpu.0) as f32 / spent * 100.0,
        );
        push(
            &mut machine.memory,
            machine.used as f32 / machine.total.max(1) as f32 * 100.0,
        );
        let seconds = now.duration_since(before.at).as_secs_f32().max(0.1);
        push(
            &mut machine.down,
            net.0.saturating_sub(before.net.0) as f32 / seconds,
        );
        push(
            &mut machine.up,
            net.1.saturating_sub(before.net.1) as f32 / seconds,
        );
        // Each core is a hundred, as top has it.
        let core = spent / raw.cores as f32;
        let mut all: Vec<Process> = raw
            .processes
            .into_iter()
            .map(|(pid, name, used, memory, yours)| {
                let was = before.ticks.get(&pid).copied().unwrap_or(used);
                Process {
                    pid,
                    name,
                    cpu: used.saturating_sub(was) as f32 / core * 100.0,
                    memory,
                    yours,
                }
            })
            .collect();
        all.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)));
        machine.busiest = all.iter().take(LISTED).cloned().collect();
        all.sort_by_key(|p| std::cmp::Reverse(p.memory));
        all.truncate(LISTED);
        machine.biggest = all;
    }
    Seen {
        cpu,
        ticks,
        net,
        at: now,
    }
}

// A process's name, CPU ticks so far and resident pages, from its /proc stat line. The name sits
// in parentheses and may hold spaces or parentheses itself, so the fields are counted after the
// last one.
fn process(stat: &str) -> Option<(String, u64, u64)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_owned();
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    let field = |n: usize| rest.get(n - 3)?.parse::<u64>().ok();
    Some((name, field(14)? + field(15)?, field(24)?))
}

// Bytes in and out over every interface but the loopback.
fn network(dev: &str) -> (u64, u64) {
    dev.lines()
        .skip(2)
        .filter_map(|line| {
            let (name, numbers) = line.split_once(':')?;
            if name.trim() == "lo" {
                return None;
            }
            let numbers: Vec<u64> = numbers
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            Some((*numbers.first()?, *numbers.get(8)?))
        })
        .fold((0, 0), |(down, up), (d, u)| (down + d, up + u))
}

// Used and total bytes of the filesystem holding `path`. statvfs's field types differ by platform.
#[allow(clippy::unnecessary_cast)]
fn disk(path: &str) -> Option<(u64, u64)> {
    let path = std::ffi::CString::new(path).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: a valid C string and room for the answer; read only when it says it filled it.
    if unsafe { libc::statvfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so it wrote the whole struct.
    let info = unsafe { info.assume_init() };
    let block = info.f_frsize as u64;
    let total = info.f_blocks as u64 * block;
    let free = info.f_bavail as u64 * block;
    Some((total.saturating_sub(free), total))
}

// Asked to stop, the way closing it would; what ignores that is for a terminal.
pub fn end(pid: u32) -> Result<(), String> {
    let pid = libc::pid_t::try_from(pid).map_err(|e| e.to_string())?;
    // SAFETY: kill only sends a signal; a pid that's gone just fails.
    if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

// 1.4 GB, 380 MB, 12 KB.
pub fn bytes(n: f32) -> String {
    match n {
        n if n >= 1e9 => format!("{:.1} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.0} MB", n / 1e6),
        n => format!("{:.0} KB", n / 1e3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_name_with_spaces_and_parentheses_reads_whole() {
        let stat = "4242 (Web Content (x)) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0 20 0 30 0 100 2000000 1234 18446744073709551615";
        assert_eq!(process(stat), Some(("Web Content (x)".into(), 300, 1234)));
        assert_eq!(process("garbage"), None);
    }

    #[test]
    fn the_network_counts_everything_but_the_loopback() {
        let dev = "Inter-|   Receive\n face |bytes\n    lo: 900 1 0 0 0 0 0 0 900 1 0 0 0 0 0 0\n  eth0: 1000 5 0 0 0 0 0 0 200 3 0 0 0 0 0 0\n wlan0: 50 1 0 0 0 0 0 0 7 1 0 0 0 0 0 0\n";
        assert_eq!(network(dev), (1050, 207));
    }

    #[test]
    fn two_readings_make_a_point_and_rank_the_processes() {
        let raw = |busy, total, ticks: [u64; 2]| Raw {
            cpu: Some((busy, total)),
            cores: 4,
            meminfo: "MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n"
                .into(),
            net: Some((busy * 10, 0)),
            disk: None,
            temperature: None,
            processes: vec![
                (1, "idle".into(), ticks[0], 9_000_000, false),
                (2, "busy".into(), ticks[1], 1_000, true),
            ],
        };
        let mut machine = Machine::default();
        let seen = take(&mut machine, None, raw(100, 1000, [5, 10]));
        assert!(machine.cpu.is_empty(), "nothing to compare the first with");
        take(&mut machine, Some(seen), raw(300, 1400, [5, 110]));
        assert_eq!(machine.cpu.back(), Some(&50.0));
        assert_eq!(machine.memory.back(), Some(&75.0));
        assert_eq!(machine.busiest[0].name, "busy");
        assert_eq!(machine.busiest[0].cpu, 100.0, "a whole core of four");
        assert_eq!(machine.biggest[0].name, "idle");
    }

    #[test]
    fn sizes_read_short() {
        assert_eq!(bytes(1.4e9), "1.4 GB");
        assert_eq!(bytes(380e6), "380 MB");
        assert_eq!(bytes(12e3), "12 KB");
    }
}

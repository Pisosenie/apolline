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
    pub gpu: Option<u8>,
    pub temperature: Option<i16>,
}

pub fn subscription(shown: apolline_config::Stats) -> Subscription<Message> {
    match (shown.any(), shown.gpu) {
        (false, _) => Subscription::none(),
        (true, false) => Subscription::run(|| watch(false)),
        (true, true) => Subscription::run(|| watch(true)),
    }
}

// Sends only what changed, so a quiet machine doesn't repaint the bar every few seconds.
fn watch(with_gpu: bool) -> impl Stream<Item = Message> {
    iced::stream::channel(1, async move |mut output| {
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
                let gpu = if with_gpu { gpu() } else { None };
                (times, memory, gpu, temperature())
            })
            .await;
            if let Ok((times, memory, gpu, temperature)) = read {
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
                    gpu,
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
pub fn cpu_times(stat: &str) -> Option<(u64, u64)> {
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

// A /proc/meminfo line's kilobytes.
pub fn kb(meminfo: &str, name: &str) -> Option<u64> {
    meminfo
        .lines()
        .find(|line| line.starts_with(name))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

// The share of memory in use, counting what the kernel could hand back as free.
fn memory(meminfo: &str) -> Option<u8> {
    let total = kb(meminfo, "MemTotal:")?;
    let available = kb(meminfo, "MemAvailable:")?;
    (total > 0).then(|| ((total.saturating_sub(available)) * 100 / total) as u8)
}

// The busiest GPU's load, in steps of 5. amdgpu tells it in sysfs, NVIDIA's driver through NVML;
// a GPU that's asleep is left asleep.
pub fn gpu() -> Option<u8> {
    let mut busiest = None;
    let mut nvidia = false;
    for entry in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let device = entry.path().join("device");
        let read = |file: &str| std::fs::read_to_string(device.join(file)).ok();
        if read("power/runtime_status").is_some_and(|status| status.trim() != "active") {
            continue;
        }
        if read("vendor").is_some_and(|vendor| vendor.trim() == "0x10de") {
            nvidia = true;
        } else if let Some(busy) =
            read("gpu_busy_percent").and_then(|v| v.trim().parse::<u8>().ok())
        {
            busiest = busiest.max(Some(busy));
        }
    }
    if nvidia {
        busiest = busiest.max(nvml::busy());
    }
    busiest.map(|busy| ((f32::from(busy.min(100)) / 5.0).round() * 5.0) as u8)
}

mod nvml {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    use libloading::Library;

    #[repr(C)]
    struct Utilization {
        gpu: u32,
        memory: u32,
    }

    // Loaded once. Each reading starts NVML and shuts it down again, since NVML holding the
    // device open keeps a laptop's GPU from sleeping.
    static NVML: OnceLock<Option<Library>> = OnceLock::new();

    pub fn busy() -> Option<u8> {
        let library = NVML
            .get_or_init(|| {
                // SAFETY: loading the driver's library runs only its own initializers.
                unsafe { Library::new("libnvidia-ml.so.1").ok() }
            })
            .as_ref()?;
        // SAFETY: NVML's own signatures; nothing of it is used outside its init and shutdown.
        unsafe {
            let init = library
                .get::<unsafe extern "C" fn() -> i32>(b"nvmlInit_v2")
                .ok()?;
            let shutdown = library
                .get::<unsafe extern "C" fn() -> i32>(b"nvmlShutdown")
                .ok()?;
            if init() != 0 {
                return None;
            }
            let busiest = read(library);
            shutdown();
            busiest
        }
    }

    /// # Safety
    /// NVML has to be started, and the handles it gives aren't kept past its shutdown.
    unsafe fn read(library: &Library) -> Option<u8> {
        unsafe {
            let count = library
                .get::<unsafe extern "C" fn(*mut u32) -> i32>(b"nvmlDeviceGetCount_v2")
                .ok()?;
            let handle = library
                .get::<unsafe extern "C" fn(u32, *mut *mut c_void) -> i32>(
                    b"nvmlDeviceGetHandleByIndex_v2",
                )
                .ok()?;
            let rates = library
                .get::<unsafe extern "C" fn(*mut c_void, *mut Utilization) -> i32>(
                    b"nvmlDeviceGetUtilizationRates",
                )
                .ok()?;
            let mut devices = 0;
            if count(&mut devices) != 0 {
                return None;
            }
            (0..devices)
                .filter_map(|index| {
                    let mut device = std::ptr::null_mut();
                    if handle(index, &mut device) != 0 {
                        return None;
                    }
                    let mut used = Utilization { gpu: 0, memory: 0 };
                    (rates(device, &mut used) == 0).then(|| used.gpu.min(100) as u8)
                })
                .max()
        }
    }
}

// The CPU's own sensor when there's one, else the first thermal zone that says it's the CPU.
pub fn temperature() -> Option<i16> {
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

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

// Past this the older half goes, so the log never grows without end.
const KEPT: u64 = 64 * 1024;

pub fn file() -> Option<PathBuf> {
    apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")
        .map(|state| state.join("apolline/crashes.log"))
}

// A panic in any of Apolline's processes lands in the log as well as on stderr.
pub fn catch_panics(who: String) {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record(&format!("{who} {info}"));
        before(info);
    }));
}

pub fn record(what: &str) {
    if let Some(path) = file()
        && let Err(err) = append(&path, &entry(what, Local::now()))
    {
        eprintln!("apolline: {}: {err}", path.display());
    }
}

fn entry(what: &str, now: DateTime<Local>) -> String {
    format!(
        "{} {} {}\n",
        now.format("%Y-%m-%d %H:%M:%S"),
        env!("CARGO_PKG_VERSION"),
        what.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn append(path: &Path, entry: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > KEPT) {
        let text = std::fs::read_to_string(path)?;
        std::fs::write(path, newer_half(&text))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(entry.as_bytes())
}

fn newer_half(text: &str) -> &str {
    let half = text.len() / 2;
    text.as_bytes()[half..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or("", |at| &text[half + at + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_is_one_line_with_the_time_and_version() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 9)
            .unwrap()
            .and_hms_opt(14, 3, 12)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();
        let line = entry(
            "shell panicked at src/bar.rs:10:5:\nindex out of bounds",
            now,
        );
        assert_eq!(
            line,
            format!(
                "2026-11-09 14:03:12 {} shell panicked at src/bar.rs:10:5: index out of bounds\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn a_full_log_keeps_its_newer_half() {
        let dir = std::env::temp_dir().join(format!("apolline-crashes-{}", std::process::id()));
        let path = dir.join("crashes.log");
        let old = "2026-11-01 10:00:00 0.3.0 shell was killed by SIGSEGV\n";
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, old.repeat(KEPT as usize / old.len() + 1)).unwrap();
        append(&path, "2026-11-09 14:03:12 0.3.0 shell: no Wayland\n").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.len() < KEPT as usize * 2 / 3);
        assert!(text.starts_with("2026-11-01"));
        assert!(text.ends_with("0.3.0 shell: no Wayland\n"));
        assert_eq!(newer_half("ü\nü"), "ü");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

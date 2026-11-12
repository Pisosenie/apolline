use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const ISSUES: &str = "https://github.com/Pisosenie/apolline/issues/new";
const CRASHES: usize = 20;
const LOG: usize = 40;

// `apolline report`: what an issue needs in one file. Versions, the machine and its screens, what
// doctor says, the latest crashes, the shell's log and the config, with private things taken out.
pub fn main(args: &[String]) -> ExitCode {
    crate::i18n::choose(Some("en"));
    let text = gather();
    if args.first().is_some_and(|arg| arg == "-") {
        print!("{text}");
        return ExitCode::SUCCESS;
    }
    let Some(path) = file() else {
        eprintln!("apolline: no home folder to write the report to");
        return ExitCode::FAILURE;
    };
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, &text));
    match written {
        Ok(()) => {
            println!("{}", path.display());
            println!("Read it through, then attach it to an issue: {ISSUES}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("apolline: {}: {err}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn file() -> Option<PathBuf> {
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")
        .map(|state| state.join(format!("apolline/report-{stamp}.md")))
}

fn gather() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    let config = std::fs::read_to_string(apolline_config::default_path())
        .unwrap_or_else(|_| "# no config.toml, the defaults\n".into());
    let crashes = crate::crashes::file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| tail(&text, CRASHES))
        .unwrap_or_default();
    let sections = [
        ("Machine", machine()),
        ("Screens", screens()),
        ("Doctor", doctor()),
        ("Crashes", fenced("text", &crashes)),
        ("Shell log", fenced("text", &shell_log())),
        ("Config", fenced("toml", &config)),
    ];
    let mut text = String::from("# Apolline report\n");
    for (title, body) in sections {
        let _ = write!(text, "\n## {title}\n\n{}\n", body.trim_end());
    }
    private(&text, &home, &user)
}

fn machine() -> String {
    let os = read("/etc/os-release")
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_owned())
        .unwrap_or_else(|| std::env::consts::OS.into());
    let lines = [
        format!("Apolline {}", env!("CARGO_PKG_VERSION")),
        run("niri", &["--version"]).unwrap_or_else(|| "niri not found".into()),
        format!("{os}, kernel {}", read("/proc/sys/kernel/osrelease").trim()),
        format!(
            "session {}",
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into())
        ),
    ];
    lines
        .into_iter()
        .chain(gpus())
        .chain(
            read("/proc/driver/nvidia/version")
                .lines()
                .next()
                .map(str::to_owned),
        )
        .map(|line| format!("- {line}\n"))
        .collect()
}

// Each card's vendor, device and the kernel driver bound to it.
fn gpus() -> Vec<String> {
    let Ok(cards) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut found: Vec<String> = cards
        .flatten()
        .filter(|card| {
            let name = card.file_name().to_string_lossy().into_owned();
            name.starts_with("card") && !name.contains('-')
        })
        .map(|card| {
            let device = card.path().join("device");
            let id = |name: &str| {
                read(device.join(name))
                    .trim()
                    .trim_start_matches("0x")
                    .to_owned()
            };
            let driver = std::fs::read_link(device.join("driver"))
                .ok()
                .and_then(|link| link.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "no driver".into());
            format!(
                "{}: {} {}:{} ({driver})",
                card.file_name().to_string_lossy(),
                vendor(&id("vendor")),
                id("vendor"),
                id("device")
            )
        })
        .collect();
    found.sort();
    found
}

fn vendor(id: &str) -> &'static str {
    match id {
        "1002" => "AMD",
        "10de" => "NVIDIA",
        "8086" => "Intel",
        _ => "GPU",
    }
}

fn screens() -> String {
    let outputs = crate::niri::outputs();
    if outputs.is_empty() {
        return "niri isn't running here\n".into();
    }
    outputs
        .iter()
        .map(|output| {
            let mode = output
                .current_mode
                .and_then(|i| output.modes.get(i))
                .map_or_else(
                    || "off".to_owned(),
                    |m| {
                        format!(
                            "{}×{} @ {:.2} Hz",
                            m.width,
                            m.height,
                            f64::from(m.refresh_rate) / 1000.0
                        )
                    },
                );
            let scale = output
                .logical
                .as_ref()
                .map_or_else(String::new, |l| format!(", scale {}", l.scale));
            let vrr = match (output.vrr_supported, output.vrr_enabled) {
                (true, true) => ", VRR on",
                (true, false) => ", VRR off",
                (false, _) => "",
            };
            format!(
                "- {} {} {}: {mode}{scale}{vrr}\n",
                output.name, output.make, output.model
            )
        })
        .collect()
}

fn doctor() -> String {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return String::new();
    };
    runtime
        .block_on(crate::doctor::gather())
        .iter()
        .map(|line| {
            let mark = match line.state {
                crate::doctor::State::Good => "ok",
                crate::doctor::State::Optional => "--",
                crate::doctor::State::Bad => "NO",
            };
            format!("- {mark} {}: {}\n", line.what, line.said)
        })
        .collect()
}

// The shell's lines from this boot, where niri runs as a systemd unit and keeps its children's
// output.
fn shell_log() -> String {
    let journal = run(
        "journalctl",
        &[
            "--user",
            "-b",
            "-u",
            "niri",
            "-o",
            "cat",
            "--no-pager",
            "-n",
            "2000",
        ],
    )
    .unwrap_or_default();
    let ours: Vec<&str> = journal
        .lines()
        .filter(|line| line.contains("apolline") || line.contains("panicked"))
        .collect();
    tail(&ours.join("\n"), LOG)
}

fn fenced(language: &str, text: &str) -> String {
    if text.trim().is_empty() {
        return "nothing\n".into();
    }
    format!("```{language}\n{}\n```\n", text.trim_end())
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

// Keys whose values say who or where someone is.
const PRIVATE_KEYS: [&str; 5] = ["weather", "user", "password", "token", "secret"];

// Links keep only their host, since calendar links carry their secret in the path; the home folder
// and the user name go too.
fn private(text: &str, home: &str, user: &str) -> String {
    let mut text: String = text
        .lines()
        .map(|line| hide_value(line).unwrap_or_else(|| hide_links(line)))
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    if home.len() > 1 {
        text = text.replace(home, "~");
    }
    if user.len() >= 3 {
        text = text.replace(user, "<user>");
    }
    text
}

fn hide_value(line: &str) -> Option<String> {
    let (key, _) = line.split_once('=')?;
    let name = key.trim().to_lowercase();
    PRIVATE_KEYS
        .iter()
        .any(|private| name == *private || name.ends_with(&format!("_{private}")))
        .then(|| format!("{key}= \"…\""))
}

fn hide_links(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at + 3);
        out.push_str(before);
        let end = after
            .find(|c: char| c.is_whitespace() || "\"'<>,]".contains(c))
            .unwrap_or(after.len());
        let link = &after[..end];
        let host = link
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .rsplit('@')
            .next()
            .unwrap_or_default();
        out.push_str(host);
        if host.len() < link.len() {
            out.push_str("/…");
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_things_are_taken_out() {
        let config = "\
[bar]
weather = \"Novosibirsk\"
calendars = [\"https://calendar.google.com/calendar/ical/me%40x/private-abc123/basic.ics\", { caldav = \"https://dav.example.org/cal/\", user = \"danil\" }]
[desktop]
wallpaper = \"/home/danil/Pictures/wall.png\"
font = \"Inter\"
# see https://github.com/Pisosenie/apolline
";
        let text = private(config, "/home/danil", "danil");
        assert!(text.contains("weather = \"…\""));
        assert!(!text.contains("Novosibirsk"));
        assert!(text.contains("calendars = [\"https://calendar.google.com/…\", { caldav = \"https://dav.example.org/…\", user = \"<user>\" }]"));
        assert!(!text.contains("private-abc123"));
        assert!(text.contains("wallpaper = \"~/Pictures/wall.png\""));
        assert!(text.contains("font = \"Inter\""));
        assert!(text.contains("# see https://github.com/…"));
        assert!(!text.contains("danil"));
    }

    #[test]
    fn a_link_keeps_its_host_only() {
        assert_eq!(
            hide_links("at https://u:pw@host.org:8443/a?b=c and http://x.y"),
            "at https://host.org:8443/… and http://x.y"
        );
        assert_eq!(hide_links("no links"), "no links");
        assert_eq!(hide_value("my_token=abc"), Some("my_token= \"…\"".into()));
        assert_eq!(hide_value("keep_awake = true"), None);
    }

    #[test]
    fn sections_are_fenced_and_tailed() {
        assert_eq!(fenced("text", " \n"), "nothing\n");
        assert_eq!(fenced("toml", "a = 1\n"), "```toml\na = 1\n```\n");
        assert_eq!(tail("1\n2\n3\n4", 2), "3\n4");
        assert_eq!(tail("1", 5), "1");
    }
}

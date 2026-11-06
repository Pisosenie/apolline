use std::path::Path;
use std::process::ExitCode;

use crate::i18n::{fill, t};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Good,
    // Something works without it, it just does less.
    Optional,
    Bad,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub state: State,
    pub what: &'static str,
    pub said: String,
}

fn line(state: State, what: &'static str, said: impl Into<String>) -> Line {
    Line {
        state,
        what,
        said: said.into(),
    }
}

// What works on this machine, and what to install for the rest.
pub async fn gather() -> Vec<Line> {
    let mut lines = vec![niri(), setup(), config(), pam()];
    lines.extend(programs());
    lines.push(sounds());
    lines.push(terminal());
    lines.push(files());
    lines.extend(services().await);
    lines
}

// `apolline doctor`, the same list in a terminal.
pub fn main() -> ExitCode {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return ExitCode::FAILURE;
    };
    let lines = runtime.block_on(gather());
    let width = lines
        .iter()
        .map(|l| t(l.what).chars().count())
        .max()
        .unwrap_or(0);
    for l in &lines {
        let mark = match l.state {
            State::Good => "✓",
            State::Optional => "·",
            State::Bad => "✗",
        };
        let what = t(l.what);
        let pad = width - what.chars().count();
        println!("{mark} {what}{}  {}", " ".repeat(pad), l.said);
    }
    if lines.iter().any(|l| l.state == State::Bad) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn niri() -> Line {
    let version = std::process::Command::new("niri")
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());
    match (version, crate::niri::connected()) {
        (None, _) => line(State::Bad, "niri", t("isn't installed: pacman -S niri")),
        (Some(version), true) => line(State::Good, "niri", version),
        (Some(version), false) => line(
            State::Optional,
            "niri",
            fill("{version}, not running here", &[("version", &version)]),
        ),
    }
}

fn setup() -> Line {
    let niri = crate::fragment::niri_config();
    let include = format!("include \"{}\"", crate::fragment::path().display());
    let included =
        std::fs::read_to_string(&niri).is_ok_and(|text| text.lines().any(|l| l.trim() == include));
    if included {
        line(State::Good, "Setup", t("niri's config includes Apolline"))
    } else {
        line(State::Bad, "Setup", t("not done yet: run  apolline setup"))
    }
}

fn config() -> Line {
    let path = apolline_config::default_path();
    let (_, problems) = apolline_config::Config::load(&path);
    match problems.len() {
        0 if !path.exists() => line(
            State::Optional,
            "Config",
            t("defaults, there's no config.toml yet"),
        ),
        0 => line(State::Good, "Config", path.display().to_string()),
        n => line(
            State::Bad,
            "Config",
            crate::i18n::count(
                n,
                "1 problem: apolline check shows it",
                "{n} problems: apolline check shows them",
            ),
        ),
    }
}

fn terminal() -> Line {
    let config = apolline_config::config_home();
    let (chosen, _) = apolline_config::Config::load(&apolline_config::default_path());
    let program = chosen
        .terminal
        .split_whitespace()
        .next()
        .unwrap_or("vif")
        .to_owned();
    if !crate::launcher::on_path(&program) {
        return line(
            State::Bad,
            "Terminal",
            fill("{program} isn't installed", &[("program", &program)]),
        );
    }
    if program == "vif" && !crate::terminal::vif_follows(&config) {
        return line(
            State::Optional,
            "Terminal",
            fill(
                "vif, in its own colors; [colors] from = \"{file}\" in vif.toml puts it in Apolline's",
                &[("file", &crate::terminal::FROM)],
            ),
        );
    }
    line(State::Good, "Terminal", program)
}

// What opens a folder: the launcher's files and every app's "show in folder" go through it.
fn files() -> Line {
    let chosen = std::process::Command::new("xdg-mime")
        .args(["query", "default", "inode/directory"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_default();
    match chosen.strip_suffix(".desktop") {
        Some(app) if !app.is_empty() => line(State::Good, "Files", app.to_owned()),
        _ => line(
            State::Optional,
            "Files",
            t("nothing opens folders: pacman -S cosmic-files"),
        ),
    }
}

fn sounds() -> Line {
    let found = crate::sound::pick(
        &apolline_config::Sound::Theme,
        &crate::notify::Heard::Nothing,
    );
    match found {
        Some(file) => line(State::Good, "Sound theme", file.display().to_string()),
        None => line(
            State::Optional,
            "Sound theme",
            t("no message sound: pacman -S sound-theme-freedesktop"),
        ),
    }
}

fn pam() -> Line {
    if Path::new("/etc/pam.d/apolline").is_file() {
        line(State::Good, "Lock screen", "/etc/pam.d/apolline")
    } else {
        line(
            State::Optional,
            "Lock screen",
            t("checks passwords through PAM's login, /etc/pam.d/apolline isn't there"),
        )
    }
}

// What each program is for, and the package that has it.
const PROGRAMS: [(&str, &str, &str, bool); 9] = [
    ("Volume", "wpctl", "wireplumber", true),
    ("Volume on the bar", "pactl", "libpulse", false),
    ("Brightness", "brightnessctl", "brightnessctl", false),
    ("Screen recording", "wf-recorder", "wf-recorder", false),
    (
        "Game replays",
        "gpu-screen-recorder",
        "gpu-screen-recorder",
        false,
    ),
    ("Login screen", "cage", "cage greetd", false),
    ("Login screen look", "pkexec", "polkit", false),
    ("Flatpak apps", "flatpak", "flatpak", false),
    ("Notification sound", "pw-play", "pipewire", false),
];

fn programs() -> Vec<Line> {
    PROGRAMS
        .iter()
        .map(|&(what, program, package, needed)| {
            if crate::launcher::on_path(program) {
                line(State::Good, what, program)
            } else {
                line(
                    if needed { State::Bad } else { State::Optional },
                    what,
                    fill(
                        "{program} is missing: pacman -S {package}",
                        &[("program", &program), ("package", &package)],
                    ),
                )
            }
        })
        .collect()
}

// System services quick settings talks to, and the package each comes from.
const SERVICES: [(&str, &str, &str); 6] = [
    ("Wi-Fi", "org.freedesktop.NetworkManager", "networkmanager"),
    ("Bluetooth", "org.bluez", "bluez"),
    ("Battery", "org.freedesktop.UPower", "upower"),
    (
        "Power profiles",
        "org.freedesktop.UPower.PowerProfiles",
        "power-profiles-daemon",
    ),
    (
        "Admin password dialog",
        "org.freedesktop.PolicyKit1",
        "polkit",
    ),
    ("Fingerprint", "net.reactivated.Fprint", "fprintd"),
];

async fn services() -> Vec<Line> {
    let system = zbus::Connection::system().await.ok();
    let session = zbus::Connection::session().await.ok();
    let mut lines = Vec::new();
    for (what, name, package) in SERVICES {
        let here = match &system {
            Some(bus) => there(bus, name).await,
            None => false,
        };
        lines.push(if here {
            line(State::Good, what, name)
        } else {
            line(
                State::Optional,
                what,
                fill(
                    "nothing answers: pacman -S {package}, then enable its service",
                    &[("package", &package)],
                ),
            )
        });
    }
    if let Some(bus) = &session {
        lines.push(match owner(bus, "org.freedesktop.Notifications").await {
            Some(who) if who == "apolline" => {
                line(State::Good, "Notifications", t("Apolline shows them"))
            }
            Some(who) => line(
                State::Optional,
                "Notifications",
                fill(
                    "{app} shows them; Apolline stays out of the way",
                    &[("app", &who)],
                ),
            ),
            None => line(
                State::Optional,
                "Notifications",
                t("nobody shows them until Apolline runs"),
            ),
        });
    }
    lines
}

async fn there(bus: &zbus::Connection, name: &str) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(bus).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(name) else {
        return false;
    };
    if dbus.name_has_owner(name.clone()).await.unwrap_or(false) {
        return true;
    }
    // Started on demand, like most of these.
    dbus.list_activatable_names()
        .await
        .is_ok_and(|names| names.iter().any(|n| n.as_str() == name.as_str()))
}

// The program holding a name, by its process.
async fn owner(bus: &zbus::Connection, name: &str) -> Option<String> {
    let dbus = zbus::fdo::DBusProxy::new(bus).await.ok()?;
    let name = zbus::names::BusName::try_from(name).ok()?;
    let pid = dbus.get_connection_unix_process_id(name).await.ok()?;
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim().to_owned())
}

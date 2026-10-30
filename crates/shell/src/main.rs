mod apps;
mod bar;
mod calc;
mod clipboard;
mod clips;
mod control;
mod controls;
mod fonts;
mod fragment;
mod icon;
mod idle;
mod keymap;
mod keys;
mod launcher;
mod lock;
mod logind;
mod media;
mod niri;
mod notify;
mod osd;
mod paint;
mod panel;
mod pick;
mod polkit;
mod popup;
mod power;
mod preview;
mod profile;
mod settings;
mod shell;
mod shot;
mod status;
mod style;
mod switcher;
mod themes;
mod wallpaper;
mod windows;

#[cfg(target_os = "linux")]
mod layers;

#[cfg(not(target_os = "linux"))]
mod layers {
    pub fn run(_: Option<String>) -> Result<(), String> {
        Err("layer surfaces need a Wayland session on Linux; use --preview".into())
    }
}

use std::path::PathBuf;
use std::process::ExitCode;

use apolline_config::{Config, Editor, slug};

const USAGE: &str = "usage: apolline [--mode NAME] [--preview] [--open launcher|panel|switch|clipboard|keys|polkit|osd] [--query TEXT] [--screenshot FILE]
       apolline mode NAME|next|none
       apolline switch | launcher | panel | clipboard | keys
       apolline volume up|down|mute | mic mute | brightness up|down
       apolline theme [list|next|NAME]
       apolline pick [--prompt TEXT] < lines
       apolline setup
       apolline settings [--page NAME]
       apolline lock [--preview] [--mode NAME] [--screenshot FILE]
       apolline check [FILE]";

#[derive(Default)]
struct Options {
    mode: Option<String>,
    preview: bool,
    screenshot: Option<PathBuf>,
    open: Vec<shell::Message>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::env::var_os("ICED_BACKEND").is_none() && args.first().is_some_and(|a| a == "lock") {
        // The lock screen must come up even when the GPU driver doesn't; it is opaque,
        // so the CPU renderer's lack of transparency on Wayland doesn't show.
        // SAFETY: nothing else runs yet, so no other thread reads the environment.
        unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
    }
    match args.first().map(String::as_str) {
        Some("check") => return check(args.get(1).map(PathBuf::from)),
        Some("mode" | "switch" | "launcher" | "panel" | "clipboard" | "keys") => {
            return command(&args.join(" "));
        }
        Some("volume" | "mic" | "brightness") => return media_key(&args.join(" ")),
        Some("theme") => return theme(args.get(1).map(String::as_str)),
        Some("pick") => return pick::main(&args[1..]),
        Some("setup") => return setup(),
        Some("settings") => return open_settings(&args[1..]),
        Some("lock") => return lock_screen(&args[1..]),
        _ => {}
    }

    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("apolline: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<(), String> {
    let layered = !options.preview
        && options.screenshot.is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_some();
    if layered {
        layers::run(options.mode)
    } else {
        preview::run(options.mode, options.screenshot, options.open).map_err(|err| err.to_string())
    }
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                options.mode = Some(args.next().ok_or("--mode needs a name")?.clone());
            }
            "--preview" => options.preview = true,
            "--open" => {
                let overlay = |overlay| shell::Message::Overlay(Some(overlay));
                options.open.push(match args.next().map(String::as_str) {
                    Some("launcher") => overlay(shell::Overlay::Launcher),
                    Some("panel") => overlay(shell::Overlay::Panel),
                    Some("switch") => overlay(shell::Overlay::Switcher),
                    Some("clipboard") => overlay(shell::Overlay::Clipboard),
                    Some("keys") => overlay(shell::Overlay::Keys),
                    Some("polkit") => shell::Message::Polkit(polkit::sample()),
                    Some("osd") => {
                        shell::Message::Control(control::Command::Nudge(controls::Nudge::VolumeUp))
                    }
                    _ => {
                        return Err(
                            "--open needs launcher, panel, switch, clipboard, keys or osd".into(),
                        );
                    }
                })
            }
            "--query" => {
                let query = args.next().ok_or("--query needs some text")?.clone();
                options
                    .open
                    .push(shell::Message::Overlay(Some(shell::Overlay::Launcher)));
                options.open.push(shell::Message::Query(query));
            }
            "--screenshot" => {
                options.screenshot = Some(args.next().ok_or("--screenshot needs a file")?.into())
            }
            other => return Err(format!("unknown argument “{other}”")),
        }
    }
    Ok(options)
}

fn command(line: &str) -> ExitCode {
    match control::Command::parse(line).and_then(|_| control::send(line)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn theme(wanted: Option<&str>) -> ExitCode {
    let path = apolline_config::default_path();
    let (config, _) = Config::load(&path);
    let themes = themes::all();
    let current = config
        .desktop
        .theme
        .as_deref()
        .and_then(|name| themes.iter().position(|(t, _)| slug(&t.name) == slug(name)));
    let index = match wanted {
        None | Some("list") => {
            for (i, (theme, _)) in themes.iter().enumerate() {
                let mark = if current == Some(i) { '*' } else { ' ' };
                println!("{mark} {}", theme.name);
            }
            return ExitCode::SUCCESS;
        }
        Some("next") => current.map_or(0, |i| (i + 1) % themes.len()),
        Some(name) => match themes.iter().position(|(t, _)| slug(&t.name) == slug(name)) {
            Some(i) => i,
            None => {
                eprintln!("apolline: no theme “{name}”; `apolline theme list` shows them");
                return ExitCode::from(2);
            }
        },
    };
    let theme = &themes[index].0;
    let applied = Editor::load(&path).and_then(|mut editor| {
        theme.apply(&mut editor);
        editor.save(&path).map_err(|err| err.to_string())
    });
    match applied {
        Ok(()) => {
            println!("{}", theme.name);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("apolline: {}: {err}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn media_key(line: &str) -> ExitCode {
    let nudge = match control::Command::parse(line) {
        Ok(control::Command::Nudge(nudge)) => nudge,
        _ => {
            eprintln!("apolline: unknown command “{line}”\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if control::send(line).is_ok() {
        return ExitCode::SUCCESS;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    match runtime.map(|rt| rt.block_on(controls::nudge(nudge))) {
        Ok(Some(_)) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn open_settings(args: &[String]) -> ExitCode {
    let mut screenshot = None;
    let mut page = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--screenshot", Some(path)) => screenshot = Some(PathBuf::from(path)),
            ("--page", Some(name)) => page = Some(name.clone()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match settings::run(screenshot, page) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn lock_screen(args: &[String]) -> ExitCode {
    let options = match parse(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("apolline: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match lock::run(options.preview, options.mode, options.screenshot) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn setup() -> ExitCode {
    let (config, _) = Config::load(&apolline_config::default_path());
    let look = fragment::Fragment {
        config: &config,
        mode: config.start(),
        outputs: &[],
    };
    match fragment::setup(&fragment::path(), &fragment::niri_config(), &look) {
        Ok(done) => {
            done.iter().for_each(|line| println!("{line}"));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn check(path: Option<PathBuf>) -> ExitCode {
    let path = path.unwrap_or_else(apolline_config::default_path);
    let (_, diagnostics) = Config::load(&path);
    if diagnostics.is_empty() {
        println!("{}: ok", path.display());
        return ExitCode::SUCCESS;
    }
    for diagnostic in &diagnostics {
        eprintln!("{}:{}: {diagnostic}", path.display(), diagnostic.line);
    }
    ExitCode::FAILURE
}

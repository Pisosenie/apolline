mod apps;
mod bar;
mod control;
mod controls;
mod fonts;
mod fragment;
mod icon;
mod idle;
mod keys;
mod launcher;
mod lock;
mod logind;
mod media;
mod niri;
mod notify;
mod panel;
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
mod wallpaper;

#[cfg(target_os = "linux")]
mod layers;

#[cfg(not(target_os = "linux"))]
mod layers {
    pub fn run(_: Option<apolline_theme::Mode>) -> Result<(), String> {
        Err("layer surfaces need a Wayland session on Linux; use --preview".into())
    }
}

use std::path::PathBuf;
use std::process::ExitCode;

use apolline_config::Config;
use apolline_theme::Mode;

const USAGE: &str = "usage: apolline [--mode work|play|evening] [--preview] [--open launcher|panel|switch] [--screenshot FILE]
       apolline mode work|play|evening|next
       apolline switch | launcher | panel
       apolline setup
       apolline settings [--page NAME]
       apolline lock [--preview] [--mode MODE] [--screenshot FILE]
       apolline check [FILE]";

#[derive(Default)]
struct Options {
    mode: Option<Mode>,
    preview: bool,
    screenshot: Option<PathBuf>,
    open: Option<shell::Overlay>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => return check(args.get(1).map(PathBuf::from)),
        Some("mode" | "switch" | "launcher" | "panel") => return command(&args.join(" ")),
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
                let value = args.next().ok_or("--mode needs a value")?;
                options.mode =
                    Some(Mode::from_key(value).ok_or_else(|| format!("unknown mode “{value}”"))?);
            }
            "--preview" => options.preview = true,
            "--open" => {
                options.open = Some(match args.next().map(String::as_str) {
                    Some("launcher") => shell::Overlay::Launcher,
                    Some("panel") => shell::Overlay::Panel,
                    Some("switch") => shell::Overlay::Switcher,
                    _ => return Err("--open needs launcher, panel or switch".into()),
                })
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
        mode: config.mode,
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

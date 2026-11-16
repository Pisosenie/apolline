// hypr2niri [CONFIG] [-o FILE]: a Hyprland config as a niri config.kdl. Without CONFIG it reads
// ~/.config/hypr/hyprland.lua, or hyprland.conf; the config goes to FILE or the screen, the report
// of what came over to the terminal.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use hypr2niri::niri::{self, Screens};

const USAGE: &str = "usage: hypr2niri [hyprland.conf | hyprland.lua] [-o config.kdl]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = None;
    let mut output = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-o" | "--output" => match rest.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => return usage(),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if input.is_none() && !arg.starts_with('-') => input = Some(PathBuf::from(arg)),
            _ => return usage(),
        }
    }
    let (path, lines, stopped) = match input {
        Some(path) if path.extension().is_some_and(|e| e == "lua") => {
            let (lines, stopped) = hypr2niri::lua::read(&path);
            (path, lines, stopped)
        }
        Some(path) => {
            let lines = hypr2niri::conf::read(&path);
            (path, lines, None)
        }
        None => hypr2niri::read(&config_home()),
    };
    if lines.is_empty() {
        eprintln!(
            "hypr2niri: nothing to read at {}{}",
            path.display(),
            stopped.map_or(String::new(), |why| format!(": {why}"))
        );
        return ExitCode::FAILURE;
    }
    let lua = path.extension().is_some_and(|e| e == "lua");
    let source = path.display().to_string();
    let converted = niri::convert(&lines, lua, &source, &screens());
    if let Some(why) = stopped {
        eprintln!("{source} stopped partway ({why}); this is what it set before that.\n");
    }
    eprintln!("Read as {}.\n", converted.era.said());
    eprint!(
        "{}",
        hypr2niri::report::report(
            &converted.notes,
            &[],
            ["Comes over", "niri does it already", "Stays behind"]
        )
    );
    match output {
        Some(file) => match std::fs::write(&file, &converted.kdl) {
            Ok(()) => {
                eprintln!(
                    "\nWrote {}. Check it with: niri validate -c {}",
                    file.display(),
                    file.display()
                );
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("hypr2niri: {}: {err}", file.display());
                ExitCode::FAILURE
            }
        },
        None => {
            print!("{}", converted.kdl);
            ExitCode::SUCCESS
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

// Under a running niri, the exact modes of each screen, so `mode` says them as niri wants.
fn screens() -> Screens {
    if std::env::var_os("NIRI_SOCKET").is_none() {
        return Screens::default();
    }
    let Ok(out) = Command::new("niri").args(["msg", "outputs"]).output() else {
        return Screens::default();
    };
    niri::screens(&String::from_utf8_lossy(&out.stdout))
}

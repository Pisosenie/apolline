use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use apolline_config::{Config, Keyboard, Mouse, Shortcut, Touchpad};
use apolline_theme::Mode;

const TEMPLATE: &str = include_str!("../../../assets/niri.kdl");
const PLAY: &str = r##"
animations {
    off
}

window-rule {
    match app-id=r#"^steam_app_"#
    match app-id=r#"^gamescope$"#
    match app-id=r#"\.exe$"#
    variable-refresh-rate true
}
"##;

pub struct Fragment<'a> {
    pub config: &'a Config,
    pub mode: Mode,
    pub outputs: &'a [String],
}

pub fn path() -> PathBuf {
    apolline_config::config_home()
        .join("apolline")
        .join("niri.kdl")
}

pub fn niri_config() -> PathBuf {
    std::env::var_os("NIRI_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            apolline_config::config_home()
                .join("niri")
                .join("config.kdl")
        })
}

pub fn render(fragment: &Fragment) -> String {
    let play = fragment.mode == Mode::Play;
    let layout = fragment.config.layout;
    let on = |yes: bool| if yes { "on" } else { "off" };
    let mut text = TEMPLATE.to_owned();
    text.push_str(&format!(
        r##"
layout {{
    gaps {gaps}
    background-color "transparent"

    focus-ring {{
        off
    }}

    border {{
        {border}
        width 1
        active-color "{accent}"
        inactive-color "#ffffff14"
        urgent-color "#ff6a3d"
    }}

    shadow {{
        {shadow}
        softness 40
        spread 0
        offset x=0 y=12
        color "#00000059"
    }}
}}

window-rule {{
    geometry-corner-radius {radius}
    clip-to-geometry true
}}

window-rule {{
    match app-id="^apolline-settings$"
    open-floating true
    default-column-width {{ fixed 1180; }}
    default-window-height {{ fixed 760; }}
}}
"##,
        gaps = layout.gaps,
        border = on(layout.borders),
        accent = fragment.config.palette(fragment.mode).accent.to_hex(),
        shadow = on(layout.shadows && !play),
        radius = layout.radius,
    ));
    let config = fragment.config;
    let binds: Vec<String> = Shortcut::ALL
        .into_iter()
        .map(|shortcut| {
            let (title, command) = action(shortcut);
            format!(
                "{} hotkey-overlay-title=\"{title}\" {{ spawn \"apolline\" \"{command}\"; }}",
                config.shortcuts.get(shortcut)
            )
        })
        .collect();
    text.push_str(&format!("\n{}", block("binds", &binds)));
    let devices: Vec<String> = [
        keyboard(&config.keyboard),
        config.touchpad.map(touchpad),
        config.mouse.map(mouse),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !devices.is_empty() {
        text.push_str(&format!("\n{}", block("input", &devices)));
    }
    for output in fragment.outputs {
        let name = output.replace('\\', "\\\\").replace('"', "\\\"");
        text.push_str(&format!(
            "\noutput \"{name}\" {{\n    variable-refresh-rate on-demand=true\n}}\n"
        ));
    }
    if play {
        text.push_str(PLAY);
    }
    text
}

pub fn action(shortcut: Shortcut) -> (&'static str, &'static str) {
    match shortcut {
        Shortcut::Launcher => ("Search apps and games", "launcher"),
        Shortcut::Panel => ("Quick settings", "panel"),
        Shortcut::Switch => ("Switch mode", "switch"),
        Shortcut::Lock => ("Lock the screen", "lock"),
    }
}

fn keyboard(keyboard: &Keyboard) -> Option<String> {
    let mut xkb = Vec::new();
    if !keyboard.layouts.is_empty() {
        xkb.push(format!("layout \"{}\"", keyboard.layouts.join(",")));
    }
    if let Some(switch) = keyboard.switch {
        xkb.push(format!("options \"{}\"", switch.xkb()));
    }
    let mut lines = Vec::new();
    if !xkb.is_empty() {
        lines.push(block("xkb", &xkb));
    }
    if let Some(delay) = keyboard.repeat_delay {
        lines.push(format!("repeat-delay {delay}"));
    }
    if let Some(rate) = keyboard.repeat_rate {
        lines.push(format!("repeat-rate {rate}"));
    }
    (!lines.is_empty()).then(|| block("keyboard", &lines))
}

fn touchpad(touchpad: Touchpad) -> String {
    let flags = [
        (touchpad.tap, "tap"),
        (touchpad.natural_scroll, "natural-scroll"),
        (touchpad.disable_while_typing, "dwt"),
    ];
    let mut lines: Vec<String> = flags
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, flag)| flag.to_owned())
        .collect();
    lines.push(format!("accel-speed {:.2}", touchpad.speed));
    block("touchpad", &lines)
}

fn mouse(mouse: Mouse) -> String {
    let mut lines = Vec::new();
    if mouse.natural_scroll {
        lines.push("natural-scroll".to_owned());
    }
    if !mouse.acceleration {
        lines.push("accel-profile \"flat\"".to_owned());
    }
    lines.push(format!("accel-speed {:.2}", mouse.speed));
    block("mouse", &lines)
}

fn block(name: &str, lines: &[String]) -> String {
    let body: String = lines
        .iter()
        .flat_map(|line| line.lines())
        .map(|line| format!("    {line}\n"))
        .collect();
    format!("{name} {{\n{body}}}\n")
}

pub fn sync(fragment: &Fragment) {
    let path = path();
    if path.exists()
        && let Err(err) = replace(&path, &render(fragment))
    {
        eprintln!("apolline: cannot update {}: {err}", path.display());
    }
}

pub fn setup(path: &Path, niri: &Path, fragment: &Fragment) -> io::Result<Vec<String>> {
    let current = fs::read_to_string(niri).map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => io::Error::new(
            err.kind(),
            format!(
                "{} is missing; start niri once so it writes its config, then run this again",
                niri.display()
            ),
        ),
        _ => err,
    })?;

    let mut done = Vec::new();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    replace(path, &render(fragment))?;
    done.push(format!("wrote {}", path.display()));

    let include = format!("include \"{}\"", path.display());
    if current.lines().any(|line| line.trim() == include) {
        done.push(format!("{} already includes it", niri.display()));
        return Ok(done);
    }

    let backup = niri.with_extension("kdl.before-apolline");
    fs::copy(niri, &backup)?;
    done.push(format!("saved your niri config as {}", backup.display()));

    let gap = if current.is_empty() || current.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let mut file = OpenOptions::new().append(true).open(niri)?;
    write!(file, "{gap}\n{include}\n")?;
    done.push(format!(
        "added the include at the end of {}",
        niri.display()
    ));
    Ok(done)
}

fn replace(path: &Path, text: &str) -> io::Result<()> {
    let staged = path.with_extension("kdl.new");
    fs::write(&staged, text)?;
    fs::rename(&staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apolline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn look(config: &Config, mode: Mode) -> Fragment<'_> {
        Fragment {
            config,
            mode,
            outputs: &[],
        }
    }

    #[test]
    fn borders_take_the_mode_accent_and_layout_follows_the_config() {
        let (config, _) = Config::parse(
            "[layout]\ngaps = 4\nradius = 0\nborders = false\n[mode.play]\naccent = \"#c084fc\"\n",
        );
        let text = render(&look(&config, Mode::Play));
        assert!(text.contains("active-color \"#c084fc\""));
        assert!(text.contains("    gaps 4\n"));
        assert!(text.contains("geometry-corner-radius 0\n"));
        assert!(text.contains("border {\n        off\n"));
        let work = render(&look(&Config::default(), Mode::Work));
        assert!(work.contains("active-color \"#86bdf0\""));
        assert!(work.contains("border {\n        on\n"));
        assert!(!work.contains("input {"), "input stays with niri until set");
        assert!(work.contains(
            "binds {\n    Mod+Space hotkey-overlay-title=\"Search apps and games\" { spawn \"apolline\" \"launcher\"; }\n"
        ));
        assert!(work.contains(
            "    Super+Alt+L hotkey-overlay-title=\"Lock the screen\" { spawn \"apolline\" \"lock\"; }\n"
        ));
    }

    #[test]
    fn input_settings_become_niri_blocks() {
        let (config, _) = Config::parse(
            "[keyboard]\nlayouts = [\"us\", \"ru\"]\nswitch = \"caps\"\nrepeat_rate = 40\n\n[touchpad]\ntap = false\nspeed = 0.3\n\n[mouse]\nacceleration = false\n",
        );
        let text = render(&look(&config, Mode::Work));
        let input = &text[text.find("\ninput {").expect("an input block")..];
        assert!(input.starts_with(
            "\ninput {\n    keyboard {\n        xkb {\n            layout \"us,ru\"\n            options \"grp:caps_toggle\"\n        }\n        repeat-rate 40\n    }\n"
        ));
        assert!(input.contains(
            "    touchpad {\n        natural-scroll\n        dwt\n        accel-speed 0.30\n    }\n"
        ));
        assert!(input.contains(
            "    mouse {\n        accel-profile \"flat\"\n        accel-speed 0.00\n    }\n"
        ));
        assert!(!input.contains("tap\n"));
    }

    #[test]
    fn play_drops_effects_and_lets_games_drive_the_refresh_rate() {
        let outputs = ["eDP-1".to_owned(), "DP-2".to_owned()];
        let config = Config::default();
        let calm = render(&Fragment {
            outputs: &outputs,
            ..look(&config, Mode::Work)
        });
        let play = render(&Fragment {
            outputs: &outputs,
            ..look(&config, Mode::Play)
        });

        for text in [&calm, &play] {
            assert!(
                text.contains("output \"eDP-1\" {\n    variable-refresh-rate on-demand=true\n}")
            );
            assert!(text.contains("output \"DP-2\""));
        }
        assert!(!calm.contains("variable-refresh-rate true"));
        assert!(play.contains("variable-refresh-rate true"));
        assert!(play.contains("animations {\n    off\n}"));
        assert!(calm.contains("shadow {\n        on"));
        assert!(play.contains("shadow {\n        off"));
        for text in [&calm, &play] {
            assert_eq!(
                text.matches("\nlayout {").count(),
                1,
                "niri allows one layout per file"
            );
        }
    }

    #[test]
    fn setup_appends_the_include_once_and_keeps_a_copy() {
        let dir = scratch("setup");
        let niri = dir.join("config.kdl");
        let fragment = dir.join("apolline").join("niri.kdl");
        fs::write(&niri, "binds {\n    Mod+T { spawn \"foot\"; }\n}").unwrap();

        let mut evening = Config::default();
        evening.mode = Mode::Evening;
        setup(&fragment, &niri, &look(&Config::default(), Mode::Work)).unwrap();
        let again = setup(&fragment, &niri, &look(&evening, Mode::Evening)).unwrap();

        let text = fs::read_to_string(&niri).unwrap();
        assert_eq!(text.matches("include").count(), 1);
        assert!(text.starts_with("binds {"));
        assert!(
            text.trim_end()
                .ends_with(&format!("include \"{}\"", fragment.display()))
        );
        assert!(again.last().unwrap().contains("already includes it"));
        assert!(fs::read_to_string(&fragment).unwrap().contains("#f2b35c"));
        assert!(dir.join("config.kdl.before-apolline").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn setup_refuses_to_invent_a_niri_config() {
        let dir = scratch("missing");
        let err = setup(
            &dir.join("apolline/niri.kdl"),
            &dir.join("config.kdl"),
            &look(&Config::default(), Mode::Work),
        )
        .unwrap_err();
        assert!(err.to_string().contains("start niri once"));
        assert!(!dir.join("config.kdl").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use apolline_config::{Center, Config, Keyboard, Mouse, Rule, Shortcut, Touchpad, Width, Windows};

const TEMPLATE: &str = include_str!("../../../assets/niri.kdl");
const PLAY: &str = r##"
window-rule {
    match app-id=r#"^steam_app_"#
    match app-id=r#"^gamescope$"#
    match app-id=r#"\.exe$"#
    variable-refresh-rate true
}
"##;

const MEDIA_KEYS: [(&str, &str); 6] = [
    ("XF86AudioRaiseVolume", "volume up"),
    ("XF86AudioLowerVolume", "volume down"),
    ("XF86AudioMute", "volume mute"),
    ("XF86AudioMicMute", "mic mute"),
    ("XF86MonBrightnessUp", "brightness up"),
    ("XF86MonBrightnessDown", "brightness down"),
];

pub struct Fragment<'a> {
    pub config: &'a Config,
    pub mode: Option<usize>,
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
    let mode = fragment.mode.and_then(|i| fragment.config.modes.get(i));
    let play = mode.is_some_and(|m| m.performance);
    let layout = fragment.config.layout;
    let on = |yes: bool| if yes { "on" } else { "off" };
    let mut text = TEMPLATE.to_owned();
    text.push_str(&format!(
        r##"
layout {{
    gaps {gaps}
    background-color "transparent"
{columns}
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
        columns = columns(&fragment.config.windows),
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
    let mut binds = binds;
    binds.extend(MEDIA_KEYS.iter().map(|(key, words)| {
        let args: Vec<String> = words.split(' ').map(|w| format!("\"{w}\"")).collect();
        format!(
            "{key} allow-when-locked=true {{ spawn \"apolline\" {}; }}",
            args.join(" ")
        )
    }));
    if let Some(mode) = mode {
        let free = |combo: &String| {
            !Shortcut::ALL
                .into_iter()
                .any(|s| crate::keys::same(config.shortcuts.get(s), combo))
        };
        binds.extend(
            mode.keys
                .iter()
                .filter(|(combo, _)| free(combo))
                .map(|(combo, command)| {
                    let title = quote(&format!("{}: {command}", mode.name));
                    format!(
                        "{combo} hotkey-overlay-title=\"{title}\" {{ spawn-sh \"{}\"; }}",
                        quote(command)
                    )
                }),
        );
    }
    text.push_str(&format!("\n{}", block("binds", &binds)));
    let windows = &config.windows;
    let devices: Vec<String> = [
        keyboard(&config.keyboard),
        config.touchpad.map(touchpad),
        config.mouse.map(mouse),
        windows
            .focus_follows_mouse
            .then(|| "focus-follows-mouse".to_owned()),
        windows.warp_mouse.then(|| "warp-mouse-to-focus".to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !devices.is_empty() {
        text.push_str(&format!("\n{}", block("input", &devices)));
    }
    for output in fragment.outputs {
        let name = quote(output);
        text.push_str(&format!(
            "\noutput \"{name}\" {{\n    variable-refresh-rate on-demand=true\n}}\n"
        ));
    }
    let speed = if play { Some(0.0) } else { windows.animations };
    if let Some(speed) = speed {
        let body = if speed <= 0.0 {
            "off".to_owned()
        } else {
            format!("slowdown {:.2}", 1.0 / speed)
        };
        text.push_str(&format!("\n{}", block("animations", &[body])));
    }
    if let Some(opacity) = windows.opacity {
        text.push_str(&format!(
            "\n{}",
            block("window-rule", &[format!("opacity {opacity:.2}")])
        ));
    }
    if let Some(opacity) = windows.inactive_opacity {
        let lines = [
            "match is-active=false".to_owned(),
            format!("opacity {opacity:.2}"),
        ];
        text.push_str(&format!("\n{}", block("window-rule", &lines)));
    }
    let switcher = if windows.switcher {
        let accent = fragment.config.palette(fragment.mode).accent.to_hex();
        let highlight = [
            format!("active-color \"{accent}\""),
            "urgent-color \"#ff6a3d\"".to_owned(),
            "padding 18".to_owned(),
            format!("corner-radius {}", layout.radius + 6),
        ];
        vec![block("highlight", &highlight)]
    } else {
        vec!["off".to_owned()]
    };
    text.push_str(&format!("\n{}", block("recent-windows", &switcher)));
    for rule in &config.rules {
        text.push_str(&format!("\n{}", app_rule(rule)));
    }
    if play {
        text.push_str(PLAY);
    }
    text
}

fn columns(windows: &Windows) -> String {
    let mut lines = Vec::new();
    if let Some(width) = windows.width {
        lines.push(block("default-column-width", &[proportion(width)]));
    }
    if let Some(presets) = &windows.presets {
        let widths: Vec<String> = presets.iter().copied().map(proportion).collect();
        lines.push(block("preset-column-widths", &widths));
    }
    if let Some(center) = windows.center {
        let value = match center {
            Center::Never => "never",
            Center::Overflow => "on-overflow",
            Center::Always => "always",
        };
        lines.push(format!("center-focused-column \"{value}\""));
    }
    if windows.center_lone {
        lines.push("always-center-single-column".to_owned());
    }
    lines
        .iter()
        .flat_map(|line| line.lines())
        .map(|line| format!("    {line}\n"))
        .collect()
}

fn proportion(width: Width) -> String {
    format!("proportion {:.5}", width.proportion())
}

fn app_rule(rule: &Rule) -> String {
    let pattern: String = rule
        .app
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_string()
            } else {
                format!("\\{c}")
            }
        })
        .collect();
    let mut lines = vec![format!("match app-id=r#\"^{pattern}$\"#")];
    if let Some(floating) = rule.floating {
        lines.push(format!("open-floating {floating}"));
    }
    if let Some(width) = rule.width {
        lines.push(format!("default-column-width {{ {}; }}", proportion(width)));
    }
    if let Some(opacity) = rule.opacity {
        lines.push(format!("opacity {opacity:.2}"));
    }
    block("window-rule", &lines)
}

pub fn action(shortcut: Shortcut) -> (&'static str, &'static str) {
    match shortcut {
        Shortcut::Launcher => ("Search apps and games", "launcher"),
        Shortcut::Panel => ("Quick settings", "panel"),
        Shortcut::Switch => ("Switch mode", "switch"),
        Shortcut::Lock => ("Lock the screen", "lock"),
        Shortcut::Clipboard => ("Clipboard history", "clipboard"),
        Shortcut::Keys => ("Keyboard shortcuts", "keys"),
    }
}

fn quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
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
    let text = render(fragment);
    if fs::read_to_string(&path).is_ok_and(|current| current != text)
        && let Err(err) = replace(&path, &text)
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

    fn look(config: &Config, mode: Option<usize>) -> Fragment<'_> {
        Fragment {
            config,
            mode,
            outputs: &[],
        }
    }

    #[test]
    fn borders_take_the_mode_accent_and_layout_follows_the_config() {
        let (config, _) = Config::parse(
            "[layout]\ngaps = 4\nradius = 0\nborders = false\n[[mode]]\nname = \"Night\"\naccent = \"#c084fc\"\nkeys = { \"Mod+N\" = \"notify-send \\\"hi\\\"\" }\n",
        );
        let text = render(&look(&config, Some(0)));
        assert!(text.contains(
            "    Mod+N hotkey-overlay-title=\"Night: notify-send \\\"hi\\\"\" { spawn-sh \"notify-send \\\"hi\\\"\"; }\n"
        ), "{text}");
        assert!(
            !render(&look(&config, None)).contains("Mod+N"),
            "mode keys leave with the mode"
        );
        let (clash, _) =
            Config::parse("[[mode]]\nname = \"X\"\nkeys = { \"Mod+Space\" = \"foot\" }\n");
        assert_eq!(
            render(&look(&clash, Some(0))).matches("Mod+Space").count(),
            1,
            "a mode key never doubles an Apolline shortcut"
        );
        assert!(text.contains("active-color \"#c084fc\""));
        assert!(text.contains("    gaps 4\n"));
        assert!(text.contains("geometry-corner-radius 0\n"));
        assert!(text.contains("border {\n        off\n"));
        let work = render(&look(&Config::default(), None));
        assert!(work.contains("active-color \"#86bdf0\""));
        assert!(work.contains("border {\n        on\n"));
        assert!(!work.contains("input {"), "input stays with niri until set");
        assert!(work.contains(
            "    Mod+Space hotkey-overlay-title=\"Search apps and games\" { spawn \"apolline\" \"launcher\"; }\n"
        ));
        assert!(work.contains(
            "    Super+Alt+L hotkey-overlay-title=\"Lock the screen\" { spawn \"apolline\" \"lock\"; }\n"
        ));
        assert!(work.contains(
            "    XF86AudioRaiseVolume allow-when-locked=true { spawn \"apolline\" \"volume\" \"up\"; }\n"
        ));
    }

    #[test]
    fn window_settings_and_app_rules_become_niri_config() {
        let (config, _) = Config::parse(
            "[windows]\nwidth = \"2/3\"\npresets = [\"1/3\", \"1/2\"]\ncenter = \"overflow\"\ncenter_lone = true\nfocus_follows_mouse = true\nopacity = 0.97\ninactive_opacity = 0.85\nanimations = 2\n\n[[rule]]\napp = \"org.pulseaudio.pavucontrol\"\nfloating = true\nwidth = \"1/3\"\n\n[[mode]]\nname = \"Play\"\nperformance = true\n",
        );
        let text = render(&look(&config, None));
        assert!(text.contains("    default-column-width {\n        proportion 0.66667\n    }\n"));
        assert!(text.contains("    preset-column-widths {\n        proportion 0.33333\n        proportion 0.50000\n    }\n"));
        assert!(text.contains(
            "    center-focused-column \"on-overflow\"\n    always-center-single-column\n"
        ));
        assert!(text.contains("    focus-follows-mouse\n"));
        assert!(text.contains("animations {\n    slowdown 0.50\n}"));
        assert!(text.contains("window-rule {\n    match is-active=false\n    opacity 0.85\n}"));
        assert!(text.contains(
            "window-rule {\n    match app-id=r#\"^org\\.pulseaudio\\.pavucontrol$\"#\n    open-floating true\n    default-column-width { proportion 0.33333; }\n}"
        ), "{text}");
        assert!(
            text.contains("recent-windows {\n    highlight {\n        active-color \"#86bdf0\"")
        );
        let play = render(&look(&config, Some(0)));
        assert_eq!(
            play.matches("\nanimations {").count(),
            1,
            "one animations block"
        );
        assert!(play.contains("animations {\n    off\n}"));
    }

    #[test]
    fn input_settings_become_niri_blocks() {
        let (config, _) = Config::parse(
            "[keyboard]\nlayouts = [\"us\", \"ru\"]\nswitch = \"caps\"\nrepeat_rate = 40\n\n[touchpad]\ntap = false\nspeed = 0.3\n\n[mouse]\nacceleration = false\n",
        );
        let text = render(&look(&config, None));
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
        let (config, _) = Config::parse("[[mode]]\nname = \"Play\"\nperformance = true\n");
        let calm = render(&Fragment {
            outputs: &outputs,
            ..look(&config, None)
        });
        let play = render(&Fragment {
            outputs: &outputs,
            ..look(&config, Some(0))
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
        evening.desktop.accent =
            apolline_config::Accent::Fixed(apolline_theme::Rgba::hex(0xf2b35c));
        setup(&fragment, &niri, &look(&Config::default(), None)).unwrap();
        let again = setup(&fragment, &niri, &look(&evening, None)).unwrap();

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
            &look(&Config::default(), None),
        )
        .unwrap_err();
        assert!(err.to_string().contains("start niri once"));
        assert!(!dir.join("config.kdl").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

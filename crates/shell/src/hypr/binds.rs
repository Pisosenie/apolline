use apolline_config::{KeyAction, Shortcut};

// What a Hyprland bind becomes here.
#[derive(Debug, PartialEq)]
pub enum Becomes {
    Key {
        combo: String,
        action: KeyAction,
        note: Option<String>,
    },
    // One of Apolline's own keys moves to where the bind was.
    Shortcut(String, Shortcut),
    // Apolline or niri does it already, on keys of their own.
    Covered(&'static str),
    Left(String),
}

const LAUNCHERS: [&str; 9] = [
    "rofi",
    "wofi",
    "fuzzel",
    "tofi",
    "anyrun",
    "walker",
    "bemenu-run",
    "dmenu_run",
    "ulauncher",
];
const LOCKERS: [&str; 4] = ["hyprlock", "swaylock", "gtklock", "waylock"];
const SHOOTERS: [&str; 6] = [
    "grimblast",
    "hyprshot",
    "grim",
    "flameshot",
    "satty",
    "swappy",
];
const SOUND_AND_LIGHT: [&str; 6] = [
    "wpctl",
    "pamixer",
    "pactl",
    "brightnessctl",
    "light",
    "swayosd-client",
];
const DRAWN_HERE: [&str; 9] = [
    "waybar",
    "swww",
    "hyprpaper",
    "waypaper",
    "eww",
    "ags",
    "quickshell",
    "swaync",
    "mako",
];

// `bind[flags] = MODS, key, dispatcher, params`; with d a description comes before the dispatcher.
pub fn bind(flags: &str, value: &str) -> Becomes {
    if flags.contains('m') {
        return Becomes::Covered("niri moves and resizes windows with Mod and the mouse already");
    }
    let fields = if flags.contains('d') { 5 } else { 4 };
    let parts: Vec<&str> = value.splitn(fields, ',').map(str::trim).collect();
    let (mods, key) = (
        parts.first().copied().unwrap_or(""),
        parts.get(1).copied().unwrap_or(""),
    );
    let dispatcher = parts.get(fields - 2).copied().unwrap_or("");
    let params = parts.get(fields - 1).copied().unwrap_or("");
    let combo = match combo(mods, key) {
        Ok(combo) => combo,
        Err(why) => return Becomes::Left(why),
    };
    if key.starts_with("XF86Audio") || key.starts_with("XF86MonBrightness") {
        return Becomes::Covered("Apolline answers the sound and brightness keys itself");
    }
    let niri = |action: &str| Becomes::Key {
        combo: combo.clone(),
        action: KeyAction::Niri(action.to_owned()),
        note: None,
    };
    let left = |why: &str| Becomes::Left(why.to_owned());
    match dispatcher.to_lowercase().as_str() {
        "exec" | "execr" => command(combo, params),
        "killactive" | "forcekillactive" => niri("close-window"),
        "exit" => niri("quit"),
        "togglefloating" => niri("toggle-window-floating"),
        "fullscreen" if params == "1" => niri("maximize-column"),
        "fullscreen" => niri("fullscreen-window"),
        "centerwindow" => niri("center-column"),
        "focuscurrentorlast" | "focusurgentorlast" => niri("focus-window-previous"),
        "togglegroup" => niri("toggle-column-tabbed-display"),
        "dpms" if params.starts_with("off") => niri("power-off-monitors"),
        "cyclenext" if params.starts_with("prev") => niri("focus-column-left"),
        "cyclenext" => niri("focus-column-right"),
        "movefocus" => {
            directed(params, "focus-column", "focus-window").map_or_else(left_way, |a| niri(&a))
        }
        "movewindow" | "swapwindow" => match params.strip_prefix("mon:") {
            Some(way) => side(way)
                .map(|side| niri(&format!("move-column-to-monitor-{side}")))
                .unwrap_or_else(|| left("only monitors to the left, right, up or down come over")),
            None => {
                directed(params, "move-column", "move-window").map_or_else(left_way, |a| niri(&a))
            }
        },
        "focusmonitor" => match params {
            "+1" => niri("focus-monitor-next"),
            "-1" => niri("focus-monitor-previous"),
            way => side(way)
                .map(|side| niri(&format!("focus-monitor-{side}")))
                .unwrap_or_else(|| left("only monitors to a side or the next one come over")),
        },
        "movecurrentworkspacetomonitor" => side(params)
            .map(|side| niri(&format!("move-workspace-to-monitor-{side}")))
            .unwrap_or_else(|| left("only monitors to a side come over")),
        "changegroupactive" if params.starts_with('b') => niri("focus-window-up"),
        "changegroupactive" => niri("focus-window-down"),
        "moveintogroup" | "moveoutofgroup" => match side(params) {
            Some(side @ ("left" | "right")) => niri(&format!("consume-or-expel-window-{side}")),
            _ => left("only taking a window into the column beside comes over"),
        },
        "workspace" => {
            workspace(params, "focus-workspace", "").map_or_else(left_owned, |a| niri(&a))
        }
        "movetoworkspace" => {
            workspace(params, "move-column-to-workspace", "").map_or_else(left_owned, |a| niri(&a))
        }
        "movetoworkspacesilent" => workspace(params, "move-column-to-workspace", " focus=false")
            .map_or_else(left_owned, |a| niri(&a)),
        "resizeactive" => resize(params).map_or_else(left_owned, |a| niri(&a)),
        "togglesplit" | "pseudo" | "splitratio" | "layoutmsg" | "swapnext" => {
            left("niri lays windows out in scrolling columns, not a split tree")
        }
        "togglespecialworkspace" => left("niri has no scratchpad workspace"),
        "submap" => left("niri has no key chords (submaps)"),
        "pin" => left("niri has no windows pinned over every workspace"),
        "pass" | "sendshortcut" | "global" => left("niri passes keys to apps by itself"),
        other => Becomes::Left(format!("{other} is Hyprland's own")),
    }
}

fn left_way(_: ()) -> Becomes {
    Becomes::Left("only left, right, up and down come over".into())
}

fn left_owned(why: String) -> Becomes {
    Becomes::Left(why)
}

// SUPER SHIFT, SUPER_SHIFT or SUPERSHIFT, and a key as xkb names it.
pub fn combo(mods: &str, key: &str) -> Result<String, String> {
    const NAMES: [(&str, &str); 9] = [
        ("SUPER", "Mod"),
        ("WIN", "Mod"),
        ("LOGO", "Mod"),
        ("MOD4", "Mod"),
        ("CONTROL", "Ctrl"),
        ("CTRL", "Ctrl"),
        ("ALT", "Alt"),
        ("MOD1", "Alt"),
        ("SHIFT", "Shift"),
    ];
    let mut rest: String = mods
        .to_uppercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '+' | '&'))
        .collect();
    let mut held: Vec<&str> = Vec::new();
    while !rest.is_empty() {
        let Some((name, as_niri)) = NAMES.iter().find(|(name, _)| rest.starts_with(name)) else {
            return Err(format!("the modifier {mods} has no match in niri"));
        };
        rest.drain(..name.len());
        if !held.contains(as_niri) {
            held.push(as_niri);
        }
    }
    held.sort_by_key(|m| ["Mod", "Ctrl", "Alt", "Shift"].iter().position(|o| o == m));
    let key = match key {
        "mouse_down" => "WheelScrollDown".to_owned(),
        "mouse_up" => "WheelScrollUp".to_owned(),
        "mouse_left" => "WheelScrollLeft".to_owned(),
        "mouse_right" => "WheelScrollRight".to_owned(),
        _ if key.starts_with("code:") => return Err("keys by keycode don't come over".into()),
        _ if key.starts_with("mouse:") => {
            return Err("mouse buttons don't come over as keys".into());
        }
        _ if key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()) => key.to_uppercase(),
        // xkb names are found in any case; "left" reads better as Left.
        _ if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
            let mut chars = key.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        }
        _ => return Err(format!("the key “{key}” has no name niri knows")),
    };
    Ok(held
        .into_iter()
        .map(str::to_owned)
        .chain(std::iter::once(key))
        .collect::<Vec<_>>()
        .join("+"))
}

fn side(way: &str) -> Option<&'static str> {
    match way {
        "l" | "left" => Some("left"),
        "r" | "right" => Some("right"),
        "u" | "up" | "t" => Some("up"),
        "d" | "down" | "b" => Some("down"),
        _ => None,
    }
}

// Sideways is between columns, up and down within one.
fn directed(way: &str, across: &str, within: &str) -> Result<String, ()> {
    match side(way).ok_or(())? {
        side @ ("left" | "right") => Ok(format!("{across}-{side}")),
        side => Ok(format!("{within}-{side}")),
    }
}

fn workspace(which: &str, action: &str, quiet: &str) -> Result<String, String> {
    let which = which.trim();
    if let Ok(number) = which.parse::<u8>() {
        return Ok(format!("{action} {number}{quiet}"));
    }
    match which.trim_start_matches(['e', 'm', 'r']) {
        "+1" => Ok(format!("{action}-down")),
        "-1" => Ok(format!("{action}-up")),
        "previous" if action == "focus-workspace" => Ok("focus-workspace-previous".into()),
        _ if which.starts_with("special") => Err("niri has no scratchpad workspace".into()),
        _ => Err(format!("the workspace “{which}” has no match in niri")),
    }
}

// resizeactive dx 0 widens the column, 0 dy heightens the window.
fn resize(params: &str) -> Result<String, String> {
    let numbers: Vec<&str> = params.split_whitespace().collect();
    let signed = |n: &str| {
        let (digits, percent) = n.strip_suffix('%').map_or((n, ""), |d| (d, "%"));
        digits
            .parse::<i32>()
            .ok()
            .map(|v| format!("{v:+}{percent}"))
    };
    match numbers.as_slice() {
        [dx, dy] if dy.trim_end_matches('%') == "0" => signed(dx)
            .map(|dx| format!("set-column-width {dx}"))
            .ok_or_else(|| "this resize has no match in niri".into()),
        [dx, dy] if dx.trim_end_matches('%') == "0" => signed(dy)
            .map(|dy| format!("set-window-height {dy}"))
            .ok_or_else(|| "this resize has no match in niri".into()),
        _ => Err("resizing both ways at once has no match in niri".into()),
    }
}

// What a command does decides where it goes: Apolline's own launcher, lock, clipboard and quick
// settings take theirs, niri takes screenshots, and the rest runs as it did.
fn command(combo: String, line: &str) -> Becomes {
    // [workspace 2 silent] rules ahead of the command are Hyprland's.
    let line = match line.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map_or(rest, |(_, after)| after).trim(),
        None => line.trim(),
    };
    let program = line
        .split_whitespace()
        .next()
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .unwrap_or_default();
    let has = |words: &[&str]| words.iter().any(|w| line.contains(w));
    if line.is_empty() {
        return Becomes::Left("it runs nothing".into());
    }
    if program == "hyprctl" {
        return Becomes::Left("it talks to Hyprland, which isn't there under niri".into());
    }
    if has(&["cliphist", "clipman", "copyq"]) {
        return Becomes::Shortcut(combo, Shortcut::Clipboard);
    }
    if LOCKERS.contains(&program) || line.starts_with("loginctl lock-session") {
        return Becomes::Shortcut(combo, Shortcut::Lock);
    }
    if LAUNCHERS.contains(&program) {
        if has(&["-show window", "--show window"]) {
            return Becomes::Covered("Alt+Tab switches windows");
        }
        if !has(&["power", "emoji", "calc", ".sh"]) {
            return Becomes::Shortcut(combo, Shortcut::Launcher);
        }
    }
    if has(&["swaync-client -t", "dunstctl history", "makoctl restore"]) {
        return Becomes::Shortcut(combo, Shortcut::Panel);
    }
    if SHOOTERS.contains(&program) {
        let action = if has(&["output", "screen", "-m output", " fullscreen"]) {
            "screenshot-screen"
        } else if has(&["active", "window"]) {
            "screenshot-window"
        } else {
            "screenshot"
        };
        return Becomes::Key {
            combo,
            action: KeyAction::Niri(action.into()),
            note: None,
        };
    }
    if SOUND_AND_LIGHT.contains(&program) {
        // Off the media keys these still mean something: a key of yours that runs it.
        return run(combo, line, None);
    }
    let killing = matches!(program, "killall" | "pkill");
    if DRAWN_HERE.contains(&program) || (killing && DRAWN_HERE.iter().any(|p| line.contains(p))) {
        return Becomes::Covered("Apolline draws the bar, notifications and the wallpaper");
    }
    run(combo, line, script_note(line))
}

fn run(combo: String, line: &str, note: Option<String>) -> Becomes {
    Becomes::Key {
        combo,
        action: KeyAction::Run(line.to_owned()),
        note,
    }
}

// A script of yours that asks Hyprland things won't get answers under niri.
fn script_note(line: &str) -> Option<String> {
    let first = line.split_whitespace().next()?;
    let path = match first.strip_prefix("~/") {
        Some(rest) => std::path::PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None => first.replace("$HOME", &std::env::var("HOME").ok()?).into(),
    };
    let text = std::fs::read_to_string(path).ok()?;
    text.contains("hyprctl")
        .then(|| "the script calls hyprctl, which won't answer under niri".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn niri(combo: &str, action: &str) -> Becomes {
        Becomes::Key {
            combo: combo.into(),
            action: KeyAction::Niri(action.into()),
            note: None,
        }
    }

    #[test]
    fn modifiers_and_keys_are_named_as_niri_names_them() {
        assert_eq!(combo("SUPER", "q"), Ok("Mod+Q".into()));
        assert_eq!(
            combo("SUPER SHIFT", "Return"),
            Ok("Mod+Shift+Return".into())
        );
        assert_eq!(combo("SUPERSHIFT", "1"), Ok("Mod+Shift+1".into()));
        assert_eq!(combo("CTRL_ALT", "Delete"), Ok("Ctrl+Alt+Delete".into()));
        assert_eq!(combo("", "Print"), Ok("Print".into()));
        assert_eq!(
            combo("SUPER", "mouse_down"),
            Ok("Mod+WheelScrollDown".into())
        );
        assert!(combo("SUPER", "code:10").is_err());
        assert!(combo("HYPER", "a").is_err());
    }

    #[test]
    fn hyprlands_dispatchers_become_niris_actions() {
        let cases = [
            ("", "SUPER, C, killactive", niri("Mod+C", "close-window")),
            (
                "",
                "SUPER, F, fullscreen, 1",
                niri("Mod+F", "maximize-column"),
            ),
            (
                "",
                "SUPER, left, movefocus, l",
                niri("Mod+Left", "focus-column-left"),
            ),
            (
                "",
                "SUPER, down, movefocus, d",
                niri("Mod+Down", "focus-window-down"),
            ),
            (
                "",
                "SUPER SHIFT, H, movewindow, l",
                niri("Mod+Shift+H", "move-column-left"),
            ),
            (
                "",
                "SUPER, 3, workspace, 3",
                niri("Mod+3", "focus-workspace 3"),
            ),
            (
                "",
                "SUPER, mouse_down, workspace, e+1",
                niri("Mod+WheelScrollDown", "focus-workspace-down"),
            ),
            (
                "",
                "SUPER SHIFT, 3, movetoworkspace, 3",
                niri("Mod+Shift+3", "move-column-to-workspace 3"),
            ),
            (
                "",
                "SUPER ALT, 3, movetoworkspacesilent, 3",
                niri("Mod+Alt+3", "move-column-to-workspace 3 focus=false"),
            ),
            (
                "e",
                "SUPER CTRL, right, resizeactive, 30 0",
                niri("Mod+Ctrl+Right", "set-column-width +30"),
            ),
            (
                "e",
                "SUPER CTRL, down, resizeactive, 0 -30",
                niri("Mod+Ctrl+Down", "set-window-height -30"),
            ),
            (
                "d",
                "SUPER, W, float it, togglefloating",
                niri("Mod+W", "toggle-window-floating"),
            ),
            (
                "",
                "SUPER, G, togglegroup",
                niri("Mod+G", "toggle-column-tabbed-display"),
            ),
        ];
        for (flags, value, wanted) in cases {
            assert_eq!(bind(flags, value), wanted, "{value}");
        }
    }

    #[test]
    fn commands_go_to_apollines_own_keys_where_it_has_them() {
        assert_eq!(
            bind("", "SUPER, R, exec, rofi -show drun"),
            Becomes::Shortcut("Mod+R".into(), Shortcut::Launcher)
        );
        assert_eq!(
            bind(
                "",
                "SUPER, V, exec, cliphist list | rofi -dmenu | cliphist decode | wl-copy"
            ),
            Becomes::Shortcut("Mod+V".into(), Shortcut::Clipboard)
        );
        assert_eq!(
            bind("", "SUPER, L, exec, hyprlock"),
            Becomes::Shortcut("Mod+L".into(), Shortcut::Lock)
        );
        assert_eq!(
            bind("", ", Print, exec, grimblast copy area"),
            niri("Print", "screenshot")
        );
        assert_eq!(
            bind("", "SUPER, Return, exec, [workspace 2 silent] kitty"),
            Becomes::Key {
                combo: "Mod+Return".into(),
                action: KeyAction::Run("kitty".into()),
                note: None
            }
        );
        assert!(matches!(
            bind(
                "l",
                ", XF86AudioMute, exec, wpctl set-mute @DEFAULT_SINK@ toggle"
            ),
            Becomes::Covered(_)
        ));
        assert!(matches!(
            bind("", "SUPER, B, exec, killall waybar || waybar"),
            Becomes::Covered(_)
        ));
        assert!(matches!(
            bind("m", "SUPER, mouse:272, movewindow"),
            Becomes::Covered(_)
        ));
        assert!(matches!(
            bind("", "SUPER, J, togglesplit"),
            Becomes::Left(_)
        ));
        assert!(matches!(
            bind("", "SUPER, S, togglespecialworkspace, magic"),
            Becomes::Left(_)
        ));
        assert!(matches!(
            bind("", "SUPER, X, exec, hyprctl dispatch exit"),
            Becomes::Left(_)
        ));
    }
}

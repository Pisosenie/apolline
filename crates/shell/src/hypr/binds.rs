use apolline_config::{KeyAction, Shortcut};
use hypr2niri::keys::{self, Action};

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

const LAUNCHERS: [&str; 10] = [
    "hyprlauncher",
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

// A Hyprland bind as it comes over: niri's action, one of Apolline's own keys, or why not.
pub fn bind(flags: &str, value: &str) -> Becomes {
    if flags.contains('m') {
        return Becomes::Covered("niri moves and resizes windows with Mod and the mouse already");
    }
    let bind = match keys::parse(flags, value) {
        Ok(bind) => bind,
        Err(why) => return Becomes::Left(why),
    };
    if bind.key.starts_with("XF86Audio") || bind.key.starts_with("XF86MonBrightness") {
        return Becomes::Covered("Apolline answers the sound and brightness keys itself");
    }
    match keys::action(&bind.dispatcher, &bind.params) {
        Ok(Action::Niri(action)) => Becomes::Key {
            combo: bind.combo,
            action: KeyAction::Niri(action),
            note: None,
        },
        Ok(Action::Exec(line)) => command(bind.combo, &line),
        Err(why) => Becomes::Left(why),
    }
}

// What a command does decides where it goes: Apolline's own launcher, lock, clipboard and quick
// settings take theirs, niri takes screenshots, and the rest runs as it did.
fn command(combo: String, line: &str) -> Becomes {
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

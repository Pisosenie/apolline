// A Hyprland bind read apart, and what its dispatcher does in niri.

#[derive(Clone, Debug, PartialEq)]
pub struct Bind {
    // As niri names keys: Mod+Shift+Return.
    pub combo: String,
    // As Hyprland wrote it: XF86AudioMute, Return, mouse:272.
    pub key: String,
    pub dispatcher: String,
    pub params: String,
    // l: works while locked, e: repeats, m: a mouse drag, r: on release, d: with a description.
    pub flags: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    // One of niri's actions, as a niri config writes it inside the braces.
    Niri(String),
    // A command line, run by a shell.
    Exec(String),
}

// `bind[flags] = MODS, key, dispatcher, params`; with d a description comes before the dispatcher.
pub fn parse(flags: &str, value: &str) -> Result<Bind, String> {
    let described = flags.contains('d');
    let fields = if described { 5 } else { 4 };
    let parts: Vec<&str> = value.splitn(fields, ',').map(str::trim).collect();
    let get = |i: usize| parts.get(i).copied().unwrap_or("").to_owned();
    let (mods, key) = (get(0), get(1));
    let combo = combo(&mods, &key)?;
    Ok(Bind {
        combo,
        key,
        dispatcher: get(fields - 2),
        params: get(fields - 1),
        flags: flags.to_owned(),
        description: described.then(|| get(2)).filter(|d| !d.is_empty()),
    })
}

// What a dispatcher and its parameters do in niri, or why nothing there does it.
pub fn action(dispatcher: &str, params: &str) -> Result<Action, String> {
    let niri = |action: &str| Ok(Action::Niri(action.to_owned()));
    let left = |why: &str| Err(why.to_owned());
    let way = || "only left, right, up and down come over".to_owned();
    match dispatcher.to_lowercase().as_str() {
        "exec" | "execr" => Ok(Action::Exec(command_line(params))),
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
        "movefocus" => directed(params, "focus-column", "focus-window")
            .map(Action::Niri)
            .ok_or_else(way),
        "movewindow" | "swapwindow" => match params.strip_prefix("mon:") {
            Some(way) => side(way)
                .map(|side| Action::Niri(format!("move-column-to-monitor-{side}")))
                .ok_or_else(|| "only monitors to the left, right, up or down come over".into()),
            None => directed(params, "move-column", "move-window")
                .map(Action::Niri)
                .ok_or_else(way),
        },
        "focusmonitor" => match params {
            "+1" => niri("focus-monitor-next"),
            "-1" => niri("focus-monitor-previous"),
            way => side(way)
                .map(|side| Action::Niri(format!("focus-monitor-{side}")))
                .ok_or_else(|| "only monitors to a side or the next one come over".into()),
        },
        "movecurrentworkspacetomonitor" => side(params)
            .map(|side| Action::Niri(format!("move-workspace-to-monitor-{side}")))
            .ok_or_else(|| "only monitors to a side come over".into()),
        "changegroupactive" if params.starts_with('b') => niri("focus-window-up"),
        "changegroupactive" => niri("focus-window-down"),
        "moveintogroup" | "moveoutofgroup" => match side(params) {
            Some(side @ ("left" | "right")) => niri(&format!("consume-or-expel-window-{side}")),
            _ => left("only taking a window into the column beside comes over"),
        },
        "workspace" => workspace(params, "focus-workspace", "").map(Action::Niri),
        "movetoworkspace" => workspace(params, "move-column-to-workspace", "").map(Action::Niri),
        "movetoworkspacesilent" => {
            workspace(params, "move-column-to-workspace", " focus=false").map(Action::Niri)
        }
        "resizeactive" => resize(params).map(Action::Niri),
        "togglesplit" | "pseudo" | "splitratio" | "layoutmsg" | "swapnext" => {
            left("niri lays windows out in scrolling columns, not a split tree")
        }
        "togglespecialworkspace" => left("niri has no scratchpad workspace"),
        "submap" => left("niri has no key chords (submaps)"),
        "pin" => left("niri has no windows pinned over every workspace"),
        "pass" | "sendshortcut" | "global" => left("niri passes keys to apps by itself"),
        other if other.starts_with("lua:") => left("it runs Lua, which only Hyprland can"),
        other => Err(format!("{other} is Hyprland's own")),
    }
}

// [workspace 2 silent] rules ahead of a command are Hyprland's.
fn command_line(line: &str) -> String {
    match line.strip_prefix('[') {
        Some(rest) => rest
            .split_once(']')
            .map_or(rest, |(_, after)| after)
            .trim()
            .to_owned(),
        None => line.trim().to_owned(),
    }
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

pub fn side(way: &str) -> Option<&'static str> {
    match way {
        "l" | "left" => Some("left"),
        "r" | "right" => Some("right"),
        "u" | "up" | "t" => Some("up"),
        "d" | "down" | "b" => Some("down"),
        _ => None,
    }
}

// Sideways is between columns, up and down within one.
pub fn directed(way: &str, across: &str, within: &str) -> Option<String> {
    match side(way)? {
        side @ ("left" | "right") => Some(format!("{across}-{side}")),
        side => Some(format!("{within}-{side}")),
    }
}

pub fn workspace(which: &str, action: &str, quiet: &str) -> Result<String, String> {
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
pub fn resize(params: &str) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            ("", "SUPER, C, killactive", ("Mod+C", "close-window")),
            ("", "SUPER, F, fullscreen, 1", ("Mod+F", "maximize-column")),
            (
                "",
                "SUPER, left, movefocus, l",
                ("Mod+Left", "focus-column-left"),
            ),
            (
                "",
                "SUPER, down, movefocus, d",
                ("Mod+Down", "focus-window-down"),
            ),
            (
                "",
                "SUPER SHIFT, H, movewindow, l",
                ("Mod+Shift+H", "move-column-left"),
            ),
            ("", "SUPER, 3, workspace, 3", ("Mod+3", "focus-workspace 3")),
            (
                "",
                "SUPER, mouse_down, workspace, e+1",
                ("Mod+WheelScrollDown", "focus-workspace-down"),
            ),
            (
                "",
                "SUPER SHIFT, 3, movetoworkspace, 3",
                ("Mod+Shift+3", "move-column-to-workspace 3"),
            ),
            (
                "",
                "SUPER ALT, 3, movetoworkspacesilent, 3",
                ("Mod+Alt+3", "move-column-to-workspace 3 focus=false"),
            ),
            (
                "e",
                "SUPER CTRL, right, resizeactive, 30 0",
                ("Mod+Ctrl+Right", "set-column-width +30"),
            ),
            (
                "e",
                "SUPER CTRL, down, resizeactive, 0 -30",
                ("Mod+Ctrl+Down", "set-window-height -30"),
            ),
            (
                "d",
                "SUPER, W, float it, togglefloating",
                ("Mod+W", "toggle-window-floating"),
            ),
            (
                "",
                "SUPER, G, togglegroup",
                ("Mod+G", "toggle-column-tabbed-display"),
            ),
        ];
        for (flags, value, (combo, action)) in cases {
            let bind = parse(flags, value).unwrap();
            assert_eq!(bind.combo, combo, "{value}");
            assert_eq!(
                super::action(&bind.dispatcher, &bind.params),
                Ok(Action::Niri(action.into())),
                "{value}"
            );
        }
    }

    #[test]
    fn commands_lose_hyprlands_rules_and_descriptions_are_kept_apart() {
        let bind = parse(
            "d",
            "SUPER, Return, Open a terminal, exec, [workspace 2 silent] kitty",
        )
        .unwrap();
        assert_eq!(bind.description.as_deref(), Some("Open a terminal"));
        assert_eq!(
            action(&bind.dispatcher, &bind.params),
            Ok(Action::Exec("kitty".into()))
        );
        assert!(action("togglesplit", "").is_err());
        assert!(parse("", "SUPER, code:10, exec, foot").is_err());
    }
}

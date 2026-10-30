use iced::keyboard::{Key, Modifiers, key::Named};

pub fn combo(key: &Key, modifiers: Modifiers) -> Option<String> {
    let name = match key.as_ref() {
        Key::Character(c) => character(c)?,
        Key::Named(named) => named_key(named)?,
        _ => return None,
    };
    let function = name.starts_with('F') && name.len() > 1 && name[1..].parse::<u8>().is_ok();
    let mut parts = Vec::new();
    if modifiers.logo() {
        parts.push("Mod");
    }
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if parts.is_empty() && !function {
        return None;
    }
    parts.push(&name);
    Some(parts.join("+"))
}

fn character(c: &str) -> Option<String> {
    let name = match c {
        "/" => "Slash",
        "\\" => "Backslash",
        "," => "Comma",
        "." => "Period",
        "-" => "Minus",
        "=" => "Equal",
        "[" => "BracketLeft",
        "]" => "BracketRight",
        ";" => "Semicolon",
        "'" => "Apostrophe",
        "`" => "Grave",
        c if c.chars().count() == 1 && c.chars().all(|c| c.is_ascii_alphanumeric()) => {
            return Some(c.to_uppercase());
        }
        _ => return None,
    };
    Some(name.to_owned())
}

fn named_key(named: Named) -> Option<String> {
    let name = match named {
        Named::Space => "Space",
        Named::Enter => "Return",
        Named::Tab => "Tab",
        Named::Backspace => "BackSpace",
        Named::Delete => "Delete",
        Named::Insert => "Insert",
        Named::Home => "Home",
        Named::End => "End",
        Named::PageUp => "Page_Up",
        Named::PageDown => "Page_Down",
        Named::ArrowLeft => "Left",
        Named::ArrowRight => "Right",
        Named::ArrowUp => "Up",
        Named::ArrowDown => "Down",
        Named::Print => "Print",
        other => {
            let debug = format!("{other:?}");
            return (debug.starts_with('F') && debug[1..].parse::<u8>().is_ok()).then_some(debug);
        }
    };
    Some(name.to_owned())
}

pub fn same(a: &str, b: &str) -> bool {
    normal(a) == normal(b)
}

fn normal(combo: &str) -> (Vec<&'static str>, String) {
    let mut parts: Vec<&str> = combo.split('+').collect();
    let key = parts.pop().unwrap_or_default().to_lowercase();
    let mut modifiers: Vec<&'static str> = parts
        .into_iter()
        .map(|m| match m.to_lowercase().as_str() {
            "mod" | "super" | "win" | "logo" => "mod",
            "ctrl" | "control" => "ctrl",
            "alt" => "alt",
            "shift" => "shift",
            _ => "other",
        })
        .collect();
    modifiers.sort_unstable();
    modifiers.dedup();
    (modifiers, key)
}

pub fn binds(niri_config: &str) -> Vec<(String, String)> {
    niri_config
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.starts_with("/-"))
        .filter_map(|line| {
            let token = line.split(|c: char| c.is_whitespace() || c == '{').next()?;
            let valid = token.contains('+')
                && token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '_');
            if !valid {
                return None;
            }
            let open = line.find('{')?;
            let close = line.rfind('}')?;
            let action = line[open + 1..close].trim().trim_end_matches(';').trim();
            Some((token.to_owned(), action.to_owned()))
        })
        .collect()
}

pub fn caps(combo: &str) -> Vec<String> {
    combo
        .split('+')
        .map(|part| match part {
            "Mod" => "Super".to_owned(),
            "Page_Up" => "PgUp".to_owned(),
            "Page_Down" => "PgDn".to_owned(),
            other => other.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_become_niri_combinations() {
        let shift_logo = Modifiers::LOGO | Modifiers::SHIFT;
        assert_eq!(
            combo(&Key::Character("m".into()), shift_logo).as_deref(),
            Some("Mod+Shift+M")
        );
        assert_eq!(
            combo(&Key::Named(Named::Space), Modifiers::LOGO).as_deref(),
            Some("Mod+Space")
        );
        assert_eq!(
            combo(&Key::Character("/".into()), Modifiers::CTRL).as_deref(),
            Some("Ctrl+Slash")
        );
        assert_eq!(combo(&Key::Character("a".into()), Modifiers::empty()), None);
        assert_eq!(
            combo(&Key::Named(Named::F5), Modifiers::empty()).as_deref(),
            Some("F5")
        );
    }

    #[test]
    fn conflicts_ignore_spelling_and_order() {
        assert!(same("Mod+Shift+M", "Super+shift+m"));
        assert!(same("Ctrl+Alt+Delete", "Alt+Control+Delete"));
        assert!(!same("Mod+M", "Mod+Shift+M"));
    }

    #[test]
    fn binds_are_read_from_a_niri_config() {
        let config = r#"
binds {
    // Mod+X { spawn "nothing"; }
    Mod+D hotkey-overlay-title="Run an Application: fuzzel" { spawn "fuzzel"; }
    Super+Alt+L { spawn "swaylock"; }
    XF86AudioMute allow-when-locked=true { spawn-sh "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"; }
    Mod+Shift+E { quit; }
}
layout {
    gaps 16
}
"#;
        assert_eq!(
            binds(config),
            [
                ("Mod+D".to_owned(), r#"spawn "fuzzel""#.to_owned()),
                ("Super+Alt+L".to_owned(), r#"spawn "swaylock""#.to_owned()),
                ("Mod+Shift+E".to_owned(), "quit".to_owned()),
            ]
        );
    }
}

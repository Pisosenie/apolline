use std::fs;
use std::path::{Path, PathBuf};

use apolline_config::{Config, Editor};

// What a Hyprland config says about the keyboard, so coming over keeps the layouts and the key
// that switches them.
#[derive(Debug, Default, PartialEq)]
pub struct Keyboard {
    pub layouts: Vec<String>,
    pub toggle: Option<String>,
}

pub fn keyboard(home: &Path) -> Keyboard {
    let mut found = Keyboard::default();
    read(&home.join("hypr/hyprland.conf"), &mut found, 0);
    found
}

fn read(path: &Path, found: &mut Keyboard, depth: u8) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "kb_layout" => {
                found.layouts = value
                    .split(',')
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "kb_options" => {
                found.toggle = value
                    .split(',')
                    .map(str::trim)
                    .find_map(|o| o.strip_prefix("grp:"))
                    .map(str::to_owned);
            }
            // HyDE and most dotfiles keep their input block in a sourced file.
            "source" if depth < 3 => {
                for file in sourced(value, path) {
                    read(&file, found, depth + 1);
                }
            }
            _ => {}
        }
    }
}

fn sourced(value: &str, from: &Path) -> Vec<PathBuf> {
    let path = match value.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from(value), |h| PathBuf::from(h).join(rest)),
        None => PathBuf::from(value),
    };
    let path = if path.is_relative() {
        from.parent().unwrap_or(Path::new(".")).join(path)
    } else {
        path
    };
    if !path.to_string_lossy().contains('*') {
        return vec![path];
    }
    let (Some(dir), Some(pattern)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let pattern = pattern.to_string_lossy();
    let (start, end) = pattern.split_once('*').unwrap_or((&pattern, ""));
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .is_some_and(|n| n.starts_with(start) && n.ends_with(end))
        })
        .collect();
    files.sort();
    files
}

// Writes Hyprland's keyboard into Apolline's config when Apolline has none of its own yet.
pub fn take_keyboard(config: &Config, found: &Keyboard, editor: &mut Editor) -> Vec<String> {
    let keyboard = &config.keyboard;
    if !keyboard.layouts.is_empty()
        || keyboard.switch.is_some()
        || !config.shortcuts.layout.is_empty()
        || found.layouts.len() < 2
    {
        return Vec::new();
    }
    let mut done = vec![format!(
        "took the layouts {} from Hyprland",
        found.layouts.join(", ")
    )];
    editor.set_list("keyboard", "layouts", &found.layouts);
    match found.toggle.as_deref() {
        Some("alt_shift_toggle") => editor.set("keyboard", "switch", "alt-shift"),
        Some("ctrl_shift_toggle") => editor.set("keyboard", "switch", "ctrl-shift"),
        Some("caps_toggle") => editor.set("keyboard", "switch", "caps"),
        Some("win_space_toggle") => {
            editor.set("shortcuts", "layout", "Mod+Space");
            done.push("Super+Space switches them, as it did in Hyprland".to_owned());
            if crate::keys::same(&config.shortcuts.launcher, "Mod+Space") {
                editor.set("shortcuts", "launcher", "Mod+D");
                done.push(
                    "so the launcher moved to Super+D; Settings → Shortcuts changes it".to_owned(),
                );
            }
        }
        _ => {}
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apolline-hypr-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("hypr/conf.d")).unwrap();
        dir
    }

    #[test]
    fn the_keyboard_is_read_through_sourced_files() {
        let home = scratch("read");
        fs::write(
            home.join("hypr/hyprland.conf"),
            "input {\n    kb_layout = de\n}\nsource = ./conf.d/*.conf\n",
        )
        .unwrap();
        fs::write(
            home.join("hypr/conf.d/input.conf"),
            "input {\n    kb_layout = us, ru # mine\n    kb_options = caps:escape,grp:win_space_toggle\n}\n",
        )
        .unwrap();
        assert_eq!(
            keyboard(&home),
            Keyboard {
                layouts: vec!["us".into(), "ru".into()],
                toggle: Some("win_space_toggle".into()),
            }
        );
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn super_space_moves_to_layouts_and_the_launcher_steps_aside() {
        let found = Keyboard {
            layouts: vec!["us".into(), "ru".into()],
            toggle: Some("win_space_toggle".into()),
        };
        let mut editor = Editor::parse("").unwrap();
        let done = take_keyboard(&Config::default(), &found, &mut editor);
        assert_eq!(done.len(), 3);
        let (config, problems) = Config::parse(&editor.text());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert_eq!(config.shortcuts.layout, "Mod+Space");
        assert_eq!(config.shortcuts.launcher, "Mod+D");

        let mut again = Editor::parse(&editor.text()).unwrap();
        assert!(
            take_keyboard(&config, &found, &mut again).is_empty(),
            "only once"
        );
    }

    #[test]
    fn a_single_layout_is_left_alone() {
        let found = Keyboard {
            layouts: vec!["us".into()],
            toggle: None,
        };
        let mut editor = Editor::parse("").unwrap();
        assert!(take_keyboard(&Config::default(), &found, &mut editor).is_empty());
        assert_eq!(editor.text(), "");
    }
}

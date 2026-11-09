use std::fs;
use std::path::{Path, PathBuf};

use apolline_config::{Config, Editor};

use super::Keyboard;
use super::plan::{Plan, Setting};

const BACKUP: &str = "config.toml.before-hyprland";
const PREFIX: &str = "hyprland-";
const MARK: &str = "X-Apolline-From=hyprland";

// Where an import writes, so tests can point it elsewhere.
pub struct Paths {
    pub config: PathBuf,
    pub autostart: PathBuf,
}

impl Paths {
    pub fn real() -> Self {
        Self {
            config: apolline_config::default_path(),
            autostart: apolline_config::config_home().join("autostart"),
        }
    }

    fn backup(&self) -> PathBuf {
        self.config.with_file_name(BACKUP)
    }
}

// Writes the plan into Apolline's config, after a copy of it as it was before the first import,
// and the programs Hyprland started into autostart entries for niri. Says what it did.
pub fn apply(
    plan: &Plan,
    keyboard: &Keyboard,
    wallpaper: Option<&str>,
    paths: &Paths,
) -> Result<Vec<String>, String> {
    let backup = paths.backup();
    let mut done = Vec::new();
    if !backup.exists() {
        let before = fs::read_to_string(&paths.config).unwrap_or_default();
        if let Some(dir) = backup.parent() {
            fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        fs::write(&backup, before).map_err(|err| format!("{}: {err}", backup.display()))?;
        done.push(format!("kept the config as it was in {}", backup.display()));
    }
    let mut editor = Editor::load(&paths.config)?;
    let (config, _) = Config::load(&paths.config);
    done.extend(super::take_keyboard(&config, keyboard, &mut editor));
    write(&mut editor, &config, plan, wallpaper);
    editor
        .save(&paths.config)
        .map_err(|err| format!("{}: {err}", paths.config.display()))?;
    done.push(format!("wrote {}", paths.config.display()));
    for command in &plan.autostart {
        match start(&paths.autostart, command) {
            Ok(Some(path)) => done.push(format!("wrote {}", path.display())),
            Ok(None) => {}
            Err(err) => done.push(err),
        }
    }
    Ok(done)
}

fn write(editor: &mut Editor, config: &Config, plan: &Plan, wallpaper: Option<&str>) {
    if !plan.keys.is_empty() {
        let keys: Vec<_> = config.keys.iter().chain(&plan.keys).cloned().collect();
        editor.set_keys(&keys);
    }
    for (shortcut, combo) in &plan.shortcuts {
        editor.set("shortcuts", shortcut.key(), combo.as_str());
    }
    for display in &plan.displays {
        let name = &display.name;
        if let Some(mode) = &display.mode {
            editor.set_display(name, "mode", mode.as_str());
        }
        if let Some(scale) = display.scale {
            editor.set_display(name, "scale", scale);
        }
        if let Some(transform) = &display.transform {
            editor.set_display(name, "transform", transform.as_str());
        }
        if let Some((x, y)) = display.position {
            editor.set_display_position(name, x.into(), y.into());
        }
        if let Some(vrr) = display.vrr {
            editor.set_display(name, "vrr", vrr);
        }
    }
    for rule in &plan.rules {
        editor.add_rule(rule);
    }
    for setting in &plan.settings {
        match setting {
            Setting::Flag(section, key, on) => editor.set(section, key, *on),
            Setting::Number(section, key, n) => editor.set(section, key, *n),
            Setting::Fraction(section, key, n) => editor.set(section, key, *n),
            Setting::Text(section, key, text) => editor.set(section, key, text.as_str()),
        }
    }
    if let Some(wallpaper) = wallpaper {
        editor.set("desktop", "wallpaper", wallpaper);
    }
}

// An autostart entry for niri only, run through sh as Hyprland ran it. None when there's one.
fn start(dir: &Path, command: &str) -> Result<Option<PathBuf>, String> {
    let program = command
        .split_whitespace()
        .next()
        .map(|w| w.rsplit('/').next().unwrap_or(w))
        .unwrap_or("program");
    let path = dir.join(format!(
        "{PREFIX}{}.desktop",
        apolline_config::slug(program)
    ));
    if path.exists() {
        return Ok(None);
    }
    let escaped: String = command
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            '%' => vec!['%', '%'],
            c => vec![c],
        })
        .collect();
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName={program}\nComment=Started by Hyprland before, by niri now\nExec=sh -c \"{escaped}\"\nOnlyShowIn=niri;\n{MARK}\n"
    );
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&path, entry))
        .map(|()| Some(path.clone()))
        .map_err(|err| format!("{}: {err}", path.display()))
}

// The config back as it was before the first import, and the autostart entries it wrote gone.
pub fn undo(paths: &Paths) -> Result<Vec<String>, String> {
    let backup = paths.backup();
    if !backup.exists() {
        return Err("nothing was brought over from Hyprland here".into());
    }
    fs::rename(&backup, &paths.config).map_err(|err| format!("{}: {err}", backup.display()))?;
    let mut done = vec![format!("put {} back", paths.config.display())];
    let ours = fs::read_dir(&paths.autostart)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(PREFIX))
                && fs::read_to_string(path).is_ok_and(|text| text.contains(MARK))
        });
    for path in ours {
        match fs::remove_file(&path) {
            Ok(()) => done.push(format!("removed {}", path.display())),
            Err(err) => done.push(format!("{}: {err}", path.display())),
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypr::conf::Line;

    #[test]
    fn an_import_writes_a_config_that_reads_and_undo_puts_it_back() {
        let dir = std::env::temp_dir().join(format!("apolline-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = Paths {
            config: dir.join("apolline/config.toml"),
            autostart: dir.join("autostart"),
        };
        fs::create_dir_all(dir.join("apolline")).unwrap();
        fs::write(&paths.config, "# mine\n[bar]\nposition = \"bottom\"\n").unwrap();
        let line = |key: &str, value: &str| Line {
            key: key.into(),
            value: value.into(),
            at: "hyprland.conf:1".into(),
        };
        let lines = [
            line("bind", "SUPER, R, exec, rofi -show drun"),
            line("bind", "SUPER, C, killactive"),
            line("bind", "SUPER, Return, exec, kitty --title \"$HOME\""),
            line("monitor", "DP-1, 2560x1440@165, -2560x0, 1.25, vrr, 1"),
            line("windowrulev2", "float, class:^(pavucontrol)$"),
            line("exec-once", "fcitx5 -d --title \"$HOME\" 100%"),
            line("general:gaps_in", "5"),
            line("general:col.active_border", "rgba(33ccffee)"),
        ];
        let plan = crate::hypr::plan::plan(&lines, &[], &Config::default(), &[]);
        let keyboard = Keyboard {
            layouts: vec!["us".into(), "ru".into()],
            toggle: Some("alt_shift_toggle".into()),
        };
        let done = apply(&plan, &keyboard, Some("/walls/dunes.jpg"), &paths).unwrap();
        assert!(
            done.iter().any(|d| d.contains("hyprland-fcitx5.desktop")),
            "{done:?}"
        );

        let text = fs::read_to_string(&paths.config).unwrap();
        assert!(text.starts_with("# mine\n"), "{text}");
        let (config, problems) = Config::parse(&text);
        assert!(problems.is_empty(), "{problems:?}\n{text}");
        assert_eq!(config.shortcuts.launcher, "Mod+R");
        assert_eq!(config.keys.len(), 2);
        assert_eq!(config.displays[0].position, Some((-2560, 0)));
        assert_eq!(config.displays[0].vrr, Some(true));
        assert_eq!(config.rules[0].app, "pavucontrol");
        assert_eq!(config.layout.gaps, 10);
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert_eq!(config.desktop.wallpaper, "/walls/dunes.jpg");
        let entry = fs::read_to_string(dir.join("autostart/hyprland-fcitx5.desktop")).unwrap();
        assert!(
            entry.contains("Exec=sh -c \"fcitx5 -d --title \\\"\\$HOME\\\" 100%%\"\n"),
            "{entry}"
        );
        assert!(entry.contains("OnlyShowIn=niri;"));

        // A second import keeps the first copy, so undo goes back to before either.
        apply(&plan, &keyboard, None, &paths).unwrap();
        let done = undo(&paths).unwrap();
        assert_eq!(done.len(), 2, "{done:?}");
        assert_eq!(
            fs::read_to_string(&paths.config).unwrap(),
            "# mine\n[bar]\nposition = \"bottom\"\n"
        );
        assert!(!dir.join("autostart/hyprland-fcitx5.desktop").exists());
        assert!(undo(&paths).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}

// The launcher left Mod+Space, which Linux keeps for the keyboard layout, for Mod+A, and quick
// settings went to Mod+S. A config still on both old keys as they came gets the new ones, once:
// a mark in the state folder keeps a choice made later from being moved again.

use std::path::{Path, PathBuf};

use apolline_config::{Config, Editor};

use crate::keys::same;

pub fn mark() -> Option<PathBuf> {
    apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")
        .map(|state| state.join("apolline/keys-moved"))
}

// Whether the config was changed.
pub fn once(config: &Path, mark: &Path) -> Result<bool, String> {
    if mark.exists() {
        return Ok(false);
    }
    let (current, _) = Config::load(config);
    let shortcuts = &current.shortcuts;
    let untouched = same(&shortcuts.launcher, "Mod+Space") && same(&shortcuts.panel, "Mod+A");
    let free = !current.keys.iter().any(|(keys, _)| same(keys, "Mod+S"));
    let moving = config.exists() && untouched && free;
    if moving {
        let mut editor = Editor::load(config)?;
        editor.set("shortcuts", "launcher", "Mod+A");
        editor.set("shortcuts", "panel", "Mod+S");
        editor
            .save(config)
            .map_err(|err| format!("{}: {err}", config.display()))?;
    }
    if let Some(dir) = mark.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    std::fs::write(mark, "").map_err(|err| format!("{}: {err}", mark.display()))?;
    Ok(moving)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_keys_move_once_and_a_later_choice_stays() {
        let dir = std::env::temp_dir().join(format!("apolline-moved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        let mark = dir.join("state/keys-moved");
        std::fs::write(
            &config,
            "[shortcuts]\nlauncher = \"Mod+Space\"     # Mod is Super\npanel = \"Mod+A\"\n",
        )
        .unwrap();

        assert_eq!(once(&config, &mark), Ok(true));
        let (moved, _) = Config::load(&config);
        assert_eq!(moved.shortcuts.launcher, "Mod+A");
        assert_eq!(moved.shortcuts.panel, "Mod+S");
        assert!(
            std::fs::read_to_string(&config)
                .unwrap()
                .contains("# Mod is Super")
        );

        std::fs::write(
            &config,
            "[shortcuts]\nlauncher = \"Mod+Space\"\npanel = \"Mod+A\"\n",
        )
        .unwrap();
        assert_eq!(
            once(&config, &mark),
            Ok(false),
            "put back by hand, it stays"
        );

        std::fs::remove_file(&mark).unwrap();
        std::fs::write(
            &config,
            "[shortcuts]\nlauncher = \"Mod+Space\"\npanel = \"Mod+A\"\n[keys]\n\"Mod+S\" = \"foot\"\n",
        )
        .unwrap();
        assert_eq!(
            once(&config, &mark),
            Ok(false),
            "Mod+S is taken by a key of yours"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

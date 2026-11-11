use std::path::{Path, PathBuf};

use apolline_config::{Theme, slug};

const BUNDLED: [&str; 10] = [
    include_str!("../../../assets/themes/fog.toml"),
    include_str!("../../../assets/themes/ember.toml"),
    include_str!("../../../assets/themes/night.toml"),
    include_str!("../../../assets/themes/tonal.toml"),
    include_str!("../../../assets/themes/mocha.toml"),
    include_str!("../../../assets/themes/latte.toml"),
    include_str!("../../../assets/themes/hardline.toml"),
    include_str!("../../../assets/themes/paper.toml"),
    include_str!("../../../assets/themes/minimal.toml"),
    include_str!("../../../assets/themes/glass.toml"),
];

pub fn dir() -> PathBuf {
    apolline_config::config_home()
        .join("apolline")
        .join("themes")
}

pub fn all() -> Vec<(Theme, Option<PathBuf>)> {
    all_in(&dir())
}

fn all_in(dir: &Path) -> Vec<(Theme, Option<PathBuf>)> {
    let mut themes: Vec<(Theme, Option<PathBuf>)> = BUNDLED
        .iter()
        .filter_map(|text| Theme::parse(text).ok())
        .map(|theme| (theme, None))
        .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .map(|path| {
                    if path.is_dir() {
                        path.join("theme.toml")
                    } else {
                        path
                    }
                })
                .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "toml"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for file in files {
        let Some(theme) = read(&file) else {
            continue;
        };
        themes.retain(|(other, _)| slug(&other.name) != slug(&theme.name));
        themes.push((theme, Some(file)));
    }
    themes
}

fn read(file: &Path) -> Option<Theme> {
    let mut theme = Theme::parse(&std::fs::read_to_string(file).ok()?).ok()?;
    let own = file.parent()?.join(&theme.wallpaper);
    if !theme.wallpaper.contains('/') && own.is_file() {
        theme.wallpaper = crate::wallpaper::shorten(&own);
    }
    // The scene when Steam has it, the theme's own picture otherwise.
    if let Some(scene) = theme.scene.as_deref().and_then(crate::live::workshop) {
        theme.wallpaper = scene;
    }
    Some(theme)
}

// The looks a theme folder hands niri, from its own niri.kdl.
// The niri part of the theme the mode wears, else the desktop's.
pub fn niri_part_of(config: &apolline_config::Config, mode: Option<usize>) -> Option<String> {
    mode.and_then(|i| config.modes.get(i))
        .and_then(|m| m.theme.as_deref())
        .or(config.desktop.theme.as_deref())
        .and_then(niri_part)
}

pub fn niri_part(name: &str) -> Option<String> {
    let wanted = slug(name);
    let (_, file) = all()
        .into_iter()
        .find(|(theme, _)| slug(&theme.name) == wanted)?;
    let file = file.filter(|f| f.file_name().is_some_and(|n| n == "theme.toml"))?;
    let text = std::fs::read_to_string(file.with_file_name("niri.kdl")).ok()?;
    (text.len() <= 64 * 1024).then(|| crate::fragment::theme_part(&text))
}

pub fn install(path: &Path) -> Result<Theme, String> {
    install_into(&dir(), path)
}

pub fn install_from(link: &str) -> Result<Theme, String> {
    if !link.starts_with("https://") {
        return Err("a theme link starts with https://".into());
    }
    let bytes = crate::catalog::get(link, 256 * 1024)?;
    let text = String::from_utf8(bytes).map_err(|err| err.to_string())?;
    let theme = Theme::parse(&text)?;
    save_into(&dir(), &theme)?;
    Ok(theme)
}

fn install_into(dir: &Path, path: &Path) -> Result<Theme, String> {
    if !path.is_dir() {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let theme = Theme::parse(&text)?;
        save_into(dir, &theme)?;
        return Ok(theme);
    }
    let text = std::fs::read_to_string(path.join("theme.toml"))
        .map_err(|_| format!("{} has no theme.toml", path.display()))?;
    let theme = Theme::parse(&text)?;
    let name = slug(&theme.name);
    if name.is_empty() {
        return Err("the theme needs a name".into());
    }
    let target = dir.join(&name);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        if entry.path().is_file() {
            std::fs::copy(entry.path(), target.join(entry.file_name()))
                .map_err(|e| e.to_string())?;
        }
    }
    read(&target.join("theme.toml")).ok_or_else(|| "the copied theme doesn't read".into())
}

pub fn save(theme: &Theme) -> Result<PathBuf, String> {
    save_into(&dir(), theme)
}

fn save_into(dir: &Path, theme: &Theme) -> Result<PathBuf, String> {
    let name = slug(&theme.name);
    if name.is_empty() {
        return Err("give the theme a name".into());
    }
    let path = dir.join(format!("{name}.toml"));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, theme.text()).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_theme_parses() {
        let names: Vec<String> = BUNDLED
            .iter()
            .map(|text| Theme::parse(text).unwrap().name)
            .collect();
        assert_eq!(
            names,
            [
                "Fog", "Ember", "Night", "Tonal", "Mocha", "Latte", "Hardline", "Paper", "Minimal",
                "Glass"
            ]
        );
    }

    #[test]
    fn a_theme_folder_brings_its_own_wallpaper() {
        let root = std::env::temp_dir().join(format!("apolline-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let source = root.join("download/Dune");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("theme.toml"),
            "[theme]\nname = \"Dune\"\n\n[desktop]\nwallpaper = \"sand.jpg\"\naccent = \"#e0a060\"\n",
        )
        .unwrap();
        std::fs::write(source.join("sand.jpg"), b"jpg").unwrap();
        let themes = root.join("themes");

        let installed = install_into(&themes, &source).unwrap();
        assert!(
            installed.wallpaper.ends_with("themes/dune/sand.jpg"),
            "{}",
            installed.wallpaper
        );
        let listed = all_in(&themes);
        assert_eq!(listed.last().unwrap().0.name, "Dune");
        assert!(
            install_into(&themes, &root.join("download")).is_err(),
            "no theme.toml"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

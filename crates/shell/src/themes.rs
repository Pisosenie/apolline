use std::path::{Path, PathBuf};

use apolline_config::{Theme, slug};

const BUNDLED: [&str; 5] = [
    include_str!("../../../assets/themes/fog.toml"),
    include_str!("../../../assets/themes/ember.toml"),
    include_str!("../../../assets/themes/night.toml"),
    include_str!("../../../assets/themes/minimal.toml"),
    include_str!("../../../assets/themes/glass.toml"),
];

pub fn dir() -> PathBuf {
    apolline_config::config_home()
        .join("apolline")
        .join("themes")
}

pub fn all() -> Vec<(Theme, Option<PathBuf>)> {
    let mut themes: Vec<(Theme, Option<PathBuf>)> = BUNDLED
        .iter()
        .filter_map(|text| Theme::parse(text).ok())
        .map(|theme| (theme, None))
        .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir())
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    for file in files
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e == "toml"))
    {
        let Ok(theme) = std::fs::read_to_string(&file)
            .map_err(drop)
            .and_then(|t| Theme::parse(&t).map_err(drop))
        else {
            continue;
        };
        themes.retain(|(other, _)| slug(&other.name) != slug(&theme.name));
        themes.push((theme, Some(file)));
    }
    themes
}

pub fn save(theme: &Theme) -> Result<PathBuf, String> {
    let name = slug(&theme.name);
    if name.is_empty() {
        return Err("give the theme a name".into());
    }
    let path = dir().join(format!("{name}.toml"));
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(&path, theme.text()).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn import(path: &Path) -> Result<Theme, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let theme = Theme::parse(&text)?;
    save(&theme)?;
    Ok(theme)
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
        assert_eq!(names, ["Fog", "Ember", "Night", "Minimal", "Glass"]);
    }
}

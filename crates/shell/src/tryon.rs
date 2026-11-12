// Wearing a theme for a minute before keeping it. The config as it was waits beside it in
// config.toml.trying until the theme is kept or put back, so a crash or a closed Settings puts
// it back as well.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const LASTS: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq)]
pub struct Trying {
    pub theme: String,
    pub until: SystemTime,
    before: String,
    // The folder of a theme fetched from the catalog just to try, gone again unless it's kept.
    fetched: Option<String>,
}

impl Trying {
    pub fn left(&self) -> Duration {
        self.until
            .duration_since(SystemTime::now())
            .unwrap_or_default()
    }
}

fn file(config: &Path) -> PathBuf {
    config.with_file_name("config.toml.trying")
}

// What's tried now, if anything.
pub fn pending(config: &Path) -> Option<Trying> {
    let table: toml::Table = std::fs::read_to_string(file(config)).ok()?.parse().ok()?;
    let word = |key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let until = table.get("until").and_then(toml::Value::as_integer)?;
    Some(Trying {
        theme: word("theme")?,
        until: UNIX_EPOCH + Duration::from_secs(until.try_into().ok()?),
        before: word("before")?,
        fetched: word("fetched").filter(|name| plain(name)),
    })
}

// Called before the theme is worn, with the config as it is. Trying one theme after another
// keeps the config from before the first, and lets go of a fetched one that wasn't kept.
pub fn start(
    config: &Path,
    themes: &Path,
    theme: &str,
    before: &str,
    fetched: Option<String>,
) -> Result<Trying, String> {
    let earlier = pending(config);
    if let Some(old) = earlier.as_ref().and_then(|t| t.fetched.as_ref())
        && fetched.as_ref() != Some(old)
    {
        let _ = std::fs::remove_dir_all(themes.join(old));
    }
    let trying = Trying {
        theme: theme.to_owned(),
        until: SystemTime::now() + LASTS,
        before: earlier.map_or_else(|| before.to_owned(), |t| t.before),
        fetched,
    };
    let mut table = toml::Table::new();
    table.insert("theme".into(), trying.theme.clone().into());
    let until = trying
        .until
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_secs();
    table.insert(
        "until".into(),
        i64::try_from(until).unwrap_or(i64::MAX).into(),
    );
    if let Some(name) = &trying.fetched {
        table.insert("fetched".into(), name.clone().into());
    }
    table.insert("before".into(), trying.before.clone().into());
    let path = file(config);
    std::fs::write(&path, table.to_string()).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(trying)
}

pub fn keep(config: &Path) {
    let _ = std::fs::remove_file(file(config));
}

// The config back as it was before the first theme tried. The name of what was tried comes back.
pub fn put_back(config: &Path, themes: &Path) -> Result<Option<String>, String> {
    let Some(trying) = pending(config) else {
        keep(config);
        return Ok(None);
    };
    std::fs::write(config, &trying.before).map_err(|err| format!("{}: {err}", config.display()))?;
    if let Some(name) = &trying.fetched {
        let _ = std::fs::remove_dir_all(themes.join(name));
    }
    keep(config);
    Ok(Some(trying.theme))
}

fn plain(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_tried_goes_back_to_before_the_first_try_and_takes_fetched_themes_with_it() {
        let dir = std::env::temp_dir().join(format!("apolline-tryon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let themes = dir.join("themes");
        std::fs::create_dir_all(themes.join("dune")).unwrap();
        std::fs::create_dir_all(themes.join("moss")).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "# mine\n[bar]\nposition = \"bottom\"\n").unwrap();
        assert_eq!(put_back(&config, &themes), Ok(None));

        let first = start(
            &config,
            &themes,
            "Dune",
            "# mine\n[bar]\nposition = \"bottom\"\n",
            Some("dune".into()),
        )
        .unwrap();
        assert!(first.left() > Duration::from_secs(50));
        std::fs::write(&config, "[desktop]\ntheme = \"Dune\"\n").unwrap();
        start(
            &config,
            &themes,
            "Moss",
            "[desktop]\ntheme = \"Dune\"\n",
            Some("moss".into()),
        )
        .unwrap();
        assert!(
            !themes.join("dune").exists(),
            "the first fetched one isn't kept"
        );
        assert_eq!(pending(&config).unwrap().theme, "Moss");

        assert_eq!(put_back(&config, &themes), Ok(Some("Moss".into())));
        assert_eq!(
            std::fs::read_to_string(&config).unwrap(),
            "# mine\n[bar]\nposition = \"bottom\"\n"
        );
        assert!(!themes.join("moss").exists());
        assert_eq!(pending(&config), None);

        start(&config, &themes, "Fog", "x", None).unwrap();
        keep(&config);
        assert_eq!(put_back(&config, &themes), Ok(None), "kept stays");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_fetched_folder_never_reaches_outside_the_themes() {
        let dir = std::env::temp_dir().join(format!("apolline-tryon-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(
            dir.join("config.toml.trying"),
            "theme = \"X\"\nuntil = 1\nfetched = \"../../home\"\nbefore = \"\"\n",
        )
        .unwrap();
        assert_eq!(pending(&config).unwrap().fetched, None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

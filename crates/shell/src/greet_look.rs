use std::path::{Path, PathBuf};

use apolline_config::{Config, Editor, FontName};
use apolline_theme::Palette;

use crate::wallpaper::{self, Source};

// The greeter can't read anyone's home, so the look it wears lives here. It's an Apolline config:
// [desktop] wallpaper, accent and font and [lock] layout and blur are what it reads.
pub const SYSTEM: &str = "/etc/apolline/greet.toml";
const FILE: &str = "greet.toml";
const PICTURE: &str = "greet-wallpaper";
const FALLBACK: &str = "fog.jpg";

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub wallpaper: String,
    pub palette: Palette,
    pub font: Option<FontName>,
    pub centered: bool,
    pub blur: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            wallpaper: FALLBACK.to_owned(),
            palette: Palette::default(),
            font: None,
            centered: false,
            blur: false,
        }
    }
}

impl Look {
    pub fn read(path: &Path) -> Self {
        if !path.is_file() {
            return Self::default();
        }
        let (config, problems) = Config::load(path);
        for problem in &problems {
            eprintln!("{}:{}: {problem}", path.display(), problem.line);
        }
        let name = &config.desktop.wallpaper;
        // A picture named on its own sits next to the file.
        let beside = path
            .parent()
            .map(|dir| dir.join(name))
            .filter(|picture| !name.is_empty() && picture.is_file());
        Self {
            wallpaper: beside.map_or_else(|| name.clone(), |p| p.display().to_string()),
            palette: config.palette(None),
            font: config.desktop.font.as_deref().map(FontName::new),
            centered: config.lock.centered,
            blur: config.lock.blur,
        }
    }
}

// The desktop's own look as the greeter's file; `picture` is the wallpaper's name next to it.
pub fn text(config: &Config, picture: &str) -> String {
    let mut out = Editor::parse("").expect("an empty file parses");
    out.set("desktop", "wallpaper", picture);
    out.set("desktop", "accent", config.palette(None).accent.to_hex());
    if let Some(font) = &config.desktop.font {
        out.set("desktop", "font", font.as_str());
    }
    if config.lock.centered {
        out.set("lock", "layout", "center");
    }
    if config.lock.blur {
        out.set("lock", "blur", true);
    }
    format!(
        "# Written by Apolline Settings → Lock screen. The login screen reads it.\n\n{}",
        out.text()
    )
}

// What to call the wallpaper in the greeter's file, and the picture to copy beside it, if any.
fn picture(name: &str) -> (String, Option<PathBuf>) {
    match wallpaper::resolve(name) {
        Source::Bundled(name, _) => (name.to_owned(), None),
        Source::File(path) if ::image::ImageFormat::from_path(&path).is_ok() && path.is_file() => {
            let extension = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (format!("{PICTURE}.{extension}"), Some(path))
        }
        Source::File(_) => (FALLBACK.to_owned(), None),
    }
}

// Writes the greeter's files where only this user can see them; Settings previews and installs
// from there.
pub fn stage(config: &Config) -> Result<Vec<PathBuf>, String> {
    let dir = crate::control::runtime_dir().join("apolline-greet");
    let failed = |err: std::io::Error| format!("{}: {err}", dir.display());
    std::fs::create_dir_all(&dir).map_err(failed)?;
    for old in std::fs::read_dir(&dir).map_err(failed)?.flatten() {
        let _ = std::fs::remove_file(old.path());
    }
    let (name, copy) = picture(config.wallpaper(None));
    let mut files = vec![dir.join(FILE)];
    if let Some(from) = copy {
        let to = dir.join(&name);
        std::fs::copy(&from, &to).map_err(|err| format!("{}: {err}", from.display()))?;
        files.push(to);
    }
    std::fs::write(&files[0], text(config, &name)).map_err(failed)?;
    Ok(files)
}

// Only `install` runs as root: the files are already written.
pub async fn install(files: Vec<PathBuf>) -> Result<(), String> {
    let dir = Path::new(SYSTEM)
        .parent()
        .unwrap_or(Path::new("/etc/apolline"));
    let out = tokio::process::Command::new("pkexec")
        .args(["install", "-D", "-m", "644", "-t"])
        .arg(dir)
        .args(&files)
        .output()
        .await
        .map_err(|err| format!("pkexec: {err}"))?;
    match out.status.code() {
        Some(0) => Ok(()),
        Some(126) => Err("Not changed: the password was cancelled".into()),
        Some(127) => Err("Not changed: this account can't change the login screen".into()),
        _ => Err(String::from_utf8_lossy(&out.stderr).trim().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_greeter_reads_back_what_settings_wrote() {
        let dir = std::env::temp_dir().join(format!("apolline-greet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut config, _) = Config::parse(
            "[desktop]\naccent = \"#ff6a3d\"\nfont = \"Inter\"\n[lock]\nlayout = \"center\"\nblur = true\n",
        );
        config.desktop.wallpaper = "ember.jpg".into();
        let file = dir.join(FILE);
        std::fs::write(&file, text(&config, "ember.jpg")).unwrap();
        let look = Look::read(&file);
        assert_eq!(look.wallpaper, "ember.jpg");
        assert_eq!(look.palette, config.palette(None));
        assert_eq!(look.font.map(FontName::name), Some("Inter"));
        assert!(look.centered && look.blur);

        std::fs::write(dir.join("greet-wallpaper.png"), b"").unwrap();
        std::fs::write(&file, text(&Config::default(), "greet-wallpaper.png")).unwrap();
        let look = Look::read(&file);
        assert_eq!(
            look.wallpaper,
            dir.join("greet-wallpaper.png").display().to_string()
        );
        assert!(!look.centered && !look.blur && look.font.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_file_is_the_plain_look() {
        assert_eq!(
            Look::read(Path::new("/nonexistent/greet.toml")),
            Look::default()
        );
    }

    #[test]
    fn shipped_wallpapers_go_by_name_and_videos_fall_back() {
        assert_eq!(picture("night.jpg"), ("night.jpg".to_owned(), None));
        assert_eq!(
            picture("/nonexistent/clip.mp4"),
            (FALLBACK.to_owned(), None)
        );
    }
}

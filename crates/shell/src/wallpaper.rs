use std::path::{Path, PathBuf};

use iced::widget::image::Handle;

const BUNDLED: [(&str, &[u8]); 3] = [
    (
        "fog.jpg",
        include_bytes!("../../../assets/wallpapers/fog.jpg"),
    ),
    (
        "ember.jpg",
        include_bytes!("../../../assets/wallpapers/ember.jpg"),
    ),
    (
        "night.jpg",
        include_bytes!("../../../assets/wallpapers/night.jpg"),
    ),
];

pub fn bundled() -> &'static [(&'static str, &'static [u8])] {
    &BUNDLED
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    File(PathBuf),
    Bundled(&'static str, &'static [u8]),
}

pub fn expand(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else if let (Some(rest), Some(home)) = (name.strip_prefix("~/"), std::env::var_os("HOME")) {
        Some(PathBuf::from(home).join(rest))
    } else {
        None
    }
}

pub fn shorten(path: &Path) -> String {
    match std::env::var_os("HOME").and_then(|home| path.strip_prefix(home).ok().map(PathBuf::from))
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub fn library(folders: &[String]) -> Vec<String> {
    let mut dirs = search_dirs();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join("Pictures/Wallpapers"));
        dirs.push(home.join("Pictures/wallpapers"));
    }
    dirs.extend(folders.iter().filter_map(|folder| expand(folder)));
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(e.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp")
                })
            })
            .collect();
        files.sort();
        for file in files {
            let name = shorten(&file);
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found.truncate(LIBRARY);
    found
}

const LIBRARY: usize = 300;

pub fn resolve(name: &str) -> Source {
    let path = Path::new(name);
    let file = if let Some(file) = expand(name) {
        Some(file)
    } else {
        search_dirs()
            .into_iter()
            .map(|dir| dir.join(name))
            .find(|p| p.exists())
    };
    match file {
        Some(file) if file.exists() => Source::File(file),
        _ => match BUNDLED.iter().find(|(bundled, _)| *bundled == name) {
            Some(&(name, bytes)) => Source::Bundled(name, bytes),
            None => Source::File(file.unwrap_or_else(|| path.to_path_buf())),
        },
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let mut dirs: Vec<PathBuf> = data
        .map(|d| d.join("apolline/wallpapers"))
        .into_iter()
        .collect();
    dirs.push(PathBuf::from("/usr/share/apolline/wallpapers"));
    dirs
}

pub async fn load(name: String) -> Result<Handle, String> {
    let decode = move || {
        let source = resolve(&name);
        let image = match &source {
            Source::File(path) => {
                ::image::open(path).map_err(|err| format!("{}: {err}", path.display()))
            }
            Source::Bundled(name, bytes) => {
                ::image::load_from_memory(bytes).map_err(|err| format!("{name}: {err}"))
            }
        }?;
        if matches!(source, Source::File(_)) {
            learn(&name, &image.thumbnail(THUMB.0, THUMB.1).to_rgba8());
        }
        let image = match screen() {
            Some((width, height)) if image.width() > width && image.height() > height => {
                image.resize_to_fill(width, height, ::image::imageops::FilterType::Triangle)
            }
            _ => image,
        };
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Handle::from_rgba(width, height, rgba.into_raw()))
    };
    tokio::task::spawn_blocking(decode)
        .await
        .map_err(|err| err.to_string())?
}

pub async fn thumbnail(name: String) -> Option<Handle> {
    let decode = move || {
        let source = resolve(&name);
        let image = match &source {
            Source::File(path) => ::image::open(path).ok()?,
            Source::Bundled(_, bytes) => ::image::load_from_memory(bytes).ok()?,
        };
        let thumb = image.thumbnail(THUMB.0, THUMB.1).to_rgba8();
        if matches!(source, Source::File(_)) {
            learn(&name, &thumb);
        }
        let (width, height) = thumb.dimensions();
        Some(Handle::from_rgba(width, height, thumb.into_raw()))
    };
    tokio::task::spawn_blocking(decode).await.ok().flatten()
}

const THUMB: (u32, u32) = (760, 475);

fn screen() -> Option<(u32, u32)> {
    std::env::var_os("NIRI_SOCKET")?;
    let output = std::process::Command::new("niri")
        .args(["msg", "-j", "outputs"])
        .output()
        .ok()?;
    apolline_niri::largest_mode(&String::from_utf8_lossy(&output.stdout))
}

fn learn(name: &str, thumb: &::image::RgbaImage) {
    if let Some(accent) = apolline_theme::accent_of(thumb.as_raw()) {
        apolline_config::remember_accent(name, accent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_names_fall_back_to_the_bundled_images() {
        assert!(matches!(
            resolve("ember.jpg"),
            Source::Bundled("ember.jpg", _)
        ));
        assert_eq!(
            resolve("/nowhere/mine.jpg"),
            Source::File("/nowhere/mine.jpg".into())
        );
        assert_eq!(resolve("mine.jpg"), Source::File("mine.jpg".into()));
    }

    #[test]
    fn a_file_on_disk_wins_over_the_bundled_one() {
        let path = std::env::temp_dir().join(format!("apolline-wall-{}.jpg", std::process::id()));
        std::fs::write(&path, BUNDLED[0].1).unwrap();
        assert_eq!(resolve(path.to_str().unwrap()), Source::File(path.clone()));
        std::fs::remove_file(path).unwrap();
    }
}

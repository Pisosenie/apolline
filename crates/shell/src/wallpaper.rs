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

pub fn resolve(name: &str) -> Source {
    let path = Path::new(name);
    let file = if path.is_absolute() {
        Some(path.to_path_buf())
    } else if let (Some(rest), Some(home)) = (name.strip_prefix("~/"), std::env::var_os("HOME")) {
        Some(PathBuf::from(home).join(rest))
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

pub async fn load(source: Source) -> Result<Handle, String> {
    let decode = move || {
        let image = match &source {
            Source::File(path) => {
                ::image::open(path).map_err(|err| format!("{}: {err}", path.display()))
            }
            Source::Bundled(name, bytes) => {
                ::image::load_from_memory(bytes).map_err(|err| format!("{name}: {err}"))
            }
        }?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Handle::from_rgba(width, height, rgba.into_raw()))
    };
    tokio::task::spawn_blocking(decode)
        .await
        .map_err(|err| err.to_string())?
}

pub async fn thumbnail(source: Source) -> Option<Handle> {
    let decode = move || {
        let image = match &source {
            Source::File(path) => ::image::open(path).ok()?,
            Source::Bundled(_, bytes) => ::image::load_from_memory(bytes).ok()?,
        };
        let thumb = image.thumbnail(760, 475).to_rgba8();
        let (width, height) = thumb.dimensions();
        Some(Handle::from_rgba(width, height, thumb.into_raw()))
    };
    tokio::task::spawn_blocking(decode).await.ok().flatten()
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

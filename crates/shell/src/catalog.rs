use std::path::Path;

use apolline_config::{Theme, slug};
use iced::widget::image::Handle;

// Themes anyone can add with a pull request: a folder each under catalog/, listed in index.toml.
const HOME: &str = "https://raw.githubusercontent.com/Pisosenie/apolline/main/catalog";
const INDEX_LIMIT: u64 = 256 * 1024;
const FILE_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub author: String,
    pub about: String,
    pub folder: String,
    pub files: Vec<String>,
    pub preview: Option<String>,
}

// APOLLINE_CATALOG points somewhere else, a local server while making themes.
fn home() -> String {
    std::env::var("APOLLINE_CATALOG").unwrap_or_else(|_| HOME.to_owned())
}

fn get(link: &str, limit: u64) -> Result<Vec<u8>, String> {
    ureq::get(link)
        .call()
        .map_err(|err| err.to_string())?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|err| err.to_string())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| err.to_string())?
}

pub async fn fetch() -> Result<Vec<Entry>, String> {
    let link = format!("{}/index.toml", home());
    blocking(move || {
        let bytes = get(&link, INDEX_LIMIT).map_err(|err| format!("{link}: {err}"))?;
        parse(&String::from_utf8_lossy(&bytes))
    })
    .await
}

pub async fn preview(entry: Entry) -> Option<Handle> {
    let name = entry.preview?;
    let link = format!("{}/{}/{name}", home(), entry.folder);
    blocking(move || get(&link, FILE_LIMIT).map(Handle::from_bytes))
        .await
        .ok()
}

// Into the themes folder, the same as a folder added by hand; the theme's name comes back.
pub async fn install(entry: Entry) -> Result<String, String> {
    blocking(move || install_into(&crate::themes::dir(), &entry, |link| get(link, FILE_LIMIT)))
        .await
}

fn install_into(
    dir: &Path,
    entry: &Entry,
    get: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    let base = format!("{}/{}", home(), entry.folder);
    let mut fetched = Vec::new();
    for name in &entry.files {
        let bytes = get(&format!("{base}/{name}")).map_err(|err| format!("{name}: {err}"))?;
        fetched.push((name, bytes));
    }
    let theme = fetched
        .iter()
        .find(|(name, _)| name.as_str() == "theme.toml")
        .ok_or("the theme has no theme.toml")?;
    let theme = Theme::parse(&String::from_utf8_lossy(&theme.1))?;
    let target = dir.join(slug(&theme.name));
    std::fs::create_dir_all(&target).map_err(|err| err.to_string())?;
    for (name, bytes) in fetched {
        std::fs::write(target.join(name), bytes).map_err(|err| format!("{name}: {err}"))?;
    }
    Ok(theme.name)
}

// A plain name, so nothing in the index can write outside the theme's own folder.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
}

fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let table: toml::Table = text.parse().map_err(|err| format!("the catalog: {err}"))?;
    let themes = table
        .get("theme")
        .and_then(toml::Value::as_array)
        .ok_or("the catalog lists no [[theme]]")?;
    let word = |t: &toml::Table, key: &str| {
        t.get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Ok(themes
        .iter()
        .filter_map(toml::Value::as_table)
        .filter_map(|t| {
            let files: Vec<String> = t
                .get("files")?
                .as_array()?
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect();
            let folder = word(t, "folder");
            let preview = Some(word(t, "preview")).filter(|p| plain(p));
            let fine = plain(&folder)
                && files.iter().all(|f| plain(f))
                && files.iter().any(|f| f == "theme.toml");
            fine.then(|| Entry {
                name: word(t, "name"),
                author: word(t, "author"),
                about: word(t, "about"),
                folder,
                files,
                preview,
            })
        })
        .filter(|entry| !entry.name.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"
[[theme]]
name = "Dune"
author = "Apolline"
about = "Warm sand, square corners"
folder = "dune"
files = ["theme.toml", "dune.jpg"]
preview = "preview.jpg"

[[theme]]
name = "Sneaky"
folder = "../../.ssh"
files = ["theme.toml"]

[[theme]]
name = "Sneakier"
folder = "ok"
files = ["theme.toml", "../evil"]
"#;

    #[test]
    fn the_index_keeps_only_themes_that_stay_in_their_folder() {
        let entries = parse(INDEX).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Dune");
        assert_eq!(entries[0].files, ["theme.toml", "dune.jpg"]);
        assert_eq!(entries[0].preview.as_deref(), Some("preview.jpg"));
        assert!(parse("nothing = 1").is_err());
    }

    #[test]
    fn an_installed_theme_lands_in_a_folder_named_after_it() {
        let dir = std::env::temp_dir().join(format!("apolline-catalog-{}", std::process::id()));
        let entry = parse(INDEX).unwrap().remove(0);
        let name = install_into(&dir, &entry, |link| {
            Ok(if link.ends_with("theme.toml") {
                b"[theme]\nname = \"Dune\"\n\n[desktop]\nwallpaper = \"dune.jpg\"\n".to_vec()
            } else {
                b"picture".to_vec()
            })
        })
        .unwrap();
        assert_eq!(name, "Dune");
        assert!(dir.join("dune/theme.toml").is_file());
        assert!(dir.join("dune/dune.jpg").is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

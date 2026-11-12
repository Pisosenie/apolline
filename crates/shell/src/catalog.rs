use std::path::Path;
use std::time::Duration;

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

// A server that trickles or never answers gives up in time, like the calendars.
pub fn get(link: &str, limit: u64) -> Result<Vec<u8>, String> {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
        .get(link)
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
    blocking(move || {
        let taken: Vec<String> = crate::themes::all()
            .iter()
            .map(|(theme, _)| slug(&theme.name))
            .collect();
        install_into(&crate::themes::dir(), &entry, &taken, |link| {
            get(link, FILE_LIMIT)
        })
    })
    .await
}

const MOST_FILES: usize = 16;
const MOST_BYTES: usize = 48 * 1024 * 1024;

fn install_into(
    dir: &Path,
    entry: &Entry,
    taken: &[String],
    get: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    if entry.files.len() > MOST_FILES {
        return Err("the theme has too many files".into());
    }
    let base = format!("{}/{}", home(), entry.folder);
    let mut fetched = Vec::new();
    let mut total = 0;
    for name in &entry.files {
        let bytes = get(&format!("{base}/{name}")).map_err(|err| format!("{name}: {err}"))?;
        total += bytes.len();
        if total > MOST_BYTES {
            return Err("the theme is too big".into());
        }
        fetched.push((name, bytes));
    }
    let theme = fetched
        .iter()
        .find(|(name, _)| name.as_str() == "theme.toml")
        .ok_or("the theme has no theme.toml")?;
    let theme = Theme::parse(&String::from_utf8_lossy(&theme.1))?;
    let name = slug(&theme.name);
    if name.is_empty() || name != slug(&entry.name) {
        return Err(crate::i18n::fill(
            "the catalog lists {listed} but the theme calls itself {named}",
            &[("listed", &entry.name), ("named", &theme.name)],
        ));
    }
    let target = dir.join(&name);
    if taken.contains(&name) || target.exists() {
        return Err(crate::i18n::fill(
            "a theme called {theme} is already here",
            &[("theme", &theme.name)],
        ));
    }
    // All of it or nothing: written aside, then moved in at once.
    let part = dir.join(format!(".{name}.part"));
    let written = (|| -> std::io::Result<()> {
        if part.exists() {
            std::fs::remove_dir_all(&part)?;
        }
        std::fs::create_dir_all(&part)?;
        for (file, bytes) in &fetched {
            std::fs::write(part.join(file), bytes)?;
        }
        std::fs::rename(&part, &target)
    })();
    if let Err(err) = written {
        let _ = std::fs::remove_dir_all(&part);
        return Err(err.to_string());
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

    // What a pull request to the catalog has to hold to: every folder listed, every file there,
    // the theme reading, and no Workshop content copied in, only linked by its id.
    #[test]
    fn every_theme_in_the_catalog_reads_and_brings_what_it_lists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
        let text = std::fs::read_to_string(root.join("index.toml")).unwrap();
        let entries = parse(&text).unwrap();
        assert_eq!(
            entries.len(),
            text.matches("[[theme]]").count(),
            "an entry was dropped as unsafe"
        );
        let mut names: Vec<String> = entries.iter().map(|e| slug(&e.name)).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), entries.len(), "two themes share a name");
        let mut folders: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        folders.sort();
        let mut listed: Vec<String> = entries.iter().map(|e| e.folder.clone()).collect();
        listed.sort();
        assert_eq!(
            folders, listed,
            "a folder isn't in index.toml, or the other way"
        );

        for entry in &entries {
            let folder = root.join(&entry.folder);
            let there: Vec<String> = std::fs::read_dir(&folder)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            for file in &there {
                let lower = file.to_lowercase();
                assert!(
                    !(lower == "project.json"
                        || lower.ends_with(".pkg")
                        || crate::live::source(&folder.join(file).to_string_lossy()).is_some()),
                    "{}: {file} — a Workshop scene goes in by its id, [theme] scene",
                    entry.folder
                );
                assert!(
                    entry.files.contains(file) || entry.preview.as_ref() == Some(file),
                    "{}: {file} isn't listed",
                    entry.folder
                );
            }
            assert!(entry.files.len() <= MOST_FILES, "{}", entry.folder);
            let mut bytes = 0;
            for file in &entry.files {
                let path = folder.join(file);
                assert!(path.is_file(), "{}: no {file}", entry.folder);
                bytes += std::fs::metadata(path).unwrap().len();
            }
            assert!(bytes <= MOST_BYTES as u64, "{} is too big", entry.folder);
            let preview = entry.preview.as_ref().expect("a preview to show");
            assert!(
                folder.join(preview).is_file(),
                "{}: no {preview}",
                entry.folder
            );
            let theme = Theme::parse(&std::fs::read_to_string(folder.join("theme.toml")).unwrap())
                .unwrap_or_else(|err| panic!("{}: {err}", entry.folder));
            assert_eq!(slug(&theme.name), slug(&entry.name), "{}", entry.folder);
            let bundled = crate::wallpaper::bundled()
                .iter()
                .any(|(name, _)| *name == theme.wallpaper);
            assert!(
                bundled || entry.files.contains(&theme.wallpaper),
                "{}: its wallpaper {} isn't one of its files",
                entry.folder,
                theme.wallpaper
            );
        }
    }

    #[test]
    fn an_installed_theme_lands_in_a_folder_named_after_it() {
        let dir = std::env::temp_dir().join(format!("apolline-catalog-{}", std::process::id()));
        let entry = parse(INDEX).unwrap().remove(0);
        let fetch = |link: &str| -> Result<Vec<u8>, String> {
            Ok(if link.ends_with("theme.toml") {
                b"[theme]\nname = \"Dune\"\n\n[desktop]\nwallpaper = \"dune.jpg\"\n".to_vec()
            } else {
                b"picture".to_vec()
            })
        };
        let name = install_into(&dir, &entry, &[], fetch).unwrap();
        assert_eq!(name, "Dune");
        assert!(dir.join("dune/theme.toml").is_file());
        assert!(dir.join("dune/dune.jpg").is_file());
        let again = install_into(&dir, &entry, &[], fetch).unwrap_err();
        assert!(again.contains("already here"), "{again}");
        let fog = Entry {
            name: "Fog".into(),
            ..entry.clone()
        };
        assert!(
            install_into(&dir, &fog, &["fog".into()], fetch).is_err(),
            "names must match"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

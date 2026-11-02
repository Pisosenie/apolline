use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub id: String,
    pub name: String,
    pub about: String,
    pub exec: String,
    pub icon: Option<Picture>,
    pub game: bool,
    pub keywords: String,
    pub class: Option<String>,
    // What the entry offers besides starting it, like a private window: (name, command).
    pub actions: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picture {
    Raster(PathBuf),
    Vector(PathBuf),
}

pub async fn scan(theme: Option<String>) -> Vec<App> {
    tokio::task::spawn_blocking(move || {
        let dirs = data_dirs();
        let themes = icon_roots(&dirs, theme.as_deref());
        let mut seen = HashSet::new();
        let mut apps = Vec::new();
        for dir in &dirs {
            let Ok(entries) = std::fs::read_dir(dir.join("applications")) else {
                continue;
            };
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            for file in files {
                let Some(id) = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".desktop"))
                else {
                    continue;
                };
                if !seen.insert(id.to_owned()) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                if let Some(mut app) = parse(id, &text) {
                    app.icon = app.icon.take().and_then(|icon| match icon {
                        Picture::Raster(name) => find_icon(&name, &themes),
                        vector => Some(vector),
                    });
                    apps.push(app);
                }
            }
        }
        apps.sort_by_key(|app| app.name.to_lowercase());
        apps
    })
    .await
    .unwrap_or_default()
}

pub fn sample() -> Vec<App> {
    let app = |name: &str, about: &str, game: bool| App {
        id: name.to_lowercase(),
        name: name.to_owned(),
        about: about.to_owned(),
        exec: name.to_lowercase(),
        icon: None,
        game,
        keywords: String::new(),
        class: None,
        actions: Vec::new(),
    };
    let firefox = App {
        actions: vec![
            ("New Window".into(), "firefox --new-window".into()),
            (
                "New Private Window".into(),
                "firefox --private-window".into(),
            ),
        ],
        ..app("Firefox", "Web Browser", false)
    };
    vec![
        app("Baldur's Gate 3", "Game", true),
        app("Counter-Strike 2", "Game", true),
        app("Hades II", "Game", true),
        app("Discord", "Internet Messenger", false),
        firefox,
        app("Files", "File Manager", false),
        app("Foot", "Terminal", false),
        app("Obsidian", "Notes", false),
        app("Steam", "Game library", false),
        app("Telegram", "Messenger", false),
        app("Visual Studio Code", "Code Editor", false),
        app("Zed", "Code Editor", false),
    ]
}

fn parse(id: &str, text: &str) -> Option<App> {
    let field = |want: &str| section(text, "[Desktop Entry]", want);
    let flag = |value: Option<String>| value.as_deref() == Some("true");

    if field("Type").as_deref() != Some("Application")
        || flag(field("NoDisplay"))
        || flag(field("Hidden"))
        || flag(field("Terminal"))
    {
        return None;
    }
    let name = field("Name")?;
    let exec = clean_exec(&field("Exec")?);
    if exec.is_empty() {
        return None;
    }
    let categories = field("Categories").unwrap_or_default();
    let game = categories.split(';').any(|c| c == "Game") || exec.contains("steam://rungameid/");
    let about = field("GenericName")
        .or_else(|| field("Comment"))
        .unwrap_or_default();
    let icon = field("Icon").filter(|i| !i.is_empty()).map(|icon| {
        let path = PathBuf::from(&icon);
        match path.extension().and_then(|e| e.to_str()) {
            Some("svg") if path.is_absolute() => Picture::Vector(path),
            Some("png") if path.is_absolute() => Picture::Raster(path),
            _ => Picture::Raster(PathBuf::from(icon)),
        }
    });
    Some(App {
        id: id.to_owned(),
        name,
        about,
        exec,
        icon,
        game,
        keywords: field("Keywords").unwrap_or_default().replace(';', " "),
        class: field("StartupWMClass"),
        actions: field("Actions")
            .unwrap_or_default()
            .split(';')
            .filter(|action| !action.trim().is_empty())
            .filter_map(|action| {
                let header = format!("[Desktop Action {}]", action.trim());
                let exec = clean_exec(&section(text, &header, "Exec")?);
                Some((section(text, &header, "Name")?, exec)).filter(|(_, e)| !e.is_empty())
            })
            .collect(),
    })
}

// A key's value in one [section] of a desktop file.
fn section(text: &str, header: &str, want: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == want
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|part| !(part.len() == 2 && part.starts_with('%')))
        .collect::<Vec<_>>()
        .join(" ")
        .replace("%%", "%")
}

fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")))
        .into_iter()
        .collect();
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(system.split(':').map(PathBuf::from));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    dirs
}

fn icon_roots(dirs: &[PathBuf], chosen: Option<&str>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| {
            chosen
                .into_iter()
                .chain(["hicolor", "Papirus", "Adwaita"])
                .map(move |theme| dir.join("icons").join(theme))
        })
        .filter(|root| root.is_dir())
        .collect();
    roots.push(PathBuf::from("/usr/share/pixmaps"));
    roots
}

pub fn icon_themes() -> Vec<String> {
    let mut found: Vec<String> = data_dirs()
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir.join("icons")).ok())
        .flat_map(|entries| entries.flatten())
        .filter(|entry| {
            let path = entry.path();
            path.join("index.theme").is_file()
                && ["scalable", "48x48", "64x64", "128x128"]
                    .iter()
                    .any(|size| path.join(size).join("apps").is_dir())
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "hicolor")
        .collect();
    found.sort();
    found.dedup();
    found
}

// A named icon from the chosen theme or the usual ones, for things that aren't apps, like the
// icons in the tray; `extra` is a folder of the app's own to look in first.
pub fn icon_named(name: &str, theme: Option<&str>, extra: Option<&Path>) -> Option<Picture> {
    let mut roots: Vec<PathBuf> = extra.into_iter().map(Path::to_path_buf).collect();
    roots.extend(icon_roots(&data_dirs(), theme));
    find_icon(Path::new(name), &roots).or_else(|| {
        let flat = extra?;
        ["svg", "png"]
            .iter()
            .map(|ext| flat.join(format!("{name}.{ext}")))
            .find(|file| file.exists())
            .map(picture)
    })
}

fn find_icon(name: &Path, roots: &[PathBuf]) -> Option<Picture> {
    if name.is_absolute() {
        return name.exists().then(|| Picture::Raster(name.to_path_buf()));
    }
    let name = name.to_str()?;
    const SIZES: [&str; 7] = [
        "scalable", "256x256", "128x128", "96x96", "64x64", "512x512", "48x48",
    ];
    for root in roots {
        if root.ends_with("pixmaps") {
            for ext in ["svg", "png"] {
                let file = root.join(format!("{name}.{ext}"));
                if file.exists() {
                    return Some(picture(file));
                }
            }
            continue;
        }
        for size in SIZES {
            for kind in ["apps", "status"] {
                for ext in ["svg", "png"] {
                    let file = root.join(size).join(kind).join(format!("{name}.{ext}"));
                    if file.exists() {
                        return Some(picture(file));
                    }
                }
            }
        }
    }
    None
}

fn picture(file: PathBuf) -> Picture {
    if file.extension().is_some_and(|e| e == "svg") {
        Picture::Vector(file)
    } else {
        Picture::Raster(file)
    }
}

pub fn find<'a>(apps: &'a [App], app_id: &str) -> Option<&'a App> {
    let wanted = app_id.to_lowercase();
    apps.iter().find(|app| {
        let id = app.id.to_lowercase();
        id == wanted
            || id.rsplit('.').next() == Some(wanted.as_str())
            || app
                .class
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(app_id))
            || app.exec.split_whitespace().next().is_some_and(|e| {
                e.rsplit('/')
                    .next()
                    .is_some_and(|e| e.eq_ignore_ascii_case(app_id))
            })
    })
}

pub fn score(query: &str, app: &App) -> Option<u32> {
    let extra = format!("{} {} {}", app.about, app.keywords, app.id);
    rank(query, &app.name, &extra)
}

// An app's action shows only when asked for: by its own name ("private"), below the apps, or
// after the app's ("firefox priv"), first.
pub fn action_score(query: &str, app: &App, action: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.len() < 3 {
        return None;
    }
    let app_name = app.name.to_lowercase();
    let after_app = app_name
        .split_whitespace()
        .chain([app.id.as_str()])
        .find_map(|word| query.strip_prefix(&word.to_lowercase()))
        .map(str::trim)
        .filter(|rest| !rest.is_empty());
    if let Some(rest) = after_app
        && rank(rest, action, "").is_some_and(|score| score >= 300)
    {
        return Some(1100);
    }
    rank(&query, action, "")
        .filter(|score| *score >= 300)
        .map(|score| score - 200)
}

pub fn rank(query: &str, name: &str, extra: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let name = name.to_lowercase();
    let short = 500 - name.len().min(500) as u32;
    if name.starts_with(&query) {
        return Some(500 + short);
    }
    if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(&query))
    {
        return Some(300 + short);
    }
    if name.contains(&query) {
        return Some(300);
    }
    if extra.to_lowercase().contains(&query) {
        return Some(200);
    }
    subsequence(&query, &name).then_some(100)
}

fn subsequence(query: &str, text: &str) -> bool {
    let mut chars = text.chars();
    query
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|q| chars.any(|c| c == q))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAM_GAME: &str = "[Desktop Entry]
Name=Hades II
Comment=Play this game on Steam
Exec=steam steam://rungameid/1145350
Icon=steam_icon_1145350
Terminal=false
Type=Application
Categories=Game;
";

    #[test]
    fn steam_games_are_games() {
        let app = parse("Hades II", STEAM_GAME).unwrap();
        assert!(app.game);
        assert_eq!(app.exec, "steam steam://rungameid/1145350");
        assert_eq!(
            app.icon,
            Some(Picture::Raster(PathBuf::from("steam_icon_1145350")))
        );
    }

    #[test]
    fn hidden_and_terminal_entries_are_skipped() {
        let hidden = "[Desktop Entry]\nType=Application\nName=x\nExec=x\nNoDisplay=true\n";
        let terminal = "[Desktop Entry]\nType=Application\nName=htop\nExec=htop\nTerminal=true\n";
        assert_eq!(parse("x", hidden), None);
        assert_eq!(parse("htop", terminal), None);
    }

    #[test]
    fn actions_do_not_override_the_main_entry() {
        let text = "[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox %u\nActions=new-window;private;\n\n[Desktop Action new-window]\nName=New Window\nExec=firefox --new-window %u\n\n[Desktop Action private]\nName=New Private Window\nExec=firefox --private-window %u\n\n[Desktop Action unlisted]\nName=Hidden\nExec=firefox --x\n";
        let app = parse("firefox", text).unwrap();
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(
            app.actions,
            [
                ("New Window".to_owned(), "firefox --new-window".to_owned()),
                (
                    "New Private Window".to_owned(),
                    "firefox --private-window".to_owned()
                ),
            ],
            "only the ones the entry lists"
        );
    }

    #[test]
    fn names_beat_descriptions_and_typos_still_match() {
        let apps = sample();
        let find = |name: &str| apps.iter().find(|a| a.name == name).unwrap();
        let code = find("Visual Studio Code");
        let zed = find("Zed");
        assert!(score("code", code) > score("code", zed));
        assert!(score("zed", zed) > score("zed", code));
        assert!(score("vsc", code).is_some());
        assert_eq!(score("qqq", code), None);
        assert!(score("fire", find("Firefox")) > score("fire", find("Files")));
    }
}

use std::path::{Path, PathBuf};

const DEPTH: usize = 5;
const LIMIT: usize = 40_000;
const SKIP: [&str; 6] = [
    "node_modules",
    "target",
    "venv",
    "__pycache__",
    "build",
    "dist",
];

// A file or folder under home, and which it is, as the walk found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub dir: bool,
}

pub async fn index() -> Vec<Found> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        walk(&home, 0, &mut found);
        found
    })
    .await
    .unwrap_or_default()
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<Found>) {
    if depth > DEPTH || found.len() >= LIMIT {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP.contains(&name.as_ref()) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            dirs.push(path.clone());
        }
        found.push(Found {
            path,
            dir: kind.is_dir(),
        });
        if found.len() >= LIMIT {
            return;
        }
    }
    for dir in dirs {
        walk(&dir, depth + 1, found);
    }
}

pub fn find<'a>(index: &'a [Found], query: &str) -> Vec<&'a Found> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, &Found)> = index
        .iter()
        .filter_map(|found| {
            let path = &found.path;
            let name = path.file_name()?.to_string_lossy().to_lowercase();
            let score = if name == query {
                0
            } else if name.starts_with(&query) {
                1
            } else if name.contains(&query) {
                2
            } else {
                return None;
            };
            Some((score * 1000 + path.components().count(), found))
        })
        .collect();
    hits.sort_by_key(|(score, found)| (*score, found.path.as_os_str().len()));
    hits.into_iter().take(30).map(|(_, found)| found).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_start_with_the_query_come_first_and_shallow_wins() {
        let index: Vec<Found> = [
            "/home/a/Documents/old/report-2024.pdf",
            "/home/a/report.txt",
            "/home/a/Documents/annual-report.odt",
            "/home/a/Music/song.mp3",
        ]
        .map(|path| Found {
            path: PathBuf::from(path),
            dir: false,
        })
        .to_vec();
        let found = find(&index, "report");
        assert_eq!(found[0].path, PathBuf::from("/home/a/report.txt"));
        assert_eq!(
            found[1].path,
            PathBuf::from("/home/a/Documents/old/report-2024.pdf")
        );
        assert_eq!(found.len(), 3);
        assert!(find(&index, "  ").is_empty());
    }
}

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

pub async fn index() -> Vec<PathBuf> {
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

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
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
        found.push(path);
        if found.len() >= LIMIT {
            return;
        }
    }
    for dir in dirs {
        walk(&dir, depth + 1, found);
    }
}

pub fn find<'a>(index: &'a [PathBuf], query: &str) -> Vec<&'a PathBuf> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, &PathBuf)> = index
        .iter()
        .filter_map(|path| {
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
            Some((score * 1000 + path.components().count(), path))
        })
        .collect();
    hits.sort_by_key(|(score, path)| (*score, path.as_os_str().len()));
    hits.into_iter().take(30).map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_start_with_the_query_come_first_and_shallow_wins() {
        let index: Vec<PathBuf> = [
            "/home/a/Documents/old/report-2024.pdf",
            "/home/a/report.txt",
            "/home/a/Documents/annual-report.odt",
            "/home/a/Music/song.mp3",
        ]
        .map(PathBuf::from)
        .to_vec();
        let found = find(&index, "report");
        assert_eq!(found[0], &PathBuf::from("/home/a/report.txt"));
        assert_eq!(
            found[1],
            &PathBuf::from("/home/a/Documents/old/report-2024.pdf")
        );
        assert_eq!(found.len(), 3);
        assert!(find(&index, "  ").is_empty());
    }
}

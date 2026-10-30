use std::path::{Path, PathBuf};

pub fn resolve(name: &str) -> PathBuf {
    let path = Path::new(name);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if let (Some(rest), Some(home)) = (name.strip_prefix("~/"), std::env::var_os("HOME")) {
        return PathBuf::from(home).join(rest);
    }
    search_dirs()
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| path.to_path_buf())
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
    if cfg!(debug_assertions) {
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/wallpapers"));
    }
    dirs
}

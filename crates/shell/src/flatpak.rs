use std::fs;
use std::path::{Path, PathBuf};

// Flatpak apps see ~/.config only when allowed; these let them read the colors and nothing else.
const GRANTS: [&str; 2] = ["xdg-config/gtk-3.0:ro", "xdg-config/gtk-4.0:ro"];

fn overrides(data: &Path) -> PathBuf {
    data.join("flatpak/overrides/global")
}

// What Apolline added itself, so turning coloring off takes back only that.
fn ledger() -> Option<PathBuf> {
    let state = apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")?;
    Some(state.join("apolline/flatpak-grants"))
}

fn installed(data: &Path) -> bool {
    data.join("flatpak").is_dir() || Path::new("/var/lib/flatpak").is_dir()
}

pub fn grant(data: &Path) -> Option<String> {
    if !installed(data) {
        return None;
    }
    let path = overrides(data);
    let text = fs::read_to_string(&path).unwrap_or_default();
    // One added before and gone now was taken out by hand, and stays out.
    let given = ledger().map(|l| read_ledger(&l)).unwrap_or_default();
    let wanted: Vec<&str> = GRANTS
        .into_iter()
        .filter(|grant| !given.iter().any(|g| g == grant))
        .collect();
    let (changed, added) = with(&text, &wanted, &[]);
    if added.is_empty() {
        return None;
    }
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&path, changed));
    if let Err(err) = written {
        return Some(format!("{}: {err}", path.display()));
    }
    if let Some(ledger) = ledger() {
        let mut kept = read_ledger(&ledger);
        kept.extend(added);
        let _ = ledger
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&ledger, kept.join("\n") + "\n"));
    }
    None
}

pub fn revoke(data: &Path) -> bool {
    let Some(ledger) = ledger() else {
        return false;
    };
    let ours = read_ledger(&ledger);
    if ours.is_empty() {
        return false;
    }
    let path = overrides(data);
    if let Ok(text) = fs::read_to_string(&path) {
        let ours: Vec<&str> = ours.iter().map(String::as_str).collect();
        let (changed, _) = with(&text, &[], &ours);
        let _ = fs::write(&path, changed);
    }
    let _ = fs::remove_file(ledger);
    true
}

fn read_ledger(ledger: &Path) -> Vec<String> {
    fs::read_to_string(ledger)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

// The keyfile with `add` in [Context] filesystems and `drop` out of it, and what was really added.
fn with(text: &str, add: &[&str], drop: &[&str]) -> (String, Vec<String>) {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let context = match lines.iter().position(|l| l.trim() == "[Context]") {
        Some(at) => at,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Context]".into());
            lines.len() - 1
        }
    };
    let end = lines[context + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |at| context + 1 + at);
    let value = |line: &str| {
        line.split_once('=')
            .filter(|(key, _)| key.trim() == "filesystems")
            .map(|(_, value)| value.trim().to_owned())
    };
    let found = lines[context + 1..end]
        .iter()
        .position(|l| value(l).is_some())
        .map(|at| context + 1 + at);
    let mut entries: Vec<String> = found
        .and_then(|at| value(&lines[at]))
        .unwrap_or_default()
        .split(';')
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
        .collect();
    // Whatever someone already says about a folder, writable or denied, stands.
    let added: Vec<String> = add
        .iter()
        .filter(|grant| !entries.iter().any(|e| folder(e) == folder(grant)))
        .map(|grant| (*grant).to_owned())
        .collect();
    entries.extend(added.iter().cloned());
    entries.retain(|e| !drop.contains(&e.as_str()));
    let line = format!("filesystems={};", entries.join(";"));
    match (found, entries.is_empty()) {
        (Some(at), true) => {
            lines.remove(at);
        }
        (Some(at), false) => lines[at] = line,
        (None, false) => lines.insert(end, line),
        (None, true) => {}
    }
    (lines.join("\n") + "\n", added)
}

// The folder an entry is about, without its access or a leading `!`.
fn folder(entry: &str) -> &str {
    let entry = entry.trim().trim_start_matches('!');
    match entry.rsplit_once(':') {
        Some((path, "ro" | "rw" | "create")) => path,
        _ => entry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_join_what_is_there_and_leave_with_only_their_own() {
        let theirs = "[Context]\nfilesystems=~/Music:ro;\n\n[Environment]\nGTK_THEME=Adwaita\n";
        let (granted, added) = with(theirs, &GRANTS, &[]);
        assert_eq!(added, GRANTS);
        assert!(
            granted.contains("filesystems=~/Music:ro;xdg-config/gtk-3.0:ro;xdg-config/gtk-4.0:ro;")
        );
        assert!(granted.contains("GTK_THEME=Adwaita"));
        let (back, _) = with(&granted, &[], &GRANTS);
        assert_eq!(back, theirs);
    }

    #[test]
    fn writable_or_denied_folders_are_left_as_they_are() {
        let text = "[Context]\nfilesystems = xdg-config/gtk-3.0;!xdg-config/gtk-4.0;\n";
        let (granted, added) = with(text, &GRANTS, &[]);
        assert!(added.is_empty());
        assert_eq!(granted.matches("filesystems").count(), 1);
    }

    #[test]
    fn a_grant_someone_already_made_is_not_counted_as_ours() {
        let (_, added) = with(
            "[Context]\nfilesystems=xdg-config/gtk-4.0:ro;\n",
            &GRANTS,
            &[],
        );
        assert_eq!(added, ["xdg-config/gtk-3.0:ro"]);
    }

    #[test]
    fn an_empty_file_gets_a_context_and_loses_it_again() {
        let (granted, _) = with("", &GRANTS, &[]);
        assert_eq!(
            granted,
            "[Context]\nfilesystems=xdg-config/gtk-3.0:ro;xdg-config/gtk-4.0:ro;\n"
        );
        let (back, _) = with(&granted, &[], &GRANTS);
        assert_eq!(back, "[Context]\n");
    }
}

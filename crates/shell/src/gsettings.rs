use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

const SCHEMA: &str = "org.gnome.desktop.interface";

enum Change {
    Set(String, String),
    Restore(String),
    // Away and back, so running apps take the key up again: it isn't written down as Apolline's.
    Nudge(String),
}

// What a key held before Apolline touched it, and what Apolline put there.
#[derive(Clone, Debug, PartialEq)]
struct Kept {
    key: String,
    theirs: String,
    ours: String,
}

pub fn set(key: &str, value: &str) {
    send(Change::Set(key.to_owned(), value.to_owned()));
}

pub fn restore(key: &str) {
    send(Change::Restore(key.to_owned()));
}

// Running GTK apps read their CSS again when the theme changes, so it changes and comes back.
pub fn nudge(key: &str) {
    send(Change::Nudge(key.to_owned()));
}

// One worker, so a set and a restore of the same key land in the order they were asked.
fn send(change: Change) {
    static WORKER: OnceLock<Sender<Change>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for change in rx {
                if let Change::Nudge(key) = &change {
                    if let Some(now) = get(key) {
                        put(key, "''");
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        put(key, &now);
                    }
                    continue;
                }
                let path = ledger();
                let mut kept = path
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .map_or_else(Vec::new, |text| read(&text));
                let before = kept.clone();
                let wanted = apply(change, &mut kept, get);
                // The ledger first: a value set before it's written down would be lost to a crash.
                if kept != before
                    && let Some(path) = &path
                {
                    let saved = path
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| std::fs::write(path, write(&kept)));
                    if let Err(err) = saved {
                        eprintln!("apolline: {}: {err}", path.display());
                    }
                }
                if let Some((key, value)) = wanted {
                    put(&key, &value);
                }
            }
        });
        tx
    });
    let _ = worker.send(change);
}

// Updates the ledger and says what to set, if anything.
fn apply(
    change: Change,
    kept: &mut Vec<Kept>,
    get: impl Fn(&str) -> Option<String>,
) -> Option<(String, String)> {
    match change {
        Change::Set(key, value) => {
            let now = get(&key);
            match kept.iter_mut().find(|k| k.key == key) {
                Some(entry) => {
                    // Changed by hand since Apolline last set it: that's theirs now.
                    if let Some(now) = now.filter(|now| bare(now) != bare(&entry.ours)) {
                        entry.theirs = now;
                    }
                    entry.ours = value.clone();
                }
                None => kept.push(Kept {
                    key: key.clone(),
                    theirs: now?,
                    ours: value.clone(),
                }),
            }
            Some((key, value))
        }
        Change::Restore(key) => {
            let at = kept.iter().position(|k| k.key == key)?;
            let entry = kept.remove(at);
            // Someone changed it since; theirs wins.
            get(&key)
                .is_some_and(|now| bare(&now) == bare(&entry.ours))
                .then_some((key, entry.theirs))
        }
        // Handled before the ledger is read.
        Change::Nudge(_) => None,
    }
}

fn bare(value: &str) -> &str {
    value.trim().trim_matches('\'')
}

fn get(key: &str) -> Option<String> {
    let out = Command::new("gsettings")
        .args(["get", SCHEMA, key])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn put(key: &str, value: &str) {
    let _ = Command::new("gsettings")
        .args(["set", SCHEMA, key, value])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn ledger() -> Option<PathBuf> {
    let state = apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")?;
    Some(state.join("apolline").join("gsettings"))
}

fn read(text: &str) -> Vec<Kept> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            Some(Kept {
                key: fields.next()?.to_owned(),
                theirs: fields.next()?.to_owned(),
                ours: fields.next()?.to_owned(),
            })
        })
        .collect()
}

fn write(kept: &[Kept]) -> String {
    kept.iter()
        .map(|k| format!("{}\t{}\t{}\n", k.key, k.theirs, k.ours))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[test]
    fn the_value_you_had_comes_back() {
        let store = RefCell::new(HashMap::from([(
            "color-scheme".to_owned(),
            "'prefer-dark'".to_owned(),
        )]));
        let mut kept = Vec::new();
        let mut run = |change: Change| {
            let get = |key: &str| store.borrow().get(key).cloned();
            if let Some((key, value)) = apply(change, &mut kept, get) {
                store
                    .borrow_mut()
                    .insert(key, format!("'{}'", bare(&value)));
            }
        };
        let now = || store.borrow()["color-scheme"].clone();
        let set = |value: &str| Change::Set("color-scheme".into(), value.into());
        let restore = || Change::Restore("color-scheme".into());

        run(set("default"));
        run(set("prefer-light"));
        assert_eq!(now(), "'prefer-light'");
        run(restore());
        assert_eq!(
            now(),
            "'prefer-dark'",
            "the first value, not the middle one"
        );

        run(set("default"));
        store
            .borrow_mut()
            .insert("color-scheme".into(), "'prefer-light'".into());
        run(restore());
        assert_eq!(now(), "'prefer-light'", "a later change by hand stays");

        run(set("default"));
        store
            .borrow_mut()
            .insert("color-scheme".into(), "'prefer-dark'".into());
        run(set("default"));
        run(restore());
        assert_eq!(
            now(),
            "'prefer-dark'",
            "a hand change between two sets becomes theirs"
        );

        run(Change::Restore("gtk-theme".into()));
        assert!(
            !store.borrow().contains_key("gtk-theme"),
            "a key never set is left alone"
        );
    }

    #[test]
    fn the_ledger_survives_a_restart() {
        let kept = vec![Kept {
            key: "cursor-size".into(),
            theirs: "24".into(),
            ours: "32".into(),
        }];
        assert_eq!(read(&write(&kept)), kept);
    }
}

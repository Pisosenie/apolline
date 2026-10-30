use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

const SCHEMA: &str = "org.gnome.desktop.interface";

enum Change {
    Set(String, String),
    Restore(String),
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

// One worker, so a set and a restore of the same key land in the order they were asked.
fn send(change: Change) {
    static WORKER: OnceLock<Sender<Change>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for change in rx {
                let Some(path) = ledger() else { continue };
                let mut kept = read(&std::fs::read_to_string(&path).unwrap_or_default());
                let before = kept.clone();
                apply(change, &mut kept, get, put);
                if kept != before {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = std::fs::write(&path, write(&kept));
                }
            }
        });
        tx
    });
    let _ = worker.send(change);
}

fn apply(
    change: Change,
    kept: &mut Vec<Kept>,
    get: impl Fn(&str) -> Option<String>,
    mut put: impl FnMut(&str, &str),
) {
    match change {
        Change::Set(key, value) => {
            match kept.iter_mut().find(|k| k.key == key) {
                Some(entry) => entry.ours = value.clone(),
                None => {
                    let Some(theirs) = get(&key) else { return };
                    kept.push(Kept {
                        key: key.clone(),
                        theirs,
                        ours: value.clone(),
                    });
                }
            }
            put(&key, &value);
        }
        Change::Restore(key) => {
            let Some(at) = kept.iter().position(|k| k.key == key) else {
                return;
            };
            let entry = kept.remove(at);
            // Someone changed it since; theirs wins.
            if get(&key).is_some_and(|now| bare(&now) == bare(&entry.ours)) {
                put(&key, &entry.theirs);
            }
        }
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
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
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
        let get = |key: &str| store.borrow().get(key).cloned();
        let put = |key: &str, value: &str| {
            store
                .borrow_mut()
                .insert(key.to_owned(), format!("'{}'", bare(value)));
        };
        let mut kept = Vec::new();

        apply(
            Change::Set("color-scheme".into(), "default".into()),
            &mut kept,
            get,
            put,
        );
        apply(
            Change::Set("color-scheme".into(), "prefer-light".into()),
            &mut kept,
            get,
            put,
        );
        assert_eq!(store.borrow()["color-scheme"], "'prefer-light'");
        assert_eq!(read(&write(&kept)), kept, "the ledger survives a restart");
        apply(Change::Restore("color-scheme".into()), &mut kept, get, put);
        assert_eq!(
            store.borrow()["color-scheme"],
            "'prefer-dark'",
            "the first value, not the middle one"
        );
        assert!(kept.is_empty());

        apply(
            Change::Set("color-scheme".into(), "default".into()),
            &mut kept,
            get,
            put,
        );
        store
            .borrow_mut()
            .insert("color-scheme".into(), "'prefer-light'".into());
        apply(Change::Restore("color-scheme".into()), &mut kept, get, put);
        assert_eq!(
            store.borrow()["color-scheme"],
            "'prefer-light'",
            "a later change by hand stays"
        );

        apply(Change::Restore("gtk-theme".into()), &mut kept, get, put);
        assert!(
            !store.borrow().contains_key("gtk-theme"),
            "a key never set is left alone"
        );
    }
}

use std::path::Path;

// Tray applets that draw what the bar already has, in an older icon set.
const DOUBLES: [(&str, &str); 2] = [
    ("nm-applet.desktop", "Wi-Fi"),
    ("blueman.desktop", "Bluetooth"),
];

// Once, at setup: the system starts these for every session, so a copy of the user's own that
// leaves out niri keeps them out of this one and in any other desktop. One the user has
// already written stays as it is.
pub fn set_up(config: &Path, system: &Path) -> Vec<String> {
    let mut done = Vec::new();
    for (file, what) in DOUBLES {
        let own = config.join("autostart").join(file);
        let Ok(entry) = std::fs::read_to_string(system.join(file)) else {
            continue;
        };
        if own.exists() {
            continue;
        }
        let wrote = std::fs::create_dir_all(config.join("autostart"))
            .and_then(|()| std::fs::write(&own, not_in_niri(&entry)));
        done.push(match wrote {
            Ok(()) => format!(
                "wrote {}, so {what} shows once, on Apolline's bar",
                own.display()
            ),
            Err(err) => format!("{}: {err}", own.display()),
        });
    }
    done
}

// The entry as it is, with niri added to the desktops it doesn't start in.
fn not_in_niri(entry: &str) -> String {
    let mut out = String::with_capacity(entry.len() + 20);
    let mut said = false;
    let mut main = false;
    for line in entry.lines() {
        if line.starts_with('[') {
            if main && !said {
                out.push_str("NotShowIn=niri;\n");
                said = true;
            }
            main = line.trim() == "[Desktop Entry]";
        }
        match line.strip_prefix("NotShowIn=") {
            Some(list) if main => {
                let list = list.trim_end_matches(';');
                out.push_str(&format!("NotShowIn={list};niri;\n"));
                said = true;
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    if main && !said {
        out.push_str("NotShowIn=niri;\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_applets_the_system_starts_and_the_user_left_alone_are_hidden() {
        let dir = std::env::temp_dir().join(format!("apolline-autostart-{}", std::process::id()));
        let system = dir.join("xdg");
        let config = dir.join("config");
        std::fs::create_dir_all(&system).unwrap();
        std::fs::create_dir_all(config.join("autostart")).unwrap();
        std::fs::write(system.join("nm-applet.desktop"), "[Desktop Entry]\n").unwrap();
        std::fs::write(system.join("blueman.desktop"), "[Desktop Entry]\n").unwrap();
        std::fs::write(config.join("autostart/blueman.desktop"), "mine\n").unwrap();

        let done = set_up(&config, &system);
        assert_eq!(done.len(), 1, "{done:?}");
        assert_eq!(
            std::fs::read_to_string(config.join("autostart/nm-applet.desktop")).unwrap(),
            "[Desktop Entry]\nNotShowIn=niri;\n"
        );
        assert_eq!(
            std::fs::read_to_string(config.join("autostart/blueman.desktop")).unwrap(),
            "mine\n"
        );
        assert!(set_up(&config, &system).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn niri_joins_the_desktops_an_entry_leaves_out_and_other_sections_stay() {
        assert_eq!(
            not_in_niri("[Desktop Entry]\nExec=nm-applet\nNotShowIn=KDE;GNOME;\n"),
            "[Desktop Entry]\nExec=nm-applet\nNotShowIn=KDE;GNOME;niri;\n"
        );
        assert_eq!(
            not_in_niri("[Desktop Entry]\nExec=a\n[Desktop Action b]\nExec=b\n"),
            "[Desktop Entry]\nExec=a\nNotShowIn=niri;\n[Desktop Action b]\nExec=b\n"
        );
    }
}

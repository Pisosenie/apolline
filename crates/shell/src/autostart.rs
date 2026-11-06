use std::path::Path;

// Tray applets that draw what the bar already has, in an older icon set.
const DOUBLES: [(&str, &str); 2] = [
    ("nm-applet.desktop", "Wi-Fi"),
    ("blueman.desktop", "Bluetooth"),
];

// Once, at setup: the system starts these for every session, so a Hidden entry of the user's own
// keeps them out of this one. One the user has already written stays as it is.
pub fn set_up(config: &Path, system: &Path) -> Vec<String> {
    let mut done = Vec::new();
    for (file, what) in DOUBLES {
        let own = config.join("autostart").join(file);
        if !system.join(file).exists() || own.exists() {
            continue;
        }
        let wrote = std::fs::create_dir_all(config.join("autostart"))
            .and_then(|()| std::fs::write(&own, "[Desktop Entry]\nHidden=true\n"));
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
            "[Desktop Entry]\nHidden=true\n"
        );
        assert_eq!(
            std::fs::read_to_string(config.join("autostart/blueman.desktop")).unwrap(),
            "mine\n"
        );
        assert!(set_up(&config, &system).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

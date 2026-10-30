use std::fs;
use std::path::{Path, PathBuf};

use apolline_theme::{Palette, Rgba};

const CSS: &str = "apolline.css";
const IMPORT: &str = "@import url(\"apolline.css\");";

pub fn apply(palette: Palette) -> Vec<String> {
    let Some(config) = config_home() else {
        return Vec::new();
    };
    let notes = write(&config, palette);
    gsettings("color-scheme", "prefer-dark");
    gsettings("accent-color", gnome_accent(palette.accent));
    notes
}

pub fn remove() {
    if let Some(config) = config_home() {
        erase(&config);
    }
}

fn write(config: &Path, palette: Palette) -> Vec<String> {
    let mut notes = Vec::new();
    for version in ["gtk-3.0", "gtk-4.0"] {
        let dir = config.join(version);
        if let Err(err) =
            fs::create_dir_all(&dir).and_then(|()| fs::write(dir.join(CSS), gtk_css(palette)))
        {
            notes.push(format!("{}: {err}", dir.display()));
            continue;
        }
        if let Some(note) = import(&dir.join("gtk.css")) {
            notes.push(note);
        }
    }
    let qt = config.join("qt6ct");
    if qt.is_dir() {
        let colors = qt.join("colors");
        let _ = fs::create_dir_all(&colors)
            .and_then(|()| fs::write(colors.join("apolline.conf"), qt_colors(palette)));
    }
    notes
}

fn erase(config: &Path) {
    for version in ["gtk-3.0", "gtk-4.0"] {
        let dir = config.join(version);
        let _ = fs::remove_file(dir.join(CSS));
        let gtk = dir.join("gtk.css");
        if gtk.is_symlink() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&gtk)
            && text.contains(IMPORT)
        {
            let kept: String = text
                .lines()
                .filter(|line| line.trim() != IMPORT)
                .map(|line| format!("{line}\n"))
                .collect();
            let _ = if kept.trim().is_empty() {
                fs::remove_file(&gtk)
            } else {
                fs::write(&gtk, kept)
            };
        }
    }
    let _ = fs::remove_file(config.join("qt6ct/colors/apolline.conf"));
}

pub fn files() -> Vec<String> {
    vec![
        "~/.config/gtk-3.0/apolline.css".into(),
        "~/.config/gtk-4.0/apolline.css".into(),
        format!("{IMPORT} at the top of gtk.css"),
        "~/.config/qt6ct/colors/apolline.conf, if you use qt6ct".into(),
        "gsettings color-scheme and accent-color".into(),
    ]
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

fn import(gtk: &Path) -> Option<String> {
    if gtk.is_symlink() {
        return Some(format!(
            "{} is a link, so it was left alone; add {IMPORT} to it yourself",
            gtk.display()
        ));
    }
    let text = fs::read_to_string(gtk).unwrap_or_default();
    if text.lines().any(|line| line.trim() == IMPORT) {
        return None;
    }
    fs::write(gtk, format!("{IMPORT}\n{text}"))
        .err()
        .map(|err| format!("{}: {err}", gtk.display()))
}

fn gtk_css(palette: Palette) -> String {
    let accent = palette.accent.to_hex();
    let on = palette.on_accent.to_hex();
    format!(
        "/* Written by Apolline from your accent. Turn off \"Color apps\" in its settings to remove it. */
@define-color accent_color {accent};
@define-color accent_bg_color {accent};
@define-color accent_fg_color {on};
@define-color theme_selected_bg_color {accent};
@define-color theme_selected_fg_color {on};
"
    )
}

fn qt_colors(palette: Palette) -> String {
    let argb = |c: Rgba| format!("#ff{}", c.to_hex().trim_start_matches('#'));
    let text = Rgba::hex(0xf4f3f0);
    let roles = [
        text,
        Rgba::hex(0x2a2c31),
        Rgba::hex(0x3a3d44),
        Rgba::hex(0x33363c),
        Rgba::hex(0x141619),
        Rgba::hex(0x23252a),
        text,
        Rgba::hex(0xffffff),
        text,
        Rgba::hex(0x17191d),
        Rgba::hex(0x1e2024),
        Rgba::hex(0x000000),
        palette.accent,
        palette.on_accent,
        palette.accent,
        palette.accent.shift_hue(40.0),
        Rgba::hex(0x1b1d21),
        Rgba::hex(0x000000),
        Rgba::hex(0x2a2c31),
        text,
        Rgba::hex(0x8a8c90),
    ];
    let active: Vec<String> = roles.iter().map(|c| argb(*c)).collect();
    let faded: Vec<String> = roles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if matches!(i, 0 | 6 | 8) {
                argb(Rgba::hex(0x8a8c90))
            } else {
                argb(*c)
            }
        })
        .collect();
    format!(
        "[ColorScheme]\nactive_colors={}\ndisabled_colors={}\ninactive_colors={}\n",
        active.join(", "),
        faded.join(", "),
        active.join(", ")
    )
}

fn gnome_accent(accent: Rgba) -> &'static str {
    let [r, g, b] = [accent.r, accent.g, accent.b].map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    if delta < 0.12 {
        return "slate";
    }
    let hue = if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    const GNOME: [(&str, f32); 8] = [
        ("blue", 213.0),
        ("teal", 189.0),
        ("green", 131.0),
        ("yellow", 41.0),
        ("orange", 23.0),
        ("red", 353.0),
        ("pink", 331.0),
        ("purple", 285.0),
    ];
    let distance = |to: f32| {
        let d = (hue - to).abs();
        d.min(360.0 - d)
    };
    GNOME
        .into_iter()
        .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
        .map_or("blue", |(name, _)| name)
}

fn gsettings(key: &str, value: &str) {
    let _ = std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", key, value])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolline_theme::ACCENTS;

    #[test]
    fn the_import_goes_on_top_once_and_leaves_with_the_file() {
        let dir = std::env::temp_dir().join(format!("apolline-paint-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("gtk-4.0")).unwrap();
        let gtk = dir.join("gtk-4.0/gtk.css");
        fs::write(&gtk, "window { border: none; }\n").unwrap();
        write(&dir, Palette::new(ACCENTS[3]));
        write(&dir, Palette::new(ACCENTS[3]));
        let text = fs::read_to_string(&gtk).unwrap();
        assert_eq!(text, format!("{IMPORT}\nwindow {{ border: none; }}\n"));
        assert!(
            fs::read_to_string(dir.join("gtk-4.0/apolline.css"))
                .unwrap()
                .contains("#c084fc")
        );
        erase(&dir);
        assert_eq!(
            fs::read_to_string(&gtk).unwrap(),
            "window { border: none; }\n"
        );
        assert!(!dir.join("gtk-4.0/apolline.css").exists());
        assert!(
            !dir.join("gtk-3.0/gtk.css").exists(),
            "a gtk.css we made is gone too"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn accents_map_to_gnome_names() {
        assert_eq!(gnome_accent(ACCENTS[0]), "blue");
        assert_eq!(gnome_accent(ACCENTS[1]), "orange");
        assert_eq!(gnome_accent(ACCENTS[3]), "purple");
        assert_eq!(gnome_accent(ACCENTS[4]), "green");
        assert_eq!(gnome_accent(ACCENTS[5]), "pink");
    }
}

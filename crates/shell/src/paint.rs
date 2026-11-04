use std::fs;
use std::path::{Path, PathBuf};

use apolline_theme::{Palette, Rgba, Scheme};

use crate::gsettings;

const CSS: &str = "apolline.css";
const IMPORT: &str = "@import url(\"apolline.css\");";
const KDE: &str = "Apolline.colors";

pub fn apply(palette: Palette, scheme: &Scheme) -> Vec<String> {
    let (Some(config), Some(data)) = (config_home(), data_home()) else {
        return Vec::new();
    };
    let before = read_css(&config);
    let notes = write(&config, &data, palette, scheme);
    if read_css(&config) != before {
        gsettings::nudge("gtk-theme");
    }
    gsettings::set(
        "color-scheme",
        if scheme.light {
            "prefer-light"
        } else {
            "prefer-dark"
        },
    );
    gsettings::set("accent-color", gnome_accent(palette.accent));
    notes
}

fn read_css(config: &Path) -> Option<String> {
    fs::read_to_string(config.join("gtk-4.0").join(CSS)).ok()
}

pub fn remove() {
    if let (Some(config), Some(data)) = (config_home(), data_home()) {
        erase(&config, &data);
    }
    gsettings::restore("color-scheme");
    gsettings::restore("accent-color");
}

fn write(config: &Path, data: &Path, palette: Palette, scheme: &Scheme) -> Vec<String> {
    let mut notes = Vec::new();
    for version in ["gtk-3.0", "gtk-4.0"] {
        let dir = config.join(version);
        if dir.is_symlink() {
            notes.push(format!("{} is a link, so it was left alone", dir.display()));
            continue;
        }
        if let Err(err) = fs::create_dir_all(&dir)
            .and_then(|()| fs::write(dir.join(CSS), gtk_css(palette, scheme)))
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
            .and_then(|()| fs::write(colors.join("apolline.conf"), qt_colors(palette, scheme)));
    }
    // KDE apps and qt6ct both list schemes from here; picking it stays your call.
    if config.join("kdeglobals").is_file() || qt.is_dir() {
        let schemes = data.join("color-schemes");
        if let Err(err) = fs::create_dir_all(&schemes)
            .and_then(|()| fs::write(schemes.join(KDE), kde_colors(palette, scheme)))
        {
            notes.push(format!("{}: {err}", schemes.display()));
        }
    }
    notes.extend(crate::flatpak::grant(data));
    notes
}

fn erase(config: &Path, data: &Path) -> bool {
    let mut ours = false;
    for version in ["gtk-3.0", "gtk-4.0"] {
        let dir = config.join(version);
        if dir.is_symlink() {
            continue;
        }
        ours |= fs::remove_file(dir.join(CSS)).is_ok();
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
    ours |= fs::remove_file(config.join("qt6ct/colors/apolline.conf")).is_ok();
    ours |= fs::remove_file(data.join("color-schemes").join(KDE)).is_ok();
    ours |= crate::flatpak::revoke(data);
    ours
}

pub fn files() -> Vec<String> {
    vec![
        "~/.config/gtk-3.0/apolline.css".into(),
        "~/.config/gtk-4.0/apolline.css".into(),
        format!("{IMPORT} at the top of gtk.css"),
        "~/.config/qt6ct/colors/apolline.conf, if you use qt6ct".into(),
        "~/.local/share/color-schemes/Apolline.colors, if you use KDE apps or qt6ct".into(),
        "gsettings color-scheme and accent-color".into(),
        "Flatpak apps may read ~/.config/gtk-3.0 and gtk-4.0, in ~/.local/share/flatpak/overrides/global"
            .into(),
    ]
}

pub fn gtk_themes() -> Vec<String> {
    let mut dirs = vec![PathBuf::from("/usr/share/themes")];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".themes"));
        dirs.push(home.join(".local/share/themes"));
    }
    let mut found: Vec<String> = dirs
        .iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.flatten())
        .filter(|entry| {
            let path = entry.path();
            path.join("gtk-3.0").is_dir() || path.join("gtk-4.0").is_dir()
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found.dedup();
    found
}

pub fn font_families() -> Vec<String> {
    let Ok(out) = std::process::Command::new("fc-list")
        .args([":", "family"])
        .output()
    else {
        return Vec::new();
    };
    let mut families: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|name| name.trim().replace('\\', ""))
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .collect();
    families.sort();
    families.dedup();
    families
}

pub fn cursors() -> Vec<String> {
    let mut dirs = vec![PathBuf::from("/usr/share/icons")];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/icons"));
        dirs.push(home.join(".icons"));
    }
    let mut found: Vec<String> = dirs
        .iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.flatten())
        .filter(|entry| entry.path().join("cursors").is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "default")
        .collect();
    found.sort();
    found.dedup();
    found
}

fn data_home() -> Option<PathBuf> {
    apolline_config::xdg_dir("XDG_DATA_HOME", ".local/share")
}

fn config_home() -> Option<PathBuf> {
    apolline_config::xdg_dir("XDG_CONFIG_HOME", ".config")
}

fn import(gtk: &Path) -> Option<String> {
    if gtk.is_symlink() {
        return Some(format!(
            "{} is a link, so it was left alone; add {IMPORT} to it yourself",
            gtk.display()
        ));
    }
    let text = match fs::read_to_string(gtk) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Some(format!(
                "{} was left alone ({err}); add {IMPORT} to it yourself",
                gtk.display()
            ));
        }
    };
    if text.lines().any(|line| line.trim() == IMPORT) {
        return None;
    }
    fs::write(gtk, format!("{IMPORT}\n{text}"))
        .err()
        .map(|err| format!("{}: {err}", gtk.display()))
}

// Solid steps of the scheme for apps that can't take see-through colors: the base, raised a bit,
// raised more, and the text dimmed.
struct Tones {
    window: Rgba,
    view: Rgba,
    raised: Rgba,
    higher: Rgba,
    header: Rgba,
    text: Rgba,
    faint: Rgba,
}

fn tones(scheme: &Scheme) -> Tones {
    let over = |a: f32, below: Rgba| scheme.overlay.alpha(a).over(below);
    let window = scheme.window;
    Tones {
        window,
        view: if scheme.light {
            over(0.0, window)
        } else {
            scheme.ink.alpha(0.5).over(window)
        },
        raised: over(0.06, window),
        higher: over(0.11, window),
        header: over(0.03, window),
        text: scheme.text,
        faint: scheme.text.alpha(0.55).over(window),
    }
}

fn gtk_css(palette: Palette, scheme: &Scheme) -> String {
    let accent = palette.accent.to_hex();
    let on = palette.on_accent.to_hex();
    let t = tones(scheme);
    let [window, view, raised, header, text] =
        [t.window, t.view, t.raised, t.header, t.text].map(Rgba::to_hex);
    format!(
        "/* Written by Apolline from your palette. Turn off \"Color apps\" in its settings to remove it. */
@define-color accent_color {accent};
@define-color accent_bg_color {accent};
@define-color accent_fg_color {on};
@define-color theme_selected_bg_color {accent};
@define-color theme_selected_fg_color {on};
@define-color window_bg_color {window};
@define-color window_fg_color {text};
@define-color view_bg_color {view};
@define-color view_fg_color {text};
@define-color headerbar_bg_color {header};
@define-color headerbar_fg_color {text};
@define-color sidebar_bg_color {header};
@define-color sidebar_fg_color {text};
@define-color card_bg_color {raised};
@define-color card_fg_color {text};
@define-color dialog_bg_color {raised};
@define-color dialog_fg_color {text};
@define-color popover_bg_color {raised};
@define-color popover_fg_color {text};
@define-color theme_bg_color {window};
@define-color theme_fg_color {text};
@define-color theme_base_color {view};
@define-color theme_text_color {text};
"
    )
}

fn kde_colors(palette: Palette, scheme: &Scheme) -> String {
    let rgb = |c: Rgba| format!("{},{},{}", c.r, c.g, c.b);
    let t = tones(scheme);
    let accent = rgb(palette.accent);
    let set = |name: &str, back: Rgba, alternate: Rgba, fore: Rgba| {
        format!(
            "[Colors:{name}]\nBackgroundAlternate={}\nBackgroundNormal={}\nDecorationFocus={accent}\nDecorationHover={accent}\nForegroundActive={accent}\nForegroundInactive={}\nForegroundLink={accent}\nForegroundNegative=255,122,107\nForegroundNeutral=242,179,92\nForegroundNormal={}\nForegroundPositive=125,207,145\nForegroundVisited={}\n\n",
            rgb(alternate),
            rgb(back),
            rgb(t.faint),
            rgb(fore),
            rgb(palette.accent.shift_hue(40.0)),
        )
    };
    let mut text_sets: Vec<String> = [
        ("Button", t.raised, t.higher),
        ("Complementary", t.view, t.window),
        ("Header", t.header, t.raised),
        ("Tooltip", t.raised, t.higher),
        ("View", t.view, t.window),
        ("Window", t.window, t.header),
    ]
    .into_iter()
    .map(|(name, back, alternate)| set(name, back, alternate, t.text))
    .collect();
    text_sets.push(set(
        "Selection",
        palette.accent,
        palette.accent_soft.over(t.window),
        palette.on_accent,
    ));
    format!(
        "[General]\nColorScheme=Apolline\nName=Apolline\n\n{}[WM]\nactiveBackground={}\nactiveBlend={accent}\nactiveForeground={}\ninactiveBackground={}\ninactiveBlend={}\ninactiveForeground={}\n",
        text_sets.concat(),
        rgb(t.header),
        rgb(t.text),
        rgb(t.window),
        rgb(t.faint),
        rgb(t.faint),
    )
}

fn qt_colors(palette: Palette, scheme: &Scheme) -> String {
    let argb = |c: Rgba| format!("#ff{}", c.to_hex().trim_start_matches('#'));
    let t = tones(scheme);
    let shadow = if scheme.light {
        Rgba::hex(0x9a9a9a)
    } else {
        Rgba::hex(0x000000)
    };
    // In qt6ct's order: text, button, light, midlight, dark, mid, button text, bright text,
    // button text again, base, window, shadow, highlight, highlighted text, link, visited link,
    // alternate base, default, tooltip base, tooltip text, placeholder.
    let roles = [
        t.text,
        t.raised,
        t.higher,
        t.raised,
        t.view,
        t.header,
        t.text,
        scheme.overlay,
        t.text,
        t.view,
        t.window,
        shadow,
        palette.accent,
        palette.on_accent,
        palette.accent,
        palette.accent.shift_hue(40.0),
        t.header,
        shadow,
        t.raised,
        t.text,
        t.faint,
    ];
    let active: Vec<String> = roles.iter().map(|c| argb(*c)).collect();
    let faded: Vec<String> = roles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if matches!(i, 0 | 6 | 8) {
                argb(t.faint)
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
        fs::write(dir.join("kdeglobals"), "[General]\n").unwrap();
        let share = dir.join("share");
        write(&dir, &share, Palette::new(ACCENTS[3]), &Scheme::DARK);
        write(&dir, &share, Palette::new(ACCENTS[3]), &Scheme::DARK);
        let kde = fs::read_to_string(share.join("color-schemes").join(KDE)).unwrap();
        assert!(kde.contains("[Colors:Selection]\n"));
        assert!(kde.contains("BackgroundNormal=192,132,252\n"));
        assert_eq!(kde.matches("[Colors:").count(), 7);
        let text = fs::read_to_string(&gtk).unwrap();
        assert_eq!(text, format!("{IMPORT}\nwindow {{ border: none; }}\n"));
        assert!(
            fs::read_to_string(dir.join("gtk-4.0/apolline.css"))
                .unwrap()
                .contains("#c084fc")
        );
        erase(&dir, &share);
        assert!(!share.join("color-schemes").join(KDE).exists());
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
    fn a_gtk_css_that_isnt_text_or_lives_in_a_linked_folder_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("apolline-paint-odd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("gtk-3.0")).unwrap();
        let latin = b"/* caf\xe9 */\nwindow { }\n".to_vec();
        fs::write(dir.join("gtk-3.0/gtk.css"), &latin).unwrap();
        fs::create_dir_all(dir.join("dotfiles/gtk-4.0")).unwrap();
        std::os::unix::fs::symlink(dir.join("dotfiles/gtk-4.0"), dir.join("gtk-4.0")).unwrap();

        let notes = write(
            &dir,
            &dir.join("share"),
            Palette::new(ACCENTS[0]),
            &Scheme::DARK,
        );
        assert_eq!(fs::read(dir.join("gtk-3.0/gtk.css")).unwrap(), latin);
        assert!(!dir.join("dotfiles/gtk-4.0/apolline.css").exists());
        assert_eq!(notes.len(), 2, "{notes:?}");
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

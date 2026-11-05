use std::path::Path;

use apolline_config::Template;
use apolline_theme::{Palette, Rgba, Scheme};

// The colors a template can ask for, by name.
pub fn colors(scheme: &Scheme, palette: Palette) -> Vec<(&'static str, Rgba)> {
    let solid = |c: Rgba| c.over(scheme.ink).alpha(1.0);
    vec![
        ("accent", palette.accent),
        ("on_accent", palette.on_accent),
        ("background", scheme.ink),
        ("surface", solid(scheme.surface)),
        ("panel", solid(scheme.panel)),
        ("window", scheme.window),
        ("text", scheme.text),
        ("muted", scheme.text.alpha(0.72).over(scheme.window)),
        ("faint", scheme.text.alpha(0.5).over(scheme.window)),
        ("border", scheme.overlay.alpha(0.12).over(scheme.window)),
        ("good", Rgba::hex(0x5dd39e)),
        ("warn", Rgba::hex(0xf2b35c)),
        ("danger", Rgba::hex(0xff7a6b)),
    ]
}

// {{name}} is #rrggbb, {{name.hex}} rrggbb, {{name.rgb}} r,g b as "r, g, b", and {{mode}} dark or
// light. Anything else in braces is left as it is.
pub fn fill(text: &str, scheme: &Scheme, palette: Palette) -> String {
    let colors = colors(scheme, palette);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let wanted = after[..end].trim();
        match value(wanted, &colors, scheme.light) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn value(wanted: &str, colors: &[(&str, Rgba)], light: bool) -> Option<String> {
    if wanted == "mode" {
        return Some(if light { "light" } else { "dark" }.to_owned());
    }
    let (name, form) = wanted.split_once('.').unwrap_or((wanted, ""));
    let color = colors.iter().find(|(known, _)| *known == name)?.1;
    match form {
        "" => Some(color.to_hex()),
        "hex" => Some(color.to_hex().trim_start_matches('#').to_owned()),
        "rgb" => Some(format!("{}, {}, {}", color.r, color.g, color.b)),
        _ => None,
    }
}

// Writes each template whose result differs from what's there; the ones written get their
// command run. Problems come back as lines to tell.
pub fn write(
    templates: &[Template],
    scheme: &Scheme,
    palette: Palette,
) -> (Vec<String>, Vec<String>) {
    let mut ran = Vec::new();
    let mut problems = Vec::new();
    for template in templates {
        let (Some(from), Some(to)) = (
            crate::wallpaper::expand(&template.from),
            crate::wallpaper::expand(&template.to),
        ) else {
            problems.push(format!(
                "{}: use a full path or one starting with ~/",
                template.from
            ));
            continue;
        };
        let text = match std::fs::read_to_string(&from) {
            Ok(text) => text,
            Err(err) => {
                problems.push(format!("{}: {err}", from.display()));
                continue;
            }
        };
        let filled = fill(&text, scheme, palette);
        if std::fs::read_to_string(&to).is_ok_and(|was| was == filled) {
            continue;
        }
        match put(&to, &filled) {
            Ok(()) => ran.extend(template.after.clone()),
            Err(err) => problems.push(format!("{}: {err}", to.display())),
        }
    }
    (ran, problems)
}

// Through a file next to it, so an app reading it never sees half. A dotfiles link is written
// through to what it points at, and the file keeps its permissions.
pub fn put(to: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let to = std::fs::canonicalize(to).unwrap_or_else(|_| to.to_path_buf());
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let next = to.with_file_name(format!(".{name}.apolline"));
    std::fs::write(&next, text)?;
    if let Ok(was) = std::fs::metadata(&to) {
        std::fs::set_permissions(&next, was.permissions())?;
    }
    std::fs::rename(&next, &to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_colors_in_the_form_asked() {
        let scheme = Scheme::DARK;
        let palette = Palette::new(Rgba::hex(0x86bdf0));
        let out = fill(
            "bg={{background}} fg={{ text.hex }} a={{accent.rgb}} m={{mode}} keep={{nope}} {{",
            &scheme,
            palette,
        );
        assert_eq!(
            out,
            format!(
                "bg=#0c0d10 fg=f4f3f0 a={}, {}, {} m=dark keep={{{{nope}}}} {{{{",
                palette.accent.r, palette.accent.g, palette.accent.b
            )
        );
    }

    #[test]
    fn a_link_is_written_through_and_the_file_keeps_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("apolline-put-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("dotfiles.conf");
        let link = dir.join("colors.conf");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        put(&link, "new").unwrap();
        assert!(link.is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        let mode = std::fs::metadata(&real).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_template_is_written_once_and_rewritten_only_when_it_changes() {
        let dir = std::env::temp_dir().join(format!("apolline-template-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("in.conf");
        let to = dir.join("out/colors.conf");
        std::fs::write(&from, "accent {{accent}}\n").unwrap();
        let template = Template {
            from: from.display().to_string(),
            to: to.display().to_string(),
            after: Some("true".into()),
        };
        let blue = Palette::new(Rgba::hex(0x86bdf0));
        let (ran, problems) = write(std::slice::from_ref(&template), &Scheme::DARK, blue);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(ran, ["true"]);
        let (ran, _) = write(std::slice::from_ref(&template), &Scheme::DARK, blue);
        assert!(ran.is_empty(), "nothing changed");
        let (ran, _) = write(
            &[template],
            &Scheme::DARK,
            Palette::new(Rgba::hex(0xff6a3d)),
        );
        assert_eq!(ran.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

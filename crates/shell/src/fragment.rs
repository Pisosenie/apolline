use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use apolline_theme::Rgba;

const TEMPLATE: &str = include_str!("../../../assets/niri.kdl");
const ACCENT: &str = "active-color \"#86bdf0\"";

pub fn path() -> PathBuf {
    apolline_config::config_home()
        .join("apolline")
        .join("niri.kdl")
}

pub fn niri_config() -> PathBuf {
    std::env::var_os("NIRI_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            apolline_config::config_home()
                .join("niri")
                .join("config.kdl")
        })
}

pub fn render(accent: Rgba) -> String {
    TEMPLATE.replace(ACCENT, &format!("active-color \"{}\"", accent.to_hex()))
}

pub fn sync(accent: Rgba) {
    let path = path();
    if path.exists()
        && let Err(err) = replace(&path, &render(accent))
    {
        eprintln!("apolline: cannot update {}: {err}", path.display());
    }
}

pub fn setup(fragment: &Path, niri: &Path, accent: Rgba) -> io::Result<Vec<String>> {
    let current = fs::read_to_string(niri).map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => io::Error::new(
            err.kind(),
            format!(
                "{} is missing; start niri once so it writes its config, then run this again",
                niri.display()
            ),
        ),
        _ => err,
    })?;

    let mut done = Vec::new();
    if let Some(dir) = fragment.parent() {
        fs::create_dir_all(dir)?;
    }
    replace(fragment, &render(accent))?;
    done.push(format!("wrote {}", fragment.display()));

    let include = format!("include \"{}\"", fragment.display());
    if current.lines().any(|line| line.trim() == include) {
        done.push(format!("{} already includes it", niri.display()));
        return Ok(done);
    }

    let backup = niri.with_extension("kdl.before-apolline");
    fs::copy(niri, &backup)?;
    done.push(format!("saved your niri config as {}", backup.display()));

    let gap = if current.is_empty() || current.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let mut file = OpenOptions::new().append(true).open(niri)?;
    write!(file, "{gap}\n{include}\n")?;
    done.push(format!(
        "added the include at the end of {}",
        niri.display()
    ));
    Ok(done)
}

fn replace(path: &Path, text: &str) -> io::Result<()> {
    let staged = path.with_extension("kdl.new");
    fs::write(&staged, text)?;
    fs::rename(&staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apolline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn borders_take_the_mode_accent() {
        assert!(
            TEMPLATE.contains(ACCENT),
            "the template lost the accent line"
        );
        let text = render(Rgba::hex(0xff6a3d));
        assert!(text.contains("active-color \"#ff6a3d\""));
        assert!(!text.contains(ACCENT));
    }

    #[test]
    fn setup_appends_the_include_once_and_keeps_a_copy() {
        let dir = scratch("setup");
        let niri = dir.join("config.kdl");
        let fragment = dir.join("apolline").join("niri.kdl");
        fs::write(&niri, "binds {\n    Mod+T { spawn \"foot\"; }\n}").unwrap();

        setup(&fragment, &niri, Rgba::hex(0x86bdf0)).unwrap();
        let again = setup(&fragment, &niri, Rgba::hex(0xf2b35c)).unwrap();

        let text = fs::read_to_string(&niri).unwrap();
        assert_eq!(text.matches("include").count(), 1);
        assert!(text.starts_with("binds {"));
        assert!(
            text.trim_end()
                .ends_with(&format!("include \"{}\"", fragment.display()))
        );
        assert!(again.last().unwrap().contains("already includes it"));
        assert!(fs::read_to_string(&fragment).unwrap().contains("#f2b35c"));
        assert!(dir.join("config.kdl.before-apolline").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn setup_refuses_to_invent_a_niri_config() {
        let dir = scratch("missing");
        let err = setup(
            &dir.join("apolline/niri.kdl"),
            &dir.join("config.kdl"),
            Rgba::hex(0x86bdf0),
        )
        .unwrap_err();
        assert!(err.to_string().contains("start niri once"));
        assert!(!dir.join("config.kdl").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

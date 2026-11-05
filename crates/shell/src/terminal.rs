// The terminal Apolline opens, and how each one is told to run something in it.

// What goes between the terminal and the command it should run.
fn run_flag(program: &str) -> &'static str {
    match program {
        "foot" | "kitty" => "",
        "wezterm" => "start --",
        "gnome-terminal" | "kgx" | "ptyxis" => "--",
        _ => "-e",
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

// A shell line that opens `terminal` running `command` through sh, so pipes and quotes work.
pub fn running(terminal: &str, command: &str) -> String {
    let program = terminal.split_whitespace().next().unwrap_or(terminal);
    let program = program.rsplit('/').next().unwrap_or(program);
    let flag = run_flag(program);
    let gap = if flag.is_empty() { "" } else { " " };
    format!("{terminal}{gap}{flag} sh -c {}", quote(command))
}

// Terminals worth offering in Settings, ours first.
pub const KNOWN: [&str; 9] = [
    "vif",
    "foot",
    "alacritty",
    "kitty",
    "ghostty",
    "wezterm",
    "konsole",
    "gnome-terminal",
    "xterm",
];

pub fn installed() -> Vec<&'static str> {
    KNOWN
        .into_iter()
        .filter(|program| crate::launcher::on_path(program))
        .collect()
}

// Where the terminal's colors are kept for vif (`[colors] from`) and anything else that reads
// pywal's colors.json.
pub fn colors_file() -> Option<std::path::PathBuf> {
    apolline_config::xdg_dir("XDG_CACHE_HOME", ".cache").map(|c| c.join("apolline/terminal.json"))
}

pub fn colors_json(scheme: &apolline_theme::Scheme, palette: apolline_theme::Palette) -> String {
    let colors = apolline_theme::terminal::colors(scheme, palette.accent);
    let numbered: Vec<String> = colors
        .iter()
        .enumerate()
        .map(|(i, c)| format!("    \"color{i}\": \"{}\"", c.to_hex()))
        .collect();
    format!(
        "{{\n  \"special\": {{\n    \"background\": \"{}\",\n    \"foreground\": \"{}\",\n    \"cursor\": \"{}\"\n  }},\n  \"colors\": {{\n{}\n  }}\n}}\n",
        scheme.window.alpha(1.0).to_hex(),
        scheme.text.alpha(1.0).to_hex(),
        palette.accent.to_hex(),
        numbered.join(",\n")
    )
}

// Written when it changes, through a file beside it so a terminal never reads half.
pub fn write_colors(
    scheme: &apolline_theme::Scheme,
    palette: apolline_theme::Palette,
) -> std::io::Result<()> {
    let Some(file) = colors_file() else {
        return Ok(());
    };
    let text = colors_json(scheme, palette);
    if std::fs::read_to_string(&file).is_ok_and(|was| was == text) {
        return Ok(());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let next = file.with_extension("json.new");
    std::fs::write(&next, text)?;
    std::fs::rename(next, file)
}

pub const FROM: &str = "~/.cache/apolline/terminal.json";

fn vif_config(config: &std::path::Path) -> std::path::PathBuf {
    std::env::var_os("VIF_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| config.join("vif/vif.toml"))
}

// Whether vif takes its colors from Apolline: `[colors] from` naming the file above.
pub fn vif_follows(config: &std::path::Path) -> bool {
    std::fs::read_to_string(vif_config(config)).is_ok_and(|text| text.contains(FROM))
}

// Once, at setup: vif in Apolline's colors where vif has no config of its own yet, and vif as the
// terminal for apps that ask for one where nothing was chosen. What's there already stays.
pub fn set_up(config: &std::path::Path) -> Vec<String> {
    let mut done = Vec::new();
    let vif = vif_config(config);
    if !vif.exists() {
        let text = format!(
            "# Written by apolline setup: vif in Apolline's colors, following them as they change.\n\n[colors]\nfrom = \"{FROM}\"\n"
        );
        let wrote = vif
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&vif, text));
        done.push(match wrote {
            Ok(()) => format!("wrote {}, so vif wears Apolline's colors", vif.display()),
            Err(err) => format!("{}: {err}", vif.display()),
        });
    } else if !vif_follows(config) {
        done.push(format!(
            "{} is yours, so it's left alone; [colors] from = \"{FROM}\" puts vif in Apolline's colors",
            vif.display()
        ));
    }
    let list = config.join("xdg-terminals.list");
    if !list.exists() && std::fs::write(&list, "vif.desktop\n").is_ok() {
        done.push(format!(
            "wrote {}, so apps that open a terminal open vif",
            list.display()
        ));
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_makes_vif_follow_only_where_nothing_was_set() {
        let dir = std::env::temp_dir().join(format!("apolline-vif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let done = set_up(&dir);
        assert_eq!(done.len(), 2, "{done:?}");
        assert!(vif_follows(&dir));
        assert_eq!(
            std::fs::read_to_string(dir.join("xdg-terminals.list")).unwrap(),
            "vif.desktop\n"
        );
        std::fs::write(dir.join("vif/vif.toml"), "[font]\nsize = 13\n").unwrap();
        let again = set_up(&dir);
        assert!(again[0].contains("left alone"), "{again:?}");
        assert_eq!(
            std::fs::read_to_string(dir.join("vif/vif.toml")).unwrap(),
            "[font]\nsize = 13\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_colors_read_as_pywal_does() {
        let json = colors_json(
            &apolline_theme::Scheme::DARK,
            apolline_theme::Palette::new(apolline_theme::Rgba::hex(0x86bdf0)),
        );
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["special"]["background"], "#141619");
        assert_eq!(value["colors"].as_object().unwrap().len(), 16);
        assert!(
            value["colors"]["color15"]
                .as_str()
                .unwrap()
                .starts_with('#')
        );
    }

    #[test]
    fn each_terminal_is_told_to_run_things_its_own_way() {
        assert_eq!(running("vif", "htop"), "vif -e sh -c 'htop'");
        assert_eq!(running("foot", "htop"), "foot sh -c 'htop'");
        assert_eq!(
            running("/usr/bin/wezterm", "nvim"),
            "/usr/bin/wezterm start -- sh -c 'nvim'"
        );
        assert_eq!(
            running("vif", "echo 'hi' | less"),
            r"vif -e sh -c 'echo '\''hi'\'' | less'"
        );
    }
}

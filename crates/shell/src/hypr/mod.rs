pub mod apply;
pub mod binds;
pub mod conf;
pub mod plan;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use apolline_config::{Config, Editor};

const USAGE: &str = "usage: apolline import hyprland [--dry-run | --undo]";

// `apolline import hyprland`: what Hyprland's config says that has a place here, written into
// Apolline's config with a note for every line looked at.
pub fn main(args: &[String]) -> ExitCode {
    let flag = match args {
        [what] if what == "hyprland" => None,
        [what, flag] if what == "hyprland" && (flag == "--dry-run" || flag == "--undo") => {
            Some(flag.as_str())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let paths = apply::Paths::real();
    if flag == Some("--undo") {
        return finish(apply::undo(&paths));
    }
    let home = apolline_config::config_home();
    let file = conf::file(&home);
    let lines = conf::read(&file);
    if lines.is_empty() {
        eprintln!("apolline: nothing to read at {}", file.display());
        return ExitCode::FAILURE;
    }
    let (config, _) = apolline_config::Config::load(&paths.config);
    let niri = std::fs::read_to_string(crate::fragment::niri_config()).unwrap_or_default();
    let outputs = crate::niri::outputs();
    let plan = plan::plan(&lines, &crate::keys::binds(&niri), &config, &outputs);
    let wallpaper = wallpaper(&lines, &home);
    // What the keyboard and the wallpaper would do, said ahead like the rest.
    let mut also = Editor::load(&paths.config)
        .map(|mut scratch| take_keyboard(&config, &keyboard_of(&lines), &mut scratch))
        .unwrap_or_default();
    also.extend(wallpaper.iter().map(|path| format!("the wallpaper {path}")));
    print!("{}", report(&plan, &also));
    if flag == Some("--dry-run") {
        println!("\nNothing written: run it without --dry-run to bring this over.");
        return ExitCode::SUCCESS;
    }
    let done = apply::apply(&plan, &keyboard_of(&lines), wallpaper.as_deref(), &paths);
    let code = finish(done);
    println!("apolline import hyprland --undo puts everything back as it was.");
    code
}

fn finish(done: Result<Vec<String>, String>) -> ExitCode {
    match done {
        Ok(done) => {
            println!();
            done.iter().for_each(|line| println!("{line}"));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

pub fn report(plan: &plan::Plan, also: &[String]) -> String {
    let mut out = String::new();
    for (group, title) in ["Comes over", "Already here", "Stays behind"]
        .into_iter()
        .enumerate()
    {
        let notes: Vec<(&plan::Note, &str)> = plan
            .notes
            .iter()
            .filter_map(|note| {
                let (at, said) = note.outcome.group();
                (at == group).then_some((note, said))
            })
            .collect();
        let also: &[String] = if group == 0 { also } else { &[] };
        let count = notes.len() + also.len();
        if count == 0 {
            continue;
        }
        out.push_str(&format!("{title} ({count})\n"));
        for (note, said) in notes {
            if group == 0 {
                out.push_str(&format!("  {:<22} {said}\n", note.at));
            } else {
                out.push_str(&format!(
                    "  {:<22} {}\n  {:<22} {said}\n",
                    note.at,
                    short(&note.line),
                    ""
                ));
            }
        }
        for said in also {
            out.push_str(&format!("  {:<22} {said}\n", ""));
        }
    }
    out
}

fn short(line: &str) -> String {
    match line.char_indices().nth(70) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

// The picture hyprpaper, swww or HyDE showed last, if it's still there.
pub fn wallpaper(lines: &[conf::Line], config_home: &Path) -> Option<String> {
    let paper = conf::read(&config_home.join("hypr/hyprpaper.conf"));
    let from_paper = paper
        .iter()
        .filter(|line| line.key == "wallpaper")
        .filter_map(|line| {
            line.value
                .split_once(',')
                .map(|(_, path)| path.trim().to_owned())
        });
    let from_swww = lines
        .iter()
        .filter(|line| line.key.starts_with("exec"))
        .filter_map(|line| {
            let mut words = line.value.split_whitespace().skip_while(|w| *w != "img");
            words.next()?;
            words.next().map(str::to_owned)
        });
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let from_hyde = home
        .as_ref()
        .and_then(|home| std::fs::canonicalize(home.join(".cache/hyde/wall.set")).ok())
        .map(|path| path.to_string_lossy().into_owned());
    from_paper
        .chain(from_swww)
        .chain(from_hyde)
        .map(|path| match (path.strip_prefix("~/"), &home) {
            (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
            _ => path,
        })
        .find(|path| Path::new(path).is_file())
}

// What a Hyprland config says about the keyboard, so coming over keeps the layouts and the key
// that switches them.
#[derive(Debug, Default, PartialEq)]
pub struct Keyboard {
    pub layouts: Vec<String>,
    pub toggle: Option<String>,
}

pub fn keyboard(home: &Path) -> Keyboard {
    keyboard_of(&conf::read(&conf::file(home)))
}

pub fn keyboard_of(lines: &[conf::Line]) -> Keyboard {
    let layouts = conf::last(lines, "input:kb_layout").map_or_else(Vec::new, |value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    });
    let toggle = conf::last(lines, "input:kb_options").and_then(|value| {
        value
            .split(',')
            .map(str::trim)
            .find_map(|o| o.strip_prefix("grp:"))
            .map(str::to_owned)
    });
    Keyboard { layouts, toggle }
}

// Writes Hyprland's keyboard into Apolline's config when Apolline has none of its own yet.
pub fn take_keyboard(config: &Config, found: &Keyboard, editor: &mut Editor) -> Vec<String> {
    let keyboard = &config.keyboard;
    if !keyboard.layouts.is_empty()
        || keyboard.switch.is_some()
        || !config.shortcuts.layout.is_empty()
        || found.layouts.len() < 2
    {
        return Vec::new();
    }
    let mut done = vec![format!(
        "took the layouts {} from Hyprland",
        found.layouts.join(", ")
    )];
    editor.set_list("keyboard", "layouts", &found.layouts);
    match found.toggle.as_deref() {
        Some("alt_shift_toggle") => editor.set("keyboard", "switch", "alt-shift"),
        Some("ctrl_shift_toggle") => editor.set("keyboard", "switch", "ctrl-shift"),
        Some("caps_toggle") => editor.set("keyboard", "switch", "caps"),
        Some("win_space_toggle") => {
            editor.set("shortcuts", "layout", "Mod+Space");
            done.push("Super+Space switches them, as it did in Hyprland".to_owned());
            if crate::keys::same(&config.shortcuts.launcher, "Mod+Space") {
                editor.set("shortcuts", "launcher", "Mod+D");
                done.push(
                    "so the launcher moved to Super+D; Settings → Shortcuts changes it".to_owned(),
                );
            }
        }
        _ => {}
    }
    done
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apolline-hypr-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("hypr/conf.d")).unwrap();
        dir
    }

    #[test]
    fn the_keyboard_is_read_through_sourced_files() {
        let home = scratch("read");
        fs::write(
            home.join("hypr/hyprland.conf"),
            "input {\n    kb_layout = de\n}\nsource = ./conf.d/*.conf\n",
        )
        .unwrap();
        fs::write(
            home.join("hypr/conf.d/input.conf"),
            "input {\n    kb_layout = us, ru # mine\n    kb_options = caps:escape,grp:win_space_toggle\n}\n",
        )
        .unwrap();
        assert_eq!(
            keyboard(&home),
            Keyboard {
                layouts: vec!["us".into(), "ru".into()],
                toggle: Some("win_space_toggle".into()),
            }
        );
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn super_space_moves_to_layouts_and_the_launcher_steps_aside() {
        let found = Keyboard {
            layouts: vec!["us".into(), "ru".into()],
            toggle: Some("win_space_toggle".into()),
        };
        let mut editor = Editor::parse("").unwrap();
        let done = take_keyboard(&Config::default(), &found, &mut editor);
        assert_eq!(done.len(), 3);
        let (config, problems) = Config::parse(&editor.text());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert_eq!(config.shortcuts.layout, "Mod+Space");
        assert_eq!(config.shortcuts.launcher, "Mod+D");

        let mut again = Editor::parse(&editor.text()).unwrap();
        assert!(
            take_keyboard(&config, &found, &mut again).is_empty(),
            "only once"
        );
    }

    #[test]
    fn a_single_layout_is_left_alone() {
        let found = Keyboard {
            layouts: vec!["us".into()],
            toggle: None,
        };
        let mut editor = Editor::parse("").unwrap();
        assert!(take_keyboard(&Config::default(), &found, &mut editor).is_empty());
        assert_eq!(editor.text(), "");
    }
}

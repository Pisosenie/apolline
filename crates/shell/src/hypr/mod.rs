pub mod apply;
pub mod binds;
pub mod conf;
pub mod lua;
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
    let (file, lines, stopped) = read(&home);
    if lines.is_empty() {
        eprintln!(
            "apolline: nothing to read at {}{}",
            file.display(),
            stopped.map_or(String::new(), |why| format!(": {why}"))
        );
        return ExitCode::FAILURE;
    }
    if let Some(why) = stopped {
        println!(
            "{} stopped partway ({why}); this is what it set before that.\n",
            file.display()
        );
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

// Hyprland reads hyprland.lua when there is one, else hyprland.conf. The Lua one is run in a
// sandbox, which says why if it stopped partway.
pub fn read(config_home: &Path) -> (PathBuf, Vec<conf::Line>, Option<String>) {
    let lua = lua::file(config_home);
    if lua.is_file() {
        let (lines, stopped) = lua::read(&lua);
        return (lua, lines, stopped);
    }
    let file = conf::file(config_home);
    let lines = conf::read(&file);
    (file, lines, None)
}

pub fn keyboard(home: &Path) -> Keyboard {
    keyboard_of(&read(home).1)
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

    // The calls Hyprland's own example hyprland.lua makes, as a whole import plan.
    #[test]
    fn a_lua_config_like_hyprlands_example_comes_over() {
        let home = scratch("lua");
        fs::write(
            home.join("hypr/hyprland.lua"),
            r#"
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
local terminal, fileManager, menu = "kitty", "dolphin", "hyprlauncher"
hl.on("hyprland.start", function () hl.exec_cmd("waybar & hyprpaper & firefox") end)
hl.env("XCURSOR_SIZE", "24")
hl.config({ general = { gaps_in = 5, border_size = 2, layout = "dwindle" },
            decoration = { rounding = 10, shadow = { enabled = true, color = 0xee1a1a1a } },
            input = { kb_layout = "us", follow_mouse = 1, touchpad = { natural_scroll = false } } })
hl.curve("easy", { type = "spring", mass = 1 })
hl.animation({ leaf = "global", enabled = true, speed = 10, bezier = "default" })
hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
hl.device({ name = "epic-mouse-v1", sensitivity = -0.5 })
local mainMod = "SUPER"
hl.bind(mainMod .. " + Q", hl.dsp.exec_cmd(terminal))
local closeWindowBind = hl.bind(mainMod .. " + C", hl.dsp.window.close())
hl.bind(mainMod .. " + V", hl.dsp.window.float({ action = "toggle" }))
hl.bind(mainMod .. " + R", hl.dsp.exec_cmd(menu))
hl.bind(mainMod .. " + J", hl.dsp.layout("togglesplit"))
hl.bind(mainMod .. " + left", hl.dsp.focus({ direction = "left" }))
for i = 1, 10 do
    local key = i % 10
    hl.bind(mainMod .. " + " .. key, hl.dsp.focus({ workspace = i }))
end
hl.bind(mainMod .. " + S", hl.dsp.workspace.toggle_special("magic"))
hl.bind(mainMod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })
hl.window_rule({ name = "suppress", match = { class = ".*" }, suppress_event = "maximize" })
hl.window_rule({ name = "run", match = { class = "hyprland-run" }, move = "20 monitor_h-120", float = true })
"#,
        )
        .unwrap();
        let (file, lines, stopped) = read(&home);
        assert!(file.ends_with("hypr/hyprland.lua"));
        assert_eq!(stopped, None);
        let plan = plan::plan(&lines, &[], &Config::default(), &[]);
        assert_eq!(
            plan.shortcuts,
            [(apolline_config::Shortcut::Launcher, "Mod+R".to_owned())]
        );
        let combos: Vec<&str> = plan.keys.iter().map(|(combo, _)| combo.as_str()).collect();
        assert!(
            combos.contains(&"Mod+Q") && combos.contains(&"Mod+V") && combos.contains(&"Mod+0")
        );
        assert!(combos.contains(&"Mod+WheelScrollDown"));
        assert!(!combos.contains(&"Mod+J") && !combos.contains(&"Mod+S"));
        assert_eq!(plan.autostart, ["firefox"]);
        assert_eq!(plan.rules.len(), 1);
        assert_eq!(plan.rules[0].app, "hyprland-run");
        assert!(plan.displays.is_empty());
        let report = report(&plan, &[]);
        assert!(report.contains("hyprland.lua:"), "{report}");
        fs::remove_dir_all(home).unwrap();
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

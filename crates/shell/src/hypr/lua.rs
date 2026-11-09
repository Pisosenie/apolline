use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, VmState};

use super::conf::Line;

const SANDBOX: &str = include_str!("sandbox.lua");
// Checks every 100 000 instructions; this many checks is far past any real config.
const CHECKS: u32 = 2_000;
const MEMORY: usize = 256 << 20;

pub fn file(config_home: &Path) -> std::path::PathBuf {
    config_home.join("hypr/hyprland.lua")
}

// A hyprland.lua run with `hl` only taking notes: the lines a hyprland.conf would have had.
// A config that stops partway gives what it noted until then, and why it stopped.
pub fn read(path: &Path) -> (Vec<Line>, Option<String>) {
    match run(path) {
        Ok(read) => read,
        Err(err) => (Vec::new(), Some(err.to_string())),
    }
}

fn run(path: &Path) -> mlua::Result<(Vec<Line>, Option<String>)> {
    // SAFETY: the debug library is what configs use to find their own folder (HyDE does); the
    // sandbox takes away running programs, writing files and loading C modules.
    let lua = unsafe { Lua::unsafe_new_with(StdLib::ALL_SAFE | StdLib::DEBUG, LuaOptions::new()) };
    lua.set_memory_limit(MEMORY)?;
    let checks = AtomicU32::new(0);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(100_000),
        move |_, _| {
            if checks.fetch_add(1, Ordering::Relaxed) > CHECKS {
                return Err(mlua::Error::runtime("the config ran for too long"));
            }
            Ok(VmState::Continue)
        },
    )?;
    let dir = path.parent().unwrap_or(Path::new("."));
    lua.globals()
        .set("__apolline_dir", dir.to_string_lossy().into_owned())?;
    lua.load(SANDBOX).set_name("=apolline-sandbox").exec()?;
    let stopped = lua
        .load(path)
        .exec()
        .err()
        .map(|err| first_line(&err.to_string()));
    let notes: Table = lua.globals().get("__apolline_notes")?;
    let mut lines = Vec::new();
    for note in notes.sequence_values::<Table>() {
        let note = note?;
        if note.get::<Option<bool>>("off")?.unwrap_or(false) {
            continue;
        }
        lines.push(Line {
            key: note.get("key")?,
            value: note.get("value")?,
            at: note.get("at")?,
        });
    }
    Ok((lines, stopped))
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_text(text: &str) -> (Vec<Line>, Option<String>) {
        let dir = std::env::temp_dir().join(format!(
            "apolline-hyprlua-{}-{}",
            std::process::id(),
            text.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("extra.lua"),
            "hl.bind('SUPER + E', hl.dsp.exec_cmd('dolphin'))\nreturn { term = 'foot' }\n",
        )
        .unwrap();
        let path = dir.join("hyprland.lua");
        std::fs::write(&path, text).unwrap();
        let read = read(&path);
        std::fs::remove_dir_all(dir).unwrap();
        read
    }

    #[test]
    fn hl_calls_come_back_as_the_lines_of_a_hyprland_conf() {
        let (lines, stopped) = read_text(
            r#"
local mainMod = "SUPER"
local extra = require("extra")
hl.monitor({ output = "DP-1", mode = "2560x1440@165", position = "0x0", scale = 1.25 })
hl.config({
    general = { gaps_in = 5, col = { active_border = { colors = {"rgba(33ccffee)", "rgba(00ff99ee)"}, angle = 45 } } },
    input = { kb_layout = "us,ru", touchpad = { natural_scroll = true } },
})
hl.on("hyprland.start", function () hl.exec_cmd("fcitx5 -d") end)
hl.bind(mainMod .. " + Return", hl.dsp.exec_cmd(extra.term))
local close = hl.bind(mainMod .. " + C", hl.dsp.window.close())
local gone = hl.bind(mainMod .. " + X", hl.dsp.window.close())
gone:set_enabled(false)
for i = 1, 2 do
    hl.bind(mainMod .. " + SHIFT + " .. i, hl.dsp.window.move({ workspace = i }))
end
hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind("XF86AudioMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
hl.window_rule({ name = "pv", match = { class = "^(pavucontrol)$" }, float = true })
hl.curve("easy", { type = "spring" })
require("missing.module").anything.at.all()
os.execute("touch /tmp/apolline-should-not-exist")
"#,
        );
        assert_eq!(stopped, None);
        let pairs: Vec<(&str, &str)> = lines
            .iter()
            .map(|l| (l.key.as_str(), l.value.as_str()))
            .collect();
        for wanted in [
            ("monitor", "DP-1, 2560x1440@165, 0x0, 1.25"),
            ("general:gaps_in", "5"),
            (
                "general:col.active_border",
                "rgba(33ccffee) rgba(00ff99ee) 45deg",
            ),
            ("input:kb_layout", "us,ru"),
            ("input:touchpad:natural_scroll", "true"),
            ("exec-once", "fcitx5 -d"),
            ("bindm", "SUPER, mouse:272, movewindow, "),
            (
                "bindl",
                ", XF86AudioMute, exec, wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle",
            ),
            ("windowrulev2", "float, class:^(pavucontrol)$"),
        ] {
            assert!(pairs.contains(&wanted), "{wanted:?} missing: {pairs:#?}");
        }
        let binds: Vec<&str> = pairs
            .iter()
            .filter(|(key, _)| *key == "bind")
            .map(|(_, value)| *value)
            .collect();
        assert_eq!(
            binds,
            [
                "SUPER, E, exec, dolphin",
                "SUPER, Return, exec, foot",
                "SUPER, C, killactive, ",
                "SUPER SHIFT, 1, movetoworkspace, 1",
                "SUPER SHIFT, 2, movetoworkspace, 2",
            ]
        );
        let skipped = pairs.iter().find(|(key, _)| *key == "lua:skipped").unwrap();
        assert_eq!(
            skipped.1,
            "require missing.module: module 'missing.module' not found:"
        );
        assert_eq!(lines[1].at, "hyprland.lua:4");
        assert_eq!(lines[0].at, "extra.lua:1");
        assert!(!Path::new("/tmp/apolline-should-not-exist").exists());
    }

    #[test]
    fn a_config_that_never_ends_is_stopped() {
        let (lines, stopped) =
            read_text("hl.bind('SUPER + Q', hl.dsp.window.close())\nwhile true do end\n");
        assert_eq!(lines.len(), 1);
        assert!(stopped.is_some_and(|why| why.contains("too long")));
    }
}

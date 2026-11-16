//! A Hyprland config, `hyprland.conf` or `hyprland.lua`, read the way the Hyprland it was written
//! for reads it, and said again in niri's terms.

pub mod conf;
pub mod era;
pub mod kdl;
pub mod keys;
#[cfg(feature = "lua")]
pub mod lua;
pub mod niri;
pub mod report;
pub mod rules;

pub use conf::Line;

// Hyprland reads hyprland.lua when there is one, else hyprland.conf. The Lua one is run in a
// sandbox, which says why if it stopped partway.
#[cfg(feature = "lua")]
pub fn read(config_home: &std::path::Path) -> (std::path::PathBuf, Vec<Line>, Option<String>) {
    let lua = lua::file(config_home);
    if lua.is_file() {
        let (lines, stopped) = lua::read(&lua);
        return (lua, lines, stopped);
    }
    let file = conf::file(config_home);
    let lines = conf::read(&file);
    (file, lines, None)
}

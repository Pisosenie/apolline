mod diagnostic;
mod edit;
mod read;

use std::path::{Path, PathBuf};

use apolline_theme::{Mode, Palette, Rgba};
pub use diagnostic::Diagnostic;
pub use edit::Editor;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    pub layout: Layout,
    pub bar: Bar,
    pub keyboard: Keyboard,
    pub touchpad: Option<Touchpad>,
    pub mouse: Option<Mouse>,
    pub alerts: Alerts,
    pub shortcuts: Shortcuts,
    modes: [ModeSettings; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub gaps: u16,
    pub radius: u16,
    pub borders: bool,
    pub shadows: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub workspaces: Workspaces,
    pub seconds: bool,
    pub date: bool,
    pub battery_percent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workspaces {
    Dots,
    Numbers,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keyboard {
    pub layouts: Vec<String>,
    pub switch: Option<Switch>,
    pub repeat_delay: Option<u16>,
    pub repeat_rate: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    AltShift,
    CtrlShift,
    SuperSpace,
    Caps,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touchpad {
    pub tap: bool,
    pub natural_scroll: bool,
    pub disable_while_typing: bool,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mouse {
    pub natural_scroll: bool,
    pub acceleration: bool,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    pub launcher: String,
    pub panel: String,
    pub switch: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Launcher,
    Panel,
    Switch,
}

impl Shortcut {
    pub const ALL: [Shortcut; 3] = [Shortcut::Launcher, Shortcut::Panel, Shortcut::Switch];

    pub fn key(self) -> &'static str {
        match self {
            Shortcut::Launcher => "launcher",
            Shortcut::Panel => "panel",
            Shortcut::Switch => "switch",
        }
    }
}

impl Shortcuts {
    pub fn get(&self, shortcut: Shortcut) -> &str {
        match shortcut {
            Shortcut::Launcher => &self.launcher,
            Shortcut::Panel => &self.panel,
            Shortcut::Switch => &self.switch,
        }
    }

    pub fn get_mut(&mut self, shortcut: Shortcut) -> &mut String {
        match shortcut {
            Shortcut::Launcher => &mut self.launcher,
            Shortcut::Panel => &mut self.panel,
            Shortcut::Switch => &mut self.switch,
        }
    }
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            launcher: "Mod+Space".into(),
            panel: "Mod+A".into(),
            switch: "Mod+Shift+M".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alerts {
    pub do_not_disturb: bool,
    pub seconds: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModeSettings {
    pub wallpaper: String,
    pub accent: Accent,
    pub notifications: Notifications,
    pub open: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accent {
    Auto,
    Fixed(Rgba),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notifications {
    All,
    Mentions,
    Hold,
}

impl Config {
    pub fn parse(src: &str) -> (Self, Vec<Diagnostic>) {
        read::document(src)
    }

    pub fn load(path: &Path) -> (Self, Vec<Diagnostic>) {
        match std::fs::read_to_string(path) {
            Ok(src) => Self::parse(&src),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Self::default(), Vec::new()),
            Err(err) => (
                Self::default(),
                vec![Diagnostic::new(
                    0,
                    format!("could not read the file: {err}"),
                )],
            ),
        }
    }

    pub fn mode(&self, mode: Mode) -> &ModeSettings {
        &self.modes[mode as usize]
    }

    pub fn mode_mut(&mut self, mode: Mode) -> &mut ModeSettings {
        &mut self.modes[mode as usize]
    }

    pub fn palette(&self, mode: Mode) -> Palette {
        match self.mode(mode).accent {
            Accent::Auto => Palette::of(mode),
            Accent::Fixed(color) => Palette::of(mode).with_accent(color),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Work,
            layout: Layout {
                gaps: 10,
                radius: 12,
                borders: true,
                shadows: true,
            },
            bar: Bar {
                workspaces: Workspaces::Dots,
                seconds: false,
                date: true,
                battery_percent: true,
            },
            keyboard: Keyboard::default(),
            touchpad: None,
            mouse: None,
            alerts: Alerts {
                do_not_disturb: false,
                seconds: 5,
            },
            shortcuts: Shortcuts::default(),
            modes: Mode::ALL.map(ModeSettings::default_for),
        }
    }
}

impl Switch {
    pub const ALL: [Switch; 4] = [
        Switch::AltShift,
        Switch::CtrlShift,
        Switch::SuperSpace,
        Switch::Caps,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Switch::AltShift => "alt-shift",
            Switch::CtrlShift => "ctrl-shift",
            Switch::SuperSpace => "super-space",
            Switch::Caps => "caps",
        }
    }

    pub fn xkb(self) -> &'static str {
        match self {
            Switch::AltShift => "grp:alt_shift_toggle",
            Switch::CtrlShift => "grp:ctrl_shift_toggle",
            Switch::SuperSpace => "grp:win_space_toggle",
            Switch::Caps => "grp:caps_toggle",
        }
    }
}

impl Default for Touchpad {
    fn default() -> Self {
        Self {
            tap: true,
            natural_scroll: true,
            disable_while_typing: true,
            speed: 0.0,
        }
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            natural_scroll: false,
            acceleration: true,
            speed: 0.0,
        }
    }
}

impl ModeSettings {
    fn default_for(mode: Mode) -> Self {
        Self {
            wallpaper: Palette::of(mode).wallpaper.to_owned(),
            accent: Accent::Auto,
            notifications: match mode {
                Mode::Work => Notifications::Mentions,
                Mode::Play => Notifications::Hold,
                Mode::Evening => Notifications::All,
            },
            open: Vec::new(),
        }
    }
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
}

pub fn default_path() -> PathBuf {
    config_home().join("apolline").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> (Config, Vec<Diagnostic>) {
        Config::parse(src)
    }

    #[test]
    fn empty_file_is_the_default() {
        assert_eq!(parse(""), (Config::default(), vec![]));
    }

    #[test]
    fn shipped_config_is_clean() {
        let (config, diagnostics) = parse(include_str!("../../../assets/config.toml"));
        assert_eq!(diagnostics, vec![]);
        assert_eq!(config.mode(Mode::Play).open, ["steam", "discord"]);
    }

    #[test]
    fn typo_is_reported_with_a_suggestion_and_the_rest_applies() {
        let (config, diagnostics) = parse("[layout]\ngaps_in = 4\nborders = false\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].message, "unknown option “gaps_in”");
        assert_eq!(diagnostics[0].hint.as_deref(), Some("did you mean “gaps”?"));
        assert!(!config.layout.borders);
        assert_eq!(config.layout.gaps, Config::default().layout.gaps);
    }

    #[test]
    fn misspelled_section_and_mode() {
        let (_, diagnostics) = parse("[layut]\ngaps = 4\n\n[mode.party]\nopen = []\n");
        let hints: Vec<_> = diagnostics
            .iter()
            .map(|d| (d.line, d.hint.clone()))
            .collect();
        assert_eq!(hints[0], (1, Some("did you mean “layout”?".into())));
        assert_eq!(hints[1], (4, Some("known: work, play, evening".into())));
    }

    #[test]
    fn wrong_types_keep_defaults() {
        let (config, diagnostics) = parse("[layout]\ngaps = \"ten\"\nradius = 99\n");
        assert_eq!(config.layout, Config::default().layout);
        assert_eq!(
            diagnostics[0].message,
            "“gaps” should be a whole number from 0 to 64"
        );
        assert_eq!(
            diagnostics[1].message,
            "“radius” should be a whole number from 0 to 32"
        );
    }

    #[test]
    fn choices_suggest_the_closest_value() {
        let (config, diagnostics) =
            parse("[general]\nmode = \"evenin\"\n\n[mode.work]\nnotifications = \"mention\"\n");
        assert_eq!(config.mode, Mode::Work);
        assert_eq!(
            diagnostics[0].hint.as_deref(),
            Some("did you mean “evening”?")
        );
        assert_eq!(
            diagnostics[1].hint.as_deref(),
            Some("did you mean “mentions”?")
        );
    }

    #[test]
    fn accent_is_auto_or_a_color() {
        let (config, diagnostics) =
            parse("[mode.play]\naccent = \"#3b2a7a\"\n\n[mode.work]\naccent = \"blue\"\n");
        assert_eq!(
            config.mode(Mode::Play).accent,
            Accent::Fixed(Rgba::hex(0x3b2a7a))
        );
        assert_eq!(config.palette(Mode::Play).accent, Rgba::hex(0x3b2a7a));
        assert_eq!(config.mode(Mode::Work).accent, Accent::Auto);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn input_sections_only_exist_when_written() {
        let (config, diagnostics) = parse(
            "[keyboard]\nlayouts = [\"us\", \"ru\"]\nswitch = \"alt-shif\"\n\n[touchpad]\ntap = false\nspeed = 1.5\n",
        );
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert_eq!(config.keyboard.switch, None);
        assert_eq!(config.keyboard.repeat_delay, None);
        assert_eq!(
            config.touchpad,
            Some(Touchpad {
                tap: false,
                ..Touchpad::default()
            })
        );
        assert_eq!(config.mouse, None);
        assert_eq!(
            diagnostics[0].hint.as_deref(),
            Some("did you mean “alt-shift”?")
        );
        assert_eq!(diagnostics[1].line, 7);

        let (_, diagnostics) = parse("[keyboard]\nlayouts = [\"English\"]\n");
        assert_eq!(
            diagnostics[0].message,
            "“English” is not a keyboard layout code"
        );
    }

    #[test]
    fn broken_syntax_is_reported_and_the_rest_applies() {
        let (config, diagnostics) = parse("[layout]\ngaps = \nborders = false\n");
        assert!(diagnostics.iter().any(|d| d.line == 2), "{diagnostics:?}");
        assert!(!config.layout.borders);
    }
}

mod diagnostic;
mod read;

use std::path::{Path, PathBuf};

use apolline_theme::{Mode, Palette, Rgba};
pub use diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    pub layout: Layout,
    modes: [ModeSettings; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub gaps: u16,
    pub radius: u16,
    pub borders: bool,
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
            },
            modes: Mode::ALL.map(ModeSettings::default_for),
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
    fn broken_syntax_is_reported_and_the_rest_applies() {
        let (config, diagnostics) = parse("[layout]\ngaps = \nborders = false\n");
        assert!(diagnostics.iter().any(|d| d.line == 2), "{diagnostics:?}");
        assert!(!config.layout.borders);
    }
}

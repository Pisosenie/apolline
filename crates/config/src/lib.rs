mod diagnostic;
mod edit;
mod read;
mod theme;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use apolline_theme::{ACCENTS, Palette, Rgba};
pub use diagnostic::Diagnostic;
pub use edit::Editor;
pub use theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Option<String>,
    pub theme: Option<(String, Option<String>)>,
    pub desktop: Desktop,
    pub layout: Layout,
    pub bar: Bar,
    pub keyboard: Keyboard,
    pub touchpad: Option<Touchpad>,
    pub mouse: Option<Mouse>,
    pub alerts: Alerts,
    pub shortcuts: Shortcuts,
    pub lock: Lock,
    pub idle: Idle,
    pub apps: Apps,
    pub windows: Windows,
    pub rules: Vec<Rule>,
    pub pinned: Vec<String>,
    pub modes: Vec<ModeSettings>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Windows {
    pub width: Option<Width>,
    pub presets: Option<Vec<Width>>,
    pub center: Option<Center>,
    pub center_lone: bool,
    pub focus_follows_mouse: bool,
    pub warp_mouse: bool,
    pub opacity: Option<f32>,
    pub inactive_opacity: Option<f32>,
    pub animations: Option<f32>,
    pub motion: Option<Motion>,
    pub appear: Option<Appear>,
    pub blur: Option<Blur>,
    pub switcher: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blur {
    Wallpaper,
    All,
}

impl Blur {
    pub const ALL: [Blur; 2] = [Blur::Wallpaper, Blur::All];

    pub fn key(self) -> &'static str {
        match self {
            Blur::Wallpaper => "wallpaper",
            Blur::All => "all",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Snappy,
    Smooth,
    Bouncy,
}

impl Motion {
    pub const ALL: [Motion; 3] = [Motion::Snappy, Motion::Smooth, Motion::Bouncy];

    pub fn key(self) -> &'static str {
        match self {
            Motion::Snappy => "snappy",
            Motion::Smooth => "smooth",
            Motion::Bouncy => "bouncy",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appear {
    Pop,
    Slide,
    Fade,
}

impl Appear {
    pub const ALL: [Appear; 3] = [Appear::Pop, Appear::Slide, Appear::Fade];

    pub fn key(self) -> &'static str {
        match self {
            Appear::Pop => "pop",
            Appear::Slide => "slide",
            Appear::Fade => "fade",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Third,
    Half,
    TwoThirds,
    ThreeQuarters,
    Full,
}

impl Width {
    pub const ALL: [Width; 5] = [
        Width::Third,
        Width::Half,
        Width::TwoThirds,
        Width::ThreeQuarters,
        Width::Full,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Width::Third => "1/3",
            Width::Half => "1/2",
            Width::TwoThirds => "2/3",
            Width::ThreeQuarters => "3/4",
            Width::Full => "full",
        }
    }

    pub fn proportion(self) -> f32 {
        match self {
            Width::Third => 1.0 / 3.0,
            Width::Half => 0.5,
            Width::TwoThirds => 2.0 / 3.0,
            Width::ThreeQuarters => 0.75,
            Width::Full => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Center {
    Never,
    Overflow,
    Always,
}

impl Center {
    pub const ALL: [Center; 3] = [Center::Never, Center::Overflow, Center::Always];

    pub fn key(self) -> &'static str {
        match self {
            Center::Never => "never",
            Center::Overflow => "overflow",
            Center::Always => "always",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Apps {
    pub colors: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub app: String,
    pub floating: Option<bool>,
    pub width: Option<Width>,
    pub opacity: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Desktop {
    pub wallpaper: String,
    pub accent: Accent,
    pub theme: Option<String>,
    pub folders: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub gaps: u16,
    pub radius: u16,
    pub borders: bool,
    pub gradient: bool,
    pub shadows: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub workspaces: Workspaces,
    pub seconds: bool,
    pub date: bool,
    pub battery_percent: bool,
    pub position: Edge,
    pub floating: bool,
    pub opacity: f32,
    pub blur: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
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
    pub lock: String,
    pub clipboard: String,
    pub keys: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Launcher,
    Panel,
    Switch,
    Lock,
    Clipboard,
    Keys,
}

impl Shortcut {
    pub const ALL: [Shortcut; 6] = [
        Shortcut::Keys,
        Shortcut::Launcher,
        Shortcut::Panel,
        Shortcut::Clipboard,
        Shortcut::Switch,
        Shortcut::Lock,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Shortcut::Launcher => "launcher",
            Shortcut::Panel => "panel",
            Shortcut::Switch => "switch",
            Shortcut::Lock => "lock",
            Shortcut::Clipboard => "clipboard",
            Shortcut::Keys => "keys",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Idle {
    pub lock_after: u16,
    pub screen_off_after: u16,
    pub suspend_after: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lock {
    pub before_sleep: bool,
    pub show_media: bool,
}

impl Shortcuts {
    pub fn get(&self, shortcut: Shortcut) -> &str {
        match shortcut {
            Shortcut::Launcher => &self.launcher,
            Shortcut::Panel => &self.panel,
            Shortcut::Switch => &self.switch,
            Shortcut::Lock => &self.lock,
            Shortcut::Clipboard => &self.clipboard,
            Shortcut::Keys => &self.keys,
        }
    }

    pub fn get_mut(&mut self, shortcut: Shortcut) -> &mut String {
        match shortcut {
            Shortcut::Launcher => &mut self.launcher,
            Shortcut::Panel => &mut self.panel,
            Shortcut::Switch => &mut self.switch,
            Shortcut::Lock => &mut self.lock,
            Shortcut::Clipboard => &mut self.clipboard,
            Shortcut::Keys => &mut self.keys,
        }
    }
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            launcher: "Mod+Space".into(),
            panel: "Mod+A".into(),
            switch: "Mod+Shift+M".into(),
            lock: "Super+Alt+L".into(),
            clipboard: "Mod+Alt+V".into(),
            keys: "Mod+Slash".into(),
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
    pub name: String,
    pub icon: String,
    pub wallpaper: Option<String>,
    pub accent: Option<Accent>,
    pub notifications: Notifications,
    pub open: Vec<String>,
    pub performance: bool,
    pub keep_awake: bool,
    pub keys: Vec<(String, String)>,
}

pub const TEMPLATES: [&str; 5] = ["Work", "Play", "Evening", "Focus", "Presentation"];

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

    pub fn find(&self, key: &str) -> Option<usize> {
        let key = slug(key);
        self.modes.iter().position(|mode| mode.key() == key)
    }

    pub fn start(&self) -> Option<usize> {
        self.mode.as_deref().and_then(|key| self.find(key))
    }

    pub fn wallpaper(&self, mode: Option<usize>) -> &str {
        mode.and_then(|i| self.modes.get(i))
            .and_then(|m| m.wallpaper.as_deref())
            .unwrap_or(&self.desktop.wallpaper)
    }

    pub fn palette(&self, mode: Option<usize>) -> Palette {
        let accent = mode
            .and_then(|i| self.modes.get(i))
            .and_then(|m| m.accent)
            .unwrap_or(self.desktop.accent);
        Palette::new(match accent {
            Accent::Auto => auto_accent(self.wallpaper(mode)),
            Accent::Fixed(color) => color,
        })
    }

    pub fn notifications(&self, mode: Option<usize>) -> Notifications {
        mode.and_then(|i| self.modes.get(i))
            .map_or(Notifications::All, |m| m.notifications)
    }
}

static SEEN: RwLock<BTreeMap<String, Rgba>> = RwLock::new(BTreeMap::new());

pub fn remember_accent(wallpaper: &str, accent: Rgba) {
    if let Ok(mut seen) = SEEN.write() {
        seen.insert(wallpaper.to_owned(), accent);
    }
}

pub fn auto_accent(wallpaper: &str) -> Rgba {
    if let Some(accent) = SEEN
        .read()
        .ok()
        .and_then(|seen| seen.get(wallpaper).copied())
    {
        return accent;
    }
    match wallpaper {
        "ember.jpg" => ACCENTS[1],
        "night.jpg" => ACCENTS[2],
        _ => ACCENTS[0],
    }
}

pub fn slug(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: None,
            theme: None,
            desktop: Desktop {
                wallpaper: "fog.jpg".into(),
                accent: Accent::Auto,
                theme: None,
                folders: Vec::new(),
            },
            layout: Layout {
                gaps: 10,
                radius: 12,
                borders: true,
                gradient: false,
                shadows: true,
            },
            bar: Bar {
                workspaces: Workspaces::Dots,
                seconds: false,
                date: true,
                battery_percent: true,
                position: Edge::Top,
                floating: true,
                opacity: 0.55,
                blur: false,
            },
            keyboard: Keyboard::default(),
            touchpad: None,
            mouse: None,
            alerts: Alerts {
                do_not_disturb: false,
                seconds: 5,
            },
            shortcuts: Shortcuts::default(),
            lock: Lock {
                before_sleep: true,
                show_media: true,
            },
            idle: Idle {
                lock_after: 5,
                screen_off_after: 10,
                suspend_after: 0,
            },
            apps: Apps { colors: false },
            windows: Windows {
                width: None,
                presets: None,
                center: None,
                center_lone: false,
                focus_follows_mouse: false,
                warp_mouse: false,
                opacity: None,
                inactive_opacity: None,
                animations: None,
                motion: None,
                appear: None,
                blur: None,
                switcher: true,
            },
            rules: Vec::new(),
            pinned: Vec::new(),
            modes: Vec::new(),
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
    pub fn key(&self) -> String {
        slug(&self.name)
    }

    pub fn blank(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            icon: "sparkles".into(),
            wallpaper: None,
            accent: None,
            notifications: Notifications::All,
            open: Vec::new(),
            performance: false,
            keep_awake: false,
            keys: Vec::new(),
        }
    }

    pub fn template(name: &str) -> Self {
        let blank = Self::blank(name);
        let wallpaper = |w: &str| Some(w.to_owned());
        match slug(name).as_str() {
            "work" => Self {
                icon: "briefcase".into(),
                wallpaper: wallpaper("fog.jpg"),
                accent: Some(Accent::Auto),
                notifications: Notifications::Mentions,
                ..blank
            },
            "play" => Self {
                icon: "gamepad".into(),
                wallpaper: wallpaper("ember.jpg"),
                accent: Some(Accent::Auto),
                notifications: Notifications::Hold,
                open: vec!["steam".into()],
                performance: true,
                keep_awake: true,
                ..blank
            },
            "evening" => Self {
                icon: "moon".into(),
                wallpaper: wallpaper("night.jpg"),
                accent: Some(Accent::Auto),
                ..blank
            },
            "focus" => Self {
                icon: "target".into(),
                accent: Some(Accent::Fixed(ACCENTS[3])),
                notifications: Notifications::Hold,
                ..blank
            },
            "presentation" => Self {
                icon: "presentation".into(),
                accent: Some(Accent::Fixed(ACCENTS[4])),
                notifications: Notifications::Hold,
                keep_awake: true,
                ..blank
            },
            _ => blank,
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
        assert!(config.modes.is_empty(), "no modes until you make one");
        assert_eq!(config.start(), None);
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
    fn misspelled_section_and_mode_option() {
        let (_, diagnostics) =
            parse("[layut]\ngaps = 4\n\n[[mode]]\nname = \"Party\"\nwalpaper = \"x.jpg\"\n");
        let hints: Vec<_> = diagnostics
            .iter()
            .map(|d| (d.line, d.hint.clone()))
            .collect();
        assert_eq!(hints[0], (1, Some("did you mean “layout”?".into())));
        assert_eq!(hints[1], (6, Some("did you mean “wallpaper”?".into())));
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
        let (config, diagnostics) = parse(
            "[general]\nmode = \"evenin\"\n\n[[mode]]\nname = \"Work\"\nnotifications = \"mention\"\n",
        );
        assert_eq!(config.start(), None);
        assert_eq!(diagnostics[0].message, "there is no mode called “evenin”");
        assert_eq!(diagnostics[0].hint.as_deref(), Some("modes you have: Work"));
        assert_eq!(
            diagnostics[1].hint.as_deref(),
            Some("did you mean “mentions”?")
        );
    }

    #[test]
    fn modes_need_names_and_unique_ones() {
        let (config, diagnostics) = parse(
            "[[mode]]\nicon = \"moon\"\n\n[[mode]]\nname = \"Night Owl\"\nkeys = { \"Mod+N\" = \"foot\", \"Mod +\" = \"x\" }\n\n[[mode]]\nname = \"night owl\"\n\n[general]\nmode = \"Night Owl\"\n",
        );
        assert_eq!(config.modes.len(), 1);
        assert_eq!(config.modes[0].key(), "night-owl");
        assert_eq!(
            config.modes[0].keys,
            [("Mod+N".to_owned(), "foot".to_owned())]
        );
        assert_eq!(config.start(), Some(0));
        let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "a mode needs a name",
                "“Mod +” is not a key combination",
                "two modes are called “night owl”"
            ]
        );
    }

    #[test]
    fn accents_fall_back_from_mode_to_desktop() {
        let (config, diagnostics) = parse(
            "[desktop]\nwallpaper = \"night.jpg\"\n\n[[mode]]\nname = \"Deep\"\naccent = \"#3b2a7a\"\n\n[[mode]]\nname = \"Same\"\naccent = \"blue\"\n",
        );
        assert_eq!(config.palette(Some(0)).accent, Rgba::hex(0x3b2a7a));
        assert_eq!(config.modes[1].accent, None);
        assert_eq!(
            config.palette(Some(1)).accent,
            ACCENTS[2],
            "auto from night.jpg"
        );
        assert_eq!(config.palette(None).accent, ACCENTS[2]);
        assert_eq!(config.wallpaper(Some(0)), "night.jpg");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 10);
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
    fn windows_and_app_rules() {
        let (config, diagnostics) = parse(
            "[windows]\nwidth = \"2/3\"\npresets = [\"1/3\", \"1/2\"]\ncenter = \"overflow\"\nopacity = 0.95\ninactive_opacity = 0.8\nanimations = 0\n\n[[rule]]\napp = \"pavucontrol\"\nfloating = true\nwidth = \"1/3\"\n\n[[rule]]\nfloating = true\n",
        );
        assert_eq!(config.windows.width, Some(Width::TwoThirds));
        assert_eq!(
            config.windows.presets,
            Some(vec![Width::Third, Width::Half])
        );
        assert_eq!(config.windows.center, Some(Center::Overflow));
        assert_eq!(config.windows.inactive_opacity, Some(0.8));
        assert_eq!(config.windows.animations, Some(0.0));
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].floating, Some(true));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "a rule needs an app");

        let mut editor = Editor::parse("").unwrap();
        editor.add_rule(&config.rules[0]);
        editor.set("windows", "opacity", 0.9);
        let (again, problems) = parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(again.rules, config.rules);
        assert!(
            editor
                .lists("rule")
                .starts_with("[[rule]]\napp = \"pavucontrol\"")
        );
        editor.remove_rule(0);
        editor.unset("windows", "opacity");
        assert_eq!(parse(&editor.text()).0.windows.opacity, None);
    }

    #[test]
    fn broken_syntax_is_reported_and_the_rest_applies() {
        let (config, diagnostics) = parse("[layout]\ngaps = \nborders = false\n");
        assert!(diagnostics.iter().any(|d| d.line == 2), "{diagnostics:?}");
        assert!(!config.layout.borders);
    }
}

mod diagnostic;
mod edit;
mod pieces;
mod read;
mod theme;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use apolline_theme::{ACCENTS, Palette, Rgba, Scheme, palettes};
pub use diagnostic::Diagnostic;
pub use edit::Editor;
pub use pieces::{Piece, TrayItem};
pub use theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Option<String>,
    // "ru", "en" or none for the system's.
    pub language: Option<String>,
    pub theme: Option<(String, Option<String>)>,
    pub desktop: Desktop,
    pub layout: Layout,
    pub bar: Bar,
    pub keyboard: Keyboard,
    pub touchpad: Option<Touchpad>,
    pub mouse: Option<Mouse>,
    pub alerts: Alerts,
    pub night: Night,
    pub style: Styles,
    pub shortcuts: Shortcuts,
    pub lock: Lock,
    pub idle: Idle,
    pub apps: Apps,
    pub windows: Windows,
    pub rules: Vec<Rule>,
    pub displays: Vec<Display>,
    pub modules: Vec<Module>,
    pub templates: Vec<Template>,
    pub pinned: Vec<String>,
    pub weather: Option<String>,
    // Where the clock's calendar gets its events.
    pub calendars: Vec<Calendar>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub radius: Option<u16>,
    pub opacity: Option<f32>,
    pub size: Option<f32>,
    pub font: Option<FontName>,
}

// A font family by name, kept once for the life of the program: the renderer wants names that
// last, and a config read again and again names the same few.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontName(&'static str);

impl FontName {
    pub fn new(name: &str) -> FontName {
        static KNOWN: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
        let name = name.trim();
        let Ok(mut known) = KNOWN.lock() else {
            return FontName(Box::leak(name.to_owned().into_boxed_str()));
        };
        if let Some(kept) = known.iter().find(|kept| **kept == name) {
            return FontName(kept);
        }
        let kept: &'static str = Box::leak(name.to_owned().into_boxed_str());
        known.push(kept);
        FontName(kept)
    }

    pub fn name(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Styles {
    pub launcher: Surface,
    pub panel: Surface,
    // The bar's glass is [bar] opacity; here its size and corners.
    pub bar: Surface,
    pub cards: Surface,
    pub osd: Surface,
    pub grid: bool,
}

// A place on the screen for something that isn't in a corner only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Center,
    Corner(Corner),
}

impl Spot {
    pub fn named(name: &str) -> Option<Spot> {
        if name == "center" {
            return Some(Spot::Center);
        }
        Corner::ALL
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, corner)| Spot::Corner(*corner))
    }

    pub fn key(self) -> &'static str {
        match self {
            Spot::Center => "center",
            Spot::Corner(corner) => corner.key(),
        }
    }
}

// Where notification cards come up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Corner {
    #[default]
    TopRight,
    Top,
    TopLeft,
    BottomRight,
    Bottom,
    BottomLeft,
}

impl Corner {
    pub const ALL: [(&'static str, Corner); 6] = [
        ("top-left", Corner::TopLeft),
        ("top", Corner::Top),
        ("top-right", Corner::TopRight),
        ("bottom-left", Corner::BottomLeft),
        ("bottom", Corner::Bottom),
        ("bottom-right", Corner::BottomRight),
    ];

    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, corner)| *corner == self)
            .map_or("top-right", |(key, _)| key)
    }

    pub fn bottom(self) -> bool {
        matches!(
            self,
            Corner::BottomRight | Corner::Bottom | Corner::BottomLeft
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Night {
    pub hours: Option<(u16, u16)>,
    pub temperature: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Apps {
    pub colors: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Module {
    // What [bar] left, center and right call it, to place it on its own.
    pub name: Option<String>,
    pub command: String,
    pub every: Option<u32>,
    pub click: Option<String>,
}

// A file of yours with {{accent}}, {{background}} and the like, filled in from the palette and
// written where an app reads its colors, whenever they change.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Template {
    pub from: String,
    pub to: String,
    // A command to run after it's written, so the app picks it up.
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Calendar {
    // A file, or an https or webcal link.
    Ics(String),
    // A CalDAV collection. Its password is kept in the Secret Service, never in this file.
    CalDav { url: String, user: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    pub mode: Option<String>,
    pub scale: Option<f64>,
    pub transform: Option<String>,
    pub position: Option<(i32, i32)>,
    pub vrr: Option<bool>,
}

pub const TRANSFORMS: [&str; 8] = [
    "normal",
    "90",
    "180",
    "270",
    "flipped",
    "flipped-90",
    "flipped-180",
    "flipped-270",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub app: String,
    pub floating: Option<bool>,
    pub width: Option<Width>,
    pub opacity: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Desktop {
    // A big clock with the date on the wallpaper, and where.
    pub clock: Option<Spot>,
    pub wallpaper: String,
    pub accent: Accent,
    // A named palette, or "wallpaper" / "wallpaper-light" for one made from the wallpaper's color.
    pub palette: String,
    pub theme: Option<String>,
    pub folders: Vec<String>,
    pub rotate: u32,
    pub shuffle: bool,
    pub outputs: Vec<(String, String)>,
    pub cursor: Option<String>,
    pub cursor_size: Option<u16>,
    pub icons: Option<String>,
    pub font: Option<String>,
    pub gtk: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub gaps: u16,
    pub radius: u16,
    pub borders: bool,
    pub border_width: u16,
    pub gradient: bool,
    pub shadows: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub workspaces: Workspaces,
    pub seconds: bool,
    pub date: bool,
    pub battery_percent: bool,
    pub position: Edge,
    pub floating: bool,
    pub shape: Shape,
    pub opacity: f32,
    pub blur: bool,
    pub stats: Stats,
    pub left: Vec<Piece>,
    pub center: Vec<Piece>,
    pub right: Vec<Piece>,
    pub tray: Vec<TrayItem>,
}

// What the bar shows of the machine itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub cpu: bool,
    pub memory: bool,
    pub temperature: bool,
}

impl Stats {
    pub const ALL: [&'static str; 3] = ["cpu", "memory", "temperature"];

    pub fn any(self) -> bool {
        self.cpu || self.memory || self.temperature
    }

    pub fn names(self) -> Vec<String> {
        Self::ALL
            .iter()
            .zip([self.cpu, self.memory, self.temperature])
            .filter(|(_, on)| *on)
            .map(|(name, _)| (*name).to_owned())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub const ALL: [(&'static str, Edge); 4] = [
        ("top", Edge::Top),
        ("bottom", Edge::Bottom),
        ("left", Edge::Left),
        ("right", Edge::Right),
    ];

    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, edge)| *edge == self)
            .map_or("top", |(key, _)| key)
    }

    // Down the side of the screen, so the bar is a column.
    pub fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

// One strip of glass, a pill for each side and the middle with the wallpaper between them, or
// everything in one pill in the middle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Bar,
    Islands,
    Pill,
}

impl Shape {
    pub fn key(self) -> &'static str {
        match self {
            Shape::Bar => "bar",
            Shape::Islands => "islands",
            Shape::Pill => "pill",
        }
    }
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
    pub replay: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Launcher,
    Panel,
    Switch,
    Lock,
    Clipboard,
    Keys,
    Replay,
}

impl Shortcut {
    pub const ALL: [Shortcut; 7] = [
        Shortcut::Keys,
        Shortcut::Launcher,
        Shortcut::Panel,
        Shortcut::Clipboard,
        Shortcut::Switch,
        Shortcut::Lock,
        Shortcut::Replay,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Shortcut::Launcher => "launcher",
            Shortcut::Panel => "panel",
            Shortcut::Switch => "switch",
            Shortcut::Lock => "lock",
            Shortcut::Clipboard => "clipboard",
            Shortcut::Keys => "keys",
            Shortcut::Replay => "replay",
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
    pub fingerprint: bool,
    // The clock and the password in the bottom left, or both in the middle.
    pub centered: bool,
    pub blur: bool,
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
            Shortcut::Replay => &self.replay,
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
            Shortcut::Replay => &mut self.replay,
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
            replay: "Mod+Alt+R".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alerts {
    pub position: Corner,
    pub do_not_disturb: bool,
    pub seconds: u16,
    pub mute: Vec<String>,
    pub always: Vec<String>,
}

impl Alerts {
    pub fn muted(&self, app: &str) -> bool {
        self.mute.iter().any(|name| name.eq_ignore_ascii_case(app))
    }

    pub fn always(&self, app: &str) -> bool {
        self.always
            .iter()
            .any(|name| name.eq_ignore_ascii_case(app))
    }
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
    pub when: Vec<Trigger>,
    pub theme: Option<String>,
    // Seconds of the screen kept while the mode is on, for `apolline replay` to save; 0 is off.
    pub replay: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    Game,
    Battery,
    Hours { from: u16, until: u16 },
    Monitor(String),
}

impl Trigger {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text {
            "game" => return Some(Trigger::Game),
            "battery" => return Some(Trigger::Battery),
            _ => {}
        }
        if let Some(name) = text.strip_prefix("monitor ") {
            let name = name.trim();
            return (!name.is_empty()).then(|| Trigger::Monitor(name.to_owned()));
        }
        let (from, until) = text.split_once('-')?;
        Some(Trigger::Hours {
            from: minutes(from)?,
            until: minutes(until)?,
        })
    }

    pub fn text(&self) -> String {
        match self {
            Trigger::Game => "game".into(),
            Trigger::Battery => "battery".into(),
            Trigger::Hours { from, until } => format!("{}-{}", clock(*from), clock(*until)),
            Trigger::Monitor(name) => format!("monitor {name}"),
        }
    }

    pub fn covers(from: u16, until: u16, now: u16) -> bool {
        if from <= until {
            (from..until).contains(&now)
        } else {
            now >= from || now < until
        }
    }
}

fn minutes(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

pub const TEMPLATES: [&str; 5] = ["Work", "Play", "Evening", "Focus", "Presentation"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accent {
    Auto,
    Fixed(Rgba),
    // The one the palette is known for.
    Palette,
}

pub const WALLPAPER_PALETTES: [&str; 2] = ["wallpaper", "wallpaper-light"];

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
        let auto = || auto_accent(self.wallpaper(mode));
        Palette::new(match accent {
            Accent::Auto => auto(),
            Accent::Fixed(color) => color,
            Accent::Palette => palettes::named(&self.desktop.palette).map_or_else(auto, |(_, a)| a),
        })
    }

    // The colors to draw with: a named palette, or neutrals tinted with the wallpaper's color.
    pub fn scheme(&self, mode: Option<usize>) -> Scheme {
        match self.desktop.palette.as_str() {
            "wallpaper" => palettes::tonal(auto_accent(self.wallpaper(mode)), false),
            "wallpaper-light" => palettes::tonal(auto_accent(self.wallpaper(mode)), true),
            name => palettes::named(name).map_or(Scheme::DARK, |(scheme, _)| scheme),
        }
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
            language: None,
            theme: None,
            desktop: Desktop {
                clock: None,
                wallpaper: "fog.jpg".into(),
                accent: Accent::Auto,
                palette: "dark".into(),
                theme: None,
                folders: Vec::new(),
                rotate: 0,
                shuffle: false,
                outputs: Vec::new(),
                cursor: None,
                cursor_size: None,
                icons: None,
                font: None,
                gtk: None,
            },
            layout: Layout {
                gaps: 10,
                radius: 12,
                borders: true,
                border_width: 1,
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
                shape: Shape::Bar,
                opacity: 0.55,
                blur: false,
                stats: Stats::default(),
                left: Piece::left(),
                center: Piece::center(),
                right: Piece::right(),
                tray: TrayItem::all(),
            },
            keyboard: Keyboard::default(),
            touchpad: None,
            mouse: None,
            alerts: Alerts {
                position: Corner::TopRight,
                do_not_disturb: false,
                seconds: 5,
                mute: Vec::new(),
                always: Vec::new(),
            },
            shortcuts: Shortcuts::default(),
            lock: Lock {
                before_sleep: true,
                show_media: true,
                fingerprint: false,
                centered: false,
                blur: false,
            },
            idle: Idle {
                lock_after: 5,
                screen_off_after: 10,
                suspend_after: 0,
            },
            apps: Apps { colors: false },
            style: Styles::default(),
            night: Night {
                hours: None,
                temperature: 4000,
            },
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
            displays: Vec::new(),
            modules: Vec::new(),
            templates: Vec::new(),
            pinned: Vec::new(),
            weather: None,
            calendars: Vec::new(),
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
            when: Vec::new(),
            theme: None,
            replay: 0,
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
                replay: 30,
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

    #[test]
    fn a_surface_can_have_its_own_font() {
        let (config, problems) = parse("[style.bar]\nfont = \"JetBrains Mono\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        let font = config.style.bar.font.unwrap();
        assert_eq!(font.name(), "JetBrains Mono");
        assert!(
            std::ptr::eq(font.name(), FontName::new("JetBrains Mono").name()),
            "kept once"
        );
    }

    #[test]
    fn the_desktop_clock_goes_where_it_is_put() {
        let (config, problems) = parse("[desktop]\nclock = \"bottom-left\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(config.desktop.clock, Some(Spot::Corner(Corner::BottomLeft)));
        assert_eq!(
            parse("[desktop]\nclock = \"center\"\n").0.desktop.clock,
            Some(Spot::Center)
        );
        assert_eq!(parse("[desktop]\nclock = \"off\"\n").0.desktop.clock, None);
    }

    #[test]
    fn templates_need_somewhere_to_come_from_and_go() {
        let (config, problems) = parse(
            "[[template]]\nfrom = \"~/t/btop.theme\"\nto = \"~/.config/btop/themes/apolline.theme\"\nafter = \"pkill -USR2 btop\"\n\n[[template]]\nfrom = \"~/t/x\"\n",
        );
        assert_eq!(config.templates.len(), 1);
        assert_eq!(
            config.templates[0].after.as_deref(),
            Some("pkill -USR2 btop")
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("from and to"));
    }

    #[test]
    fn a_palette_names_the_colors_and_can_lend_its_accent() {
        let (config, problems) =
            parse("[desktop]\npalette = \"catppuccin-mocha\"\naccent = \"palette\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(config.scheme(None).text, Rgba::hex(0xcdd6f4));
        assert_eq!(config.palette(None).accent, Rgba::hex(0xcba6f7));
        let (light, _) = parse("[desktop]\npalette = \"wallpaper-light\"\n");
        assert!(light.scheme(None).light);
        let (_, problems) = parse("[desktop]\npalette = \"catpuccin-mocha\"\n");
        assert_eq!(
            problems[0].hint.as_deref(),
            Some("did you mean “catppuccin-mocha”?")
        );
    }

    #[test]
    fn cards_and_popups_are_placed_and_styled() {
        let (config, problems) = parse(
            "[notifications]\nposition = \"bottom-left\"\n\n[style.notifications]\nradius = 4\nopacity = 0.8\n\n[style.osd]\nsize = 1.2\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(config.alerts.position, Corner::BottomLeft);
        assert_eq!(config.style.cards.radius, Some(4));
        assert_eq!(config.style.cards.opacity, Some(0.8));
        assert_eq!(config.style.osd.size, Some(1.2));
        let (_, problems) = parse("[notifications]\nposition = \"left\"\n");
        assert_eq!(problems.len(), 1);
    }

    fn parse(src: &str) -> (Config, Vec<Diagnostic>) {
        Config::parse(src)
    }

    #[test]
    fn apps_can_be_muted_or_always_shown() {
        let (config, problems) =
            Config::parse("[notifications]\nmute = [\"discord\"]\nalways = [\"Telegram\"]\n");
        assert_eq!(problems, vec![]);
        assert!(config.alerts.muted("Discord"));
        assert!(config.alerts.always("telegram"));
        assert!(!config.alerts.muted("slack"));
    }

    #[test]
    fn calendars_are_files_links_or_caldav() {
        let (config, problems) = Config::parse(
            "[bar]\ncalendars = [\n  \"~/home.ics\",\n  { caldav = \"https://dav.example.org/me/\", user = \"me\", password = \"x\", colour = \"red\" },\n  { caldav = \"https://dav.example.org/them/\" },\n]\n",
        );
        assert_eq!(
            config.calendars,
            [
                Calendar::Ics("~/home.ics".into()),
                Calendar::CalDav {
                    url: "https://dav.example.org/me/".into(),
                    user: "me".into()
                }
            ]
        );
        let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "unknown option “colour”",
                "passwords aren't read from this file",
                "a CalDAV calendar needs caldav and user"
            ]
        );
        assert_eq!(problems[2].line, 5);

        let (config, problems) = Config::parse("[bar]\ncalendars = [\"a.ics\", 5]\n");
        assert_eq!(
            problems[0].message,
            r#"“calendars” should be a list of files and links, or { caldav = "…", user = "…" }"#
        );
        assert!(config.calendars.is_empty());
    }

    #[test]
    fn surfaces_take_their_own_look() {
        let (config, problems) = Config::parse(
            "[style.launcher]\nradius = 8\nsize = 1.2\ngrid = true\n\n[style.panel]\nopacity = 0.9\nsize = 3\n",
        );
        assert_eq!(problems.len(), 1, "a panel three times the size is refused");
        assert_eq!(config.style.launcher.size, Some(1.2));
        assert_eq!(config.style.launcher.radius, Some(8));
        assert!(config.style.grid);
        assert_eq!(config.style.panel.size, None);
        assert_eq!(config.style.panel.opacity, Some(0.9));
    }

    #[test]
    fn modes_say_when_they_switch_themselves_on() {
        let (config, problems) = Config::parse(
            "[[mode]]\nname = \"Play\"\nwhen = [\"game\", \"monitor HDMI-A-1\"]\n\n[[mode]]\nname = \"Evening\"\nwhen = [\"20:00-07:00\", \"battery\"]\n\n[[mode]]\nname = \"Bad\"\nwhen = [\"25:00-01:00\"]\n",
        );
        assert_eq!(
            config.modes[0].when,
            [Trigger::Game, Trigger::Monitor("HDMI-A-1".into())]
        );
        assert_eq!(
            config.modes[1].when,
            [
                Trigger::Hours {
                    from: 1200,
                    until: 420
                },
                Trigger::Battery
            ]
        );
        assert_eq!(config.modes[1].when[0].text(), "20:00-07:00");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(Trigger::covers(1200, 420, 1380) && Trigger::covers(1200, 420, 60));
        assert!(!Trigger::covers(1200, 420, 720));
        assert!(Trigger::covers(540, 1080, 600) && !Trigger::covers(540, 1080, 1080));
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
        assert_eq!(
            config.palette(Some(0)).accent,
            palettes::readable(Rgba::hex(0x3b2a7a), &Scheme::DARK),
            "too dark to read, so lightened"
        );
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

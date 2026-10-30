use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use apolline_config::{
    Accent, Appear, Blur, Center, Config, Diagnostic, Edge, Editor, ModeSettings, Motion,
    Notifications, Rule, Shortcut, Switch, TEMPLATES, Theme as Look, Trigger, Width, Workspaces,
    auto_accent,
};
use apolline_theme::{ACCENTS, Palette, Rgba, font, neutral};
use iced::keyboard;
use iced::widget::text::Span;
use iced::widget::{
    Column, Row, Space, button, column, container, image, pick_list, responsive, rich_text, row,
    scrollable, slider, space, span, stack, text, text_input, toggler,
};
use iced::{
    Alignment, Background, Border, ContentFit, Element, Fill, Font, Length, Shadow, Subscription,
    Task, Theme, Vector, window,
};

use crate::apps::{self, App};
use crate::controls::Controls;
use crate::fragment::{self, Fragment};
use crate::icon::{Icon, MODE_ICONS, icon};
use crate::status::Status;
use crate::{bar, fonts, keys, notify, shot, style, themes, wallpaper};

const WINDOW: Rgba = Rgba::hex(0x111316);
const SIDEBAR: Rgba = Rgba::hex(0x16181c);
const HISTORY: usize = 100;
const WALLS: usize = 11;
const PER_ROW: usize = 4;
const WIDE: f32 = 1120.0;
const NARROW: f32 = 860.0;
const STRING: Rgba = Rgba::hex(0xa8d8a0);

const LAYOUTS: [(&str, &str); 24] = [
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("ru", "Russian"),
    ("ua", "Ukrainian"),
    ("by", "Belarusian"),
    ("kz", "Kazakh"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("br", "Portuguese (Brazil)"),
    ("pl", "Polish"),
    ("cz", "Czech"),
    ("se", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("fi", "Finnish"),
    ("tr", "Turkish"),
    ("gr", "Greek"),
    ("il", "Hebrew"),
    ("ara", "Arabic"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
];

pub fn run(screenshot: Option<PathBuf>, page: Option<String>) -> iced::Result {
    let app = iced::application(
        move || Settings::new(screenshot.clone(), page.as_deref()),
        Settings::update,
        Settings::view,
    )
    .title("Apolline Settings")
    .subscription(Settings::subscription)
    .theme(Theme::Dark)
    .default_font(Font::with_name(font::UI))
    .window(window::Settings {
        size: iced::Size::new(1280.0, 820.0),
        min_size: Some(iced::Size::new(560.0, 480.0)),
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: "apolline-settings".into(),
            ..Default::default()
        },
        ..window::Settings::default()
    });
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Modes,
    Themes,
    Appearance,
    Windows,
    Bar,
    Lock,
    Notifications,
    Keyboard,
    Pointer,
    Shortcuts,
    About,
    Welcome,
}

impl Page {
    const ALL: [Page; 11] = [
        Page::Modes,
        Page::Themes,
        Page::Appearance,
        Page::Windows,
        Page::Bar,
        Page::Lock,
        Page::Notifications,
        Page::Keyboard,
        Page::Pointer,
        Page::Shortcuts,
        Page::About,
    ];

    fn key(self) -> &'static str {
        match self {
            Page::Modes => "modes",
            Page::Themes => "themes",
            Page::Appearance => "appearance",
            Page::Windows => "windows",
            Page::Bar => "bar",
            Page::Lock => "lock",
            Page::Notifications => "notifications",
            Page::Keyboard => "keyboard",
            Page::Pointer => "pointer",
            Page::Shortcuts => "shortcuts",
            Page::About => "about",
            Page::Welcome => "welcome",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Page::Modes => "Modes",
            Page::Themes => "Themes",
            Page::Appearance => "Appearance",
            Page::Windows => "Windows",
            Page::Bar => "Bar",
            Page::Lock => "Lock screen",
            Page::Notifications => "Notifications",
            Page::Keyboard => "Keyboard",
            Page::Pointer => "Mouse & touchpad",
            Page::Shortcuts => "Shortcuts",
            Page::About => "About",
            Page::Welcome => "Welcome to Apolline",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Page::Modes => "One key changes wallpaper, apps, notifications and performance.",
            Page::Themes => "A whole look in one click, and yours in one file to share.",
            Page::Appearance => "Gaps, corners, borders and shadows of every window.",
            Page::Windows => {
                "How windows open, move, fade and animate, for all of them or one app."
            }
            Page::Bar => "What the bar at the top shows.",
            Page::Lock => "What the screen shows while you're away, and when it locks.",
            Page::Notifications => "When popups show up and how long they stay.",
            Page::Keyboard => "Layouts, how to switch them and how keys repeat.",
            Page::Pointer => "Tapping, scrolling and pointer speed.",
            Page::Shortcuts => "Keys that open Apolline, and what they replace.",
            Page::About => "Version, setup and a health check of your config.",
            Page::Welcome => {
                "Four quick steps, all optional. Everything here can be changed later."
            }
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Modes => Icon::Layers,
            Page::Themes => Icon::Brush,
            Page::Appearance => Icon::Palette,
            Page::Windows => Icon::AppWindow,
            Page::Bar => Icon::PanelTop,
            Page::Lock => Icon::Lock,
            Page::Notifications => Icon::Bell,
            Page::Keyboard => Icon::Keyboard,
            Page::Pointer => Icon::Pointer,
            Page::Shortcuts => Icon::Command,
            Page::About => Icon::Info,
            Page::Welcome => Icon::Sparkles,
        }
    }

    fn sections(self) -> &'static [&'static str] {
        match self {
            Page::Modes | Page::About | Page::Welcome => &[],
            Page::Themes => &["desktop", "layout", "windows", "bar"],
            Page::Appearance => &["desktop", "layout", "apps"],
            Page::Windows => &["windows"],
            Page::Bar => &["bar"],
            Page::Lock => &["lock", "idle"],
            Page::Notifications => &["notifications"],
            Page::Keyboard => &["keyboard"],
            Page::Pointer => &["touchpad", "mouse"],
            Page::Shortcuts => &["shortcuts"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout(&'static str, &'static str);

impl std::fmt::Display for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}  ·  {}", self.1, self.0)
    }
}

#[derive(Clone, Debug)]
enum Message {
    Page(Page),
    Select(usize),
    Adding(bool),
    AddMode(&'static str),
    RemoveMode,
    MoveMode(isize),
    ModeName(String),
    ModeNameDone,
    ModeIcon(&'static str),
    ModeWallpaper(Option<String>),
    ModeAccent(Option<Accent>),
    DesktopWallpaper(String),
    DesktopAccent(Accent),
    WallpaperDraft(String),
    UseWallpaper,
    AllWallpapers,
    ColorApps(bool),
    Apps(Vec<App>),
    PinQuery(String),
    WeatherDraft(String),
    SetWeather,
    PinApp(String),
    UnpinApp(usize),
    RemoveFolder(usize),
    Notifications(usize, Notifications),
    Performance(bool),
    KeepAwake(bool),
    WhenGame(bool),
    WhenBattery(bool),
    HoursDraft(String),
    MonitorDraft(String),
    SetHours,
    SetMonitor,
    DropTrigger(usize),
    StartHere(bool),
    KeyCommand(String),
    RecordModeKey,
    AddModeKey,
    RemoveModeKey(usize),
    Draft(String),
    Add,
    Remove(usize),
    Gaps(u16),
    Radius(u16),
    BorderStyle(u8),
    Shadows(bool),
    Workspaces(Workspaces),
    Seconds(bool),
    BarEdge(Edge),
    BarFloating(bool),
    BarOpacity(u16),
    BarBlur(bool),
    Date(bool),
    BatteryPercent(bool),
    DoNotDisturb(bool),
    PopupSeconds(u16),
    Test,
    Tested(Result<(), String>),
    AddLayout(Layout),
    RemoveLayout(usize),
    Switch(Switch),
    RepeatDelay(u16),
    RepeatRate(u16),
    Typing(String),
    Tap(bool),
    TouchNatural(bool),
    TouchTyping(bool),
    TouchSpeed(f32),
    MouseNatural(bool),
    MouseAcceleration(bool),
    MouseSpeed(f32),
    Record(Option<Shortcut>),
    Key(keyboard::Event),
    ResetShortcut(Shortcut),
    LockBeforeSleep(bool),
    LockMedia(bool),
    LockNow,
    IdleLock(u16),
    IdleScreen(u16),
    IdleSuspend(u16),
    WinWidth(Width),
    WinPreset(Width),
    WinCenter(Center),
    CenterLone(bool),
    RecentWindows(bool),
    FocusFollows(bool),
    WarpMouse(bool),
    Opacity(u16),
    InactiveOpacity(u16),
    Animations(u8),
    WinMotion(Option<Motion>),
    WinAppear(Option<Appear>),
    WinBlur(Option<Blur>),
    RuleApp(String),
    RuleFloating(bool),
    RuleWidth(Width),
    AddRule,
    RemoveRule(usize),
    ApplyTheme(usize),
    ThemeName(String),
    SaveTheme,
    ThemePath(String),
    ImportTheme,
    OpenThemes,
    Setup,
    Finish,
    OpenFile,
    Commit,
    Thumbs(Vec<(String, image::Handle)>),
    Thumb(String, Option<image::Handle>),
    Undo,
    Redo,
    Tick,
    Shot(window::Screenshot),
    Noop,
}

struct Settings {
    path: PathBuf,
    stamp: Option<SystemTime>,
    editor: Result<Editor, String>,
    saved: String,
    undo: Vec<String>,
    redo: Vec<String>,
    config: Config,
    diagnostics: Vec<Diagnostic>,
    page: Page,
    selected: usize,
    adding: bool,
    armed: bool,
    name_draft: Option<String>,
    key_combo: Option<String>,
    key_command: String,
    recording_mode_key: bool,
    rule: Rule,
    themes: Vec<(Look, Option<PathBuf>)>,
    theme_name: String,
    theme_path: String,
    theme_note: Option<Result<String, String>>,
    draft: String,
    wallpaper_draft: String,
    library: Vec<String>,
    apps: Vec<App>,
    pin_query: String,
    weather_draft: Option<String>,
    live: crate::control::Shared,
    hours_draft: String,
    monitor_draft: String,
    queue: Vec<String>,
    all_walls: bool,
    typing: String,
    thumbs: Vec<(String, image::Handle)>,
    recording: Option<Shortcut>,
    shortcut_error: Option<String>,
    niri_binds: Vec<(String, String)>,
    sample: Status,
    controls: Controls,
    setup_log: Vec<String>,
    tested: Option<Result<(), String>>,
    error: Option<String>,
    screenshot: Option<PathBuf>,
    shooting: bool,
}

impl Settings {
    fn new(screenshot: Option<PathBuf>, page: Option<&str>) -> (Self, Task<Message>) {
        let (mut settings, task) = Self::open(apolline_config::default_path(), screenshot);
        if let Some(page) = page.and_then(|p| {
            Page::ALL
                .into_iter()
                .chain([Page::Welcome])
                .find(|page| page.key() == p)
        }) {
            settings.page = page;
        }
        (settings, task)
    }

    fn open(path: PathBuf, screenshot: Option<PathBuf>) -> (Self, Task<Message>) {
        let editor = Editor::load(&path);
        let (config, diagnostics) = Config::load(&path);
        let settings = Self {
            stamp: stamp(&path),
            path,
            saved: editor.as_ref().map(Editor::text).unwrap_or_default(),
            undo: Vec::new(),
            redo: Vec::new(),
            editor,
            selected: config.start().unwrap_or(0),
            adding: false,
            armed: false,
            name_draft: None,
            key_combo: None,
            key_command: String::new(),
            recording_mode_key: false,
            rule: Rule {
                app: String::new(),
                floating: Some(true),
                width: None,
                opacity: None,
            },
            themes: themes::all(),
            theme_name: String::new(),
            theme_path: String::new(),
            theme_note: None,
            config,
            diagnostics,
            page: Page::Modes,
            draft: String::new(),
            wallpaper_draft: String::new(),
            library: Vec::new(),
            apps: Vec::new(),
            pin_query: String::new(),
            weather_draft: None,
            live: crate::control::shared(),
            hours_draft: String::new(),
            monitor_draft: String::new(),
            queue: Vec::new(),
            all_walls: false,
            typing: String::new(),
            thumbs: Vec::new(),
            recording: None,
            shortcut_error: None,
            niri_binds: read_niri_binds(),
            sample: Status::sample(),
            controls: Controls::sample(),
            setup_log: Vec::new(),
            tested: None,
            error: None,
            screenshot,
            shooting: false,
        };
        let mut settings = settings;
        let custom = settings.custom_thumb();
        let library = settings.scan_library();
        (
            settings,
            Task::batch([
                Task::perform(thumbnails(), Message::Thumbs),
                custom,
                library,
                Task::perform(apps::scan(), Message::Apps),
            ]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let index = self.selected;
        match message {
            Message::Page(page) => {
                self.page = page;
                self.recording = None;
                self.recording_mode_key = false;
                self.armed = false;
            }
            Message::Select(index) => {
                self.selected = index;
                self.adding = false;
                self.armed = false;
                self.name_draft = None;
                self.key_combo = None;
                return self.custom_thumb();
            }
            Message::Adding(adding) => self.adding = adding,
            Message::AddMode(template) => {
                let base = if template.is_empty() {
                    "New mode"
                } else {
                    template
                };
                let mut name = base.to_owned();
                let mut n = 2;
                while self.config.find(&name).is_some() {
                    name = format!("{base} {n}");
                    n += 1;
                }
                let mode = if template.is_empty() {
                    ModeSettings::blank(&name)
                } else {
                    ModeSettings {
                        name,
                        ..ModeSettings::template(template)
                    }
                };
                let mut added = None;
                self.edit(|e| added = Some(e.add_mode(&mode)));
                self.selected = added.unwrap_or(0);
                self.adding = false;
                return self.custom_thumb();
            }
            Message::RemoveMode => {
                if !self.armed {
                    self.armed = true;
                    return Task::none();
                }
                let starts_here = self.config.start() == Some(index);
                self.edit(|e| {
                    if starts_here {
                        e.set("general", "mode", "");
                    }
                    e.remove_mode(index);
                });
                self.selected = index.saturating_sub(1);
                self.armed = false;
            }
            Message::MoveMode(by) => {
                let to = index as isize + by;
                if to >= 0 && (to as usize) < self.config.modes.len() {
                    self.edit(|e| e.move_mode(index, to as usize));
                    self.selected = to as usize;
                }
            }
            Message::ModeName(name) => self.name_draft = Some(name),
            Message::ModeNameDone => {
                let Some(name) = self.name_draft.take().map(|n| n.trim().to_owned()) else {
                    return Task::none();
                };
                let taken = self.config.find(&name).is_some_and(|other| other != index);
                if name.is_empty() || taken {
                    self.error = Some(if taken {
                        format!("there is already a mode called “{name}”")
                    } else {
                        "a mode needs a name".to_owned()
                    });
                    return Task::none();
                }
                let starts_here = self.config.start() == Some(index);
                self.edit(|e| {
                    e.set_mode(index, "name", name.as_str());
                    if starts_here {
                        e.set("general", "mode", name.as_str());
                    }
                });
            }
            Message::ModeIcon(name) => self.edit(|e| e.set_mode(index, "icon", name)),
            Message::ModeWallpaper(wallpaper) => {
                self.edit(|e| match &wallpaper {
                    Some(name) => e.set_mode(index, "wallpaper", name.as_str()),
                    None => e.unset_mode(index, "wallpaper"),
                });
                return self.custom_thumb();
            }
            Message::ModeAccent(accent) => self.edit(|e| e.set_mode_accent(index, accent)),
            Message::DesktopWallpaper(name) => {
                self.edit(|e| e.set("desktop", "wallpaper", name.as_str()));
                return self.custom_thumb();
            }
            Message::DesktopAccent(accent) => self.edit(|e| e.set_accent(accent)),
            Message::WallpaperDraft(draft) => self.wallpaper_draft = draft,
            Message::AllWallpapers => self.all_walls = !self.all_walls,
            Message::ColorApps(on) => self.edit(|e| {
                if on {
                    e.set("apps", "colors", true);
                } else {
                    e.unset("apps", "colors");
                }
            }),
            Message::Apps(apps) => self.apps = apps,
            Message::PinQuery(query) => self.pin_query = query,
            Message::WeatherDraft(draft) => self.weather_draft = Some(draft),
            Message::SetWeather => {
                if let Some(place) = self.weather_draft.take() {
                    let place = place.trim().to_owned();
                    self.edit(|e| {
                        if place.is_empty() {
                            e.unset("bar", "weather");
                        } else {
                            e.set("bar", "weather", place.as_str());
                        }
                    });
                }
            }
            Message::PinApp(id) => {
                let mut pinned = self.config.pinned.clone();
                if !pinned.contains(&id) {
                    pinned.push(id);
                    self.edit(|e| e.set_list("bar", "pinned", &pinned));
                }
                self.pin_query.clear();
            }
            Message::UnpinApp(index) => {
                let mut pinned = self.config.pinned.clone();
                if index < pinned.len() {
                    pinned.remove(index);
                    self.edit(|e| e.set_list("bar", "pinned", &pinned));
                }
            }
            Message::RemoveFolder(index) => {
                let mut folders = self.config.desktop.folders.clone();
                if index < folders.len() {
                    folders.remove(index);
                    self.edit(|e| e.set_list("desktop", "folders", &folders));
                    return self.scan_library();
                }
            }
            Message::UseWallpaper => {
                let path = self.wallpaper_draft.trim().to_owned();
                let folder = wallpaper::expand(&path).filter(|p| p.is_dir());
                match wallpaper::resolve(&path) {
                    _ if path.is_empty() => {}
                    _ if folder.is_some() => {
                        let mut folders = self.config.desktop.folders.clone();
                        let name = wallpaper::shorten(&folder.unwrap_or_default());
                        if !folders.contains(&name) {
                            folders.push(name);
                            self.edit(|e| e.set_list("desktop", "folders", &folders));
                        }
                        self.wallpaper_draft.clear();
                        return self.scan_library();
                    }
                    wallpaper::Source::File(file) if !file.exists() => {
                        self.error = Some(format!("{} does not exist", file.display()));
                    }
                    _ => {
                        self.wallpaper_draft.clear();
                        return self.update(if self.page == Page::Modes {
                            Message::ModeWallpaper(Some(path))
                        } else {
                            Message::DesktopWallpaper(path)
                        });
                    }
                }
            }
            Message::Notifications(index, n) => self.edit(|e| e.set_mode_notifications(index, n)),
            Message::Performance(on) => self.edit(|e| e.set_mode(index, "performance", on)),
            Message::KeepAwake(on) => self.edit(|e| e.set_mode(index, "keep_awake", on)),
            Message::WhenGame(on) => self.set_trigger(index, Trigger::Game, on),
            Message::WhenBattery(on) => self.set_trigger(index, Trigger::Battery, on),
            Message::HoursDraft(draft) => self.hours_draft = draft,
            Message::MonitorDraft(draft) => self.monitor_draft = draft,
            Message::SetHours => match Trigger::parse(self.hours_draft.trim()) {
                Some(hours @ Trigger::Hours { .. }) => {
                    self.set_trigger(index, hours, true);
                    self.hours_draft.clear();
                }
                _ => self.error = Some("Hours look like 20:00-07:00".into()),
            },
            Message::SetMonitor => {
                let name = self.monitor_draft.trim().to_owned();
                if !name.is_empty() {
                    self.set_trigger(index, Trigger::Monitor(name), true);
                    self.monitor_draft.clear();
                }
            }
            Message::DropTrigger(at) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut when = mode.when.clone();
                    if at < when.len() {
                        when.remove(at);
                        self.write_triggers(index, &when);
                    }
                }
            }
            Message::StartHere(on) => {
                let name = self.config.modes.get(index).map(|m| m.name.clone());
                self.edit(|e| {
                    e.set(
                        "general",
                        "mode",
                        if on {
                            name.unwrap_or_default()
                        } else {
                            String::new()
                        },
                    )
                });
            }
            Message::KeyCommand(command) => self.key_command = command,
            Message::RecordModeKey => {
                self.recording_mode_key = true;
                self.recording = None;
            }
            Message::AddModeKey => {
                let command = self.key_command.trim().to_owned();
                if let (Some(combo), false) = (self.key_combo.clone(), command.is_empty())
                    && let Some(mode) = self.config.modes.get(index)
                {
                    let mut keys: Vec<(String, String)> = mode
                        .keys
                        .iter()
                        .filter(|(c, _)| !keys::same(c, &combo))
                        .cloned()
                        .collect();
                    keys.push((combo, command));
                    self.edit(|e| e.set_mode_keys(index, &keys));
                    self.key_combo = None;
                    self.key_command.clear();
                }
            }
            Message::RemoveModeKey(k) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut keys = mode.keys.clone();
                    if k < keys.len() {
                        keys.remove(k);
                        self.edit(|e| e.set_mode_keys(index, &keys));
                    }
                }
            }
            Message::Draft(draft) => self.draft = draft,
            Message::Add => {
                let command = self.draft.trim().to_owned();
                if !command.is_empty()
                    && let Some(mode) = self.config.modes.get(index)
                {
                    let mut open = mode.open.clone();
                    open.push(command);
                    self.edit(|e| e.set_mode_list(index, "open", &open));
                    self.draft.clear();
                }
            }
            Message::Remove(app) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut open = mode.open.clone();
                    if app < open.len() {
                        open.remove(app);
                        self.edit(|e| e.set_mode_list(index, "open", &open));
                    }
                }
            }
            Message::Gaps(v) => self.stage(|e| e.set("layout", "gaps", i64::from(v))),
            Message::Radius(v) => self.stage(|e| e.set("layout", "radius", i64::from(v))),
            Message::BorderStyle(style) => self.edit(|e| {
                e.set("layout", "borders", style > 0);
                if style == 2 {
                    e.set("layout", "gradient", true);
                } else {
                    e.unset("layout", "gradient");
                }
            }),
            Message::Shadows(on) => self.edit(|e| e.set("layout", "shadows", on)),
            Message::Workspaces(style) => {
                let value = match style {
                    Workspaces::Dots => "dots",
                    Workspaces::Numbers => "numbers",
                };
                self.edit(|e| e.set("bar", "workspaces", value));
            }
            Message::Seconds(on) => self.edit(|e| e.set("bar", "seconds", on)),
            Message::BarEdge(edge) => self.edit(|e| {
                e.set(
                    "bar",
                    "position",
                    if edge == Edge::Top { "top" } else { "bottom" },
                )
            }),
            Message::BarFloating(on) => self.edit(|e| e.set("bar", "floating", on)),
            Message::BarOpacity(v) => self.stage(|e| e.set("bar", "opacity", f64::from(v) / 100.0)),
            Message::BarBlur(on) => self.edit(|e| {
                if on {
                    e.set("bar", "blur", true);
                } else {
                    e.unset("bar", "blur");
                }
            }),
            Message::Date(on) => self.edit(|e| e.set("bar", "date", on)),
            Message::BatteryPercent(on) => self.edit(|e| e.set("bar", "battery_percent", on)),
            Message::DoNotDisturb(on) => {
                self.edit(|e| e.set("notifications", "do_not_disturb", on))
            }
            Message::PopupSeconds(v) => {
                self.stage(|e| e.set("notifications", "seconds", i64::from(v)))
            }
            Message::Test => {
                self.tested = None;
                return Task::perform(
                    notify::send(
                        "This is how a popup looks".into(),
                        format!("It stays for {} seconds", self.config.alerts.seconds),
                    ),
                    Message::Tested,
                );
            }
            Message::Tested(result) => self.tested = Some(result),
            Message::AddLayout(layout) => {
                let mut layouts = self.config.keyboard.layouts.clone();
                if !layouts.iter().any(|l| l == layout.0) {
                    layouts.push(layout.0.to_owned());
                    self.edit(|e| e.set_list("keyboard", "layouts", &layouts));
                }
            }
            Message::RemoveLayout(index) => {
                let mut layouts = self.config.keyboard.layouts.clone();
                if index < layouts.len() {
                    layouts.remove(index);
                    self.edit(|e| e.set_list("keyboard", "layouts", &layouts));
                }
            }
            Message::Switch(switch) => self.edit(|e| e.set("keyboard", "switch", switch.key())),
            Message::RepeatDelay(v) => {
                self.stage(|e| e.set("keyboard", "repeat_delay", i64::from(v)))
            }
            Message::RepeatRate(v) => {
                self.stage(|e| e.set("keyboard", "repeat_rate", i64::from(v)))
            }
            Message::Typing(text) => self.typing = text,
            Message::Tap(on) => self.edit(|e| e.set("touchpad", "tap", on)),
            Message::TouchNatural(on) => self.edit(|e| e.set("touchpad", "natural_scroll", on)),
            Message::TouchTyping(on) => {
                self.edit(|e| e.set("touchpad", "disable_while_typing", on))
            }
            Message::TouchSpeed(v) => self.stage(|e| e.set_speed("touchpad", v)),
            Message::MouseNatural(on) => self.edit(|e| e.set("mouse", "natural_scroll", on)),
            Message::MouseAcceleration(on) => self.edit(|e| e.set("mouse", "acceleration", on)),
            Message::MouseSpeed(v) => self.stage(|e| e.set_speed("mouse", v)),
            Message::Record(shortcut) => {
                self.recording = shortcut;
                self.shortcut_error = None;
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. })
                if self.recording_mode_key =>
            {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.recording_mode_key = false;
                } else if let Some(combo) = keys::combo(&key, modifiers) {
                    self.key_combo = Some(combo);
                    self.recording_mode_key = false;
                }
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(shortcut) = self.recording {
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        self.recording = None;
                    } else if let Some(combo) = keys::combo(&key, modifiers) {
                        self.record(shortcut, combo);
                    }
                } else if modifiers.command() && key.as_ref() == keyboard::Key::Character("z") {
                    self.step(!modifiers.shift());
                }
            }
            Message::Undo => self.step(true),
            Message::Redo => self.step(false),
            Message::ResetShortcut(shortcut) => {
                let combo = apolline_config::Shortcuts::default()
                    .get(shortcut)
                    .to_owned();
                self.record(shortcut, combo);
            }
            Message::LockBeforeSleep(on) => self.edit(|e| e.set("lock", "before_sleep", on)),
            Message::LockMedia(on) => self.edit(|e| e.set("lock", "show_media", on)),
            Message::IdleLock(v) => self.stage(|e| e.set("idle", "lock_after", i64::from(v))),
            Message::IdleScreen(v) => {
                self.stage(|e| e.set("idle", "screen_off_after", i64::from(v)))
            }
            Message::IdleSuspend(v) => self.stage(|e| e.set("idle", "suspend_after", i64::from(v))),
            Message::LockNow => {
                let line = format!("{} lock &", crate::control::me());
                return Task::future(async move {
                    let _ = tokio::process::Command::new("sh")
                        .args(["-c", &line])
                        .status()
                        .await;
                })
                .discard();
            }
            Message::WinWidth(width) => self.edit(|e| e.set("windows", "width", width.key())),
            Message::WinPreset(width) => {
                let mut presets = self
                    .config
                    .windows
                    .presets
                    .clone()
                    .unwrap_or_else(|| vec![Width::Third, Width::Half, Width::TwoThirds]);
                if presets.contains(&width) {
                    presets.retain(|w| *w != width);
                } else {
                    presets.push(width);
                    presets.sort_by(|a, b| a.proportion().total_cmp(&b.proportion()));
                }
                if !presets.is_empty() {
                    let keys: Vec<String> = presets.iter().map(|w| w.key().to_owned()).collect();
                    self.edit(|e| e.set_list("windows", "presets", &keys));
                }
            }
            Message::WinCenter(center) => self.edit(|e| e.set("windows", "center", center.key())),
            Message::CenterLone(on) => self.edit(|e| e.set("windows", "center_lone", on)),
            Message::RecentWindows(on) => self.edit(|e| e.set("windows", "switcher", on)),
            Message::FocusFollows(on) => self.edit(|e| e.set("windows", "focus_follows_mouse", on)),
            Message::WarpMouse(on) => self.edit(|e| e.set("windows", "warp_mouse", on)),
            Message::Opacity(v) => {
                self.stage(|e| e.set("windows", "opacity", f64::from(v) / 100.0))
            }
            Message::InactiveOpacity(v) => {
                self.stage(|e| e.set("windows", "inactive_opacity", f64::from(v) / 100.0))
            }
            Message::WinMotion(motion) => self.edit(|e| match motion {
                Some(motion) => e.set("windows", "motion", motion.key()),
                None => e.unset("windows", "motion"),
            }),
            Message::WinAppear(appear) => self.edit(|e| match appear {
                Some(appear) => e.set("windows", "appear", appear.key()),
                None => e.unset("windows", "appear"),
            }),
            Message::WinBlur(blur) => self.edit(|e| match blur {
                Some(blur) => e.set("windows", "blur", blur.key()),
                None => e.unset("windows", "blur"),
            }),
            Message::Animations(choice) => match choice {
                2 => self.edit(|e| e.unset("windows", "animations")),
                _ => {
                    let speed = [0.0, 2.0, 1.0, 0.5][usize::from(choice.min(3))];
                    self.edit(|e| e.set("windows", "animations", speed));
                }
            },
            Message::RuleApp(app) => self.rule.app = app,
            Message::RuleFloating(on) => self.rule.floating = Some(on),
            Message::RuleWidth(width) => {
                self.rule.width = (self.rule.width != Some(width)).then_some(width)
            }
            Message::AddRule => {
                let app = self.rule.app.trim().to_owned();
                if !app.is_empty() {
                    let rule = Rule {
                        app,
                        ..self.rule.clone()
                    };
                    self.edit(|e| e.add_rule(&rule));
                    self.rule.app.clear();
                }
            }
            Message::RemoveRule(index) => self.edit(|e| e.remove_rule(index)),
            Message::ApplyTheme(index) => {
                if let Some((theme, _)) = self.themes.get(index).cloned() {
                    self.edit(|e| theme.apply(e));
                    self.theme_note = None;
                    return self.custom_thumb();
                }
            }
            Message::ThemeName(name) => self.theme_name = name,
            Message::SaveTheme => {
                let name = self.theme_name.trim().to_owned();
                if !name.is_empty() {
                    let theme = Look::of(&self.config, &name, std::env::var("USER").ok());
                    self.theme_note = Some(themes::save(&theme).map(|path| {
                        format!(
                            "Saved as {}. Send that file to share the look.",
                            path.display()
                        )
                    }));
                    if self.theme_note.as_ref().is_some_and(Result::is_ok) {
                        self.edit(|e| e.set("desktop", "theme", name.as_str()));
                        self.theme_name.clear();
                    }
                    self.themes = themes::all();
                }
            }
            Message::ThemePath(path) => self.theme_path = path,
            Message::ImportTheme => {
                let path = self.theme_path.trim();
                if !path.is_empty() {
                    let path = match path.strip_prefix("~/") {
                        Some(rest) => std::env::var_os("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_default()
                            .join(rest),
                        None => PathBuf::from(path),
                    };
                    self.theme_note = Some(
                        themes::import(&path)
                            .map(|theme| format!("“{}” is in the gallery now", theme.name)),
                    );
                    self.theme_path.clear();
                    self.themes = themes::all();
                    return self.custom_thumb();
                }
            }
            Message::OpenThemes => {
                let _ = std::fs::create_dir_all(themes::dir());
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let dir = themes::dir();
                return Task::future(async move {
                    let _ = tokio::process::Command::new(opener).arg(dir).status().await;
                })
                .discard();
            }
            Message::Setup => {
                let look = Fragment {
                    config: &self.config,
                    mode: self.config.start(),
                    outputs: &[],
                };
                self.setup_log =
                    match fragment::setup(&fragment::path(), &fragment::niri_config(), &look) {
                        Ok(done) => done,
                        Err(err) => vec![format!("could not set up: {err}")],
                    };
            }
            Message::Finish => return iced::exit(),
            Message::OpenFile => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let path = self.path.clone();
                return Task::future(async move {
                    let _ = tokio::process::Command::new(opener)
                        .arg(path)
                        .status()
                        .await;
                })
                .discard();
            }
            Message::Commit => self.save(),
            Message::Thumbs(thumbs) => {
                for (name, handle) in thumbs {
                    self.thumbs.retain(|(n, _)| *n != name);
                    self.thumbs.push((name, handle));
                }
            }
            Message::Thumb(name, Some(handle)) => {
                self.thumbs.retain(|(n, _)| *n != name);
                self.thumbs.push((name, handle));
                return self.next_thumb();
            }
            Message::Thumb(_, None) => return self.next_thumb(),
            Message::Tick => {
                self.live = crate::control::shared();
                if stamp(&self.path) != self.stamp {
                    self.reload();
                }
                if self.screenshot.is_some()
                    && !self.shooting
                    && self.queue.is_empty()
                    && wallpaper::bundled()
                        .iter()
                        .all(|(name, _)| self.thumb(name).is_some())
                {
                    self.shooting = true;
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Shot);
                }
            }
            Message::Shot(capture) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = shot::save(&path, &capture)
                {
                    eprintln!("apolline: could not save {}: {err}", path.display());
                }
                return iced::exit();
            }
            Message::Key(_) | Message::Noop => {}
        }
        Task::none()
    }

    fn record(&mut self, shortcut: Shortcut, combo: String) {
        let taken = Shortcut::ALL.into_iter().find(|&other| {
            other != shortcut && keys::same(self.config.shortcuts.get(other), &combo)
        });
        match taken {
            Some(other) => {
                self.shortcut_error = Some(format!(
                    "{combo} already opens “{}”",
                    fragment::action(other).0
                ));
            }
            None => {
                self.edit(|e| e.set("shortcuts", shortcut.key(), combo.as_str()));
                self.recording = None;
                self.shortcut_error = None;
            }
        }
    }

    fn custom_thumb(&self) -> Task<Message> {
        let mut names = vec![self.config.desktop.wallpaper.clone()];
        names.extend(self.themes.iter().map(|(theme, _)| theme.wallpaper.clone()));
        names.sort();
        names.dedup();
        if !self.config.modes.is_empty() {
            names.push(self.config.wallpaper(Some(self.selected)).to_owned());
        }
        Task::batch(
            names
                .into_iter()
                .filter(|name| self.thumb(name).is_none() && !is_bundled(name))
                .map(|name| {
                    Task::perform(wallpaper::thumbnail(name.clone()), move |thumb| {
                        Message::Thumb(name.clone(), thumb)
                    })
                }),
        )
    }

    fn scan_library(&mut self) -> Task<Message> {
        self.library = wallpaper::library(&self.config.desktop.folders);
        self.queue = self
            .library
            .iter()
            .rev()
            .filter(|name| self.thumb(name).is_none())
            .cloned()
            .collect();
        Task::batch([self.next_thumb(), self.next_thumb()])
    }

    fn next_thumb(&mut self) -> Task<Message> {
        let Some(name) = self.queue.pop() else {
            return Task::none();
        };
        Task::perform(wallpaper::thumbnail(name.clone()), move |thumb| {
            Message::Thumb(name.clone(), thumb)
        })
    }

    fn thumb(&self, name: &str) -> Option<&image::Handle> {
        self.thumbs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, handle)| handle)
    }

    fn stage(&mut self, change: impl FnOnce(&mut Editor)) {
        let Ok(editor) = &mut self.editor else {
            return;
        };
        change(editor);
        let (config, diagnostics) = Config::parse(&editor.text());
        self.config = config;
        self.diagnostics = diagnostics;
    }

    fn edit(&mut self, change: impl FnOnce(&mut Editor)) {
        self.stage(change);
        self.save();
    }

    fn save(&mut self) {
        let Ok(editor) = &self.editor else {
            return;
        };
        match editor.save(&self.path) {
            Ok(()) => {
                self.stamp = stamp(&self.path);
                self.error = None;
                self.remember(editor.text());
            }
            Err(err) => self.error = Some(format!("could not save: {err}")),
        }
    }

    fn remember(&mut self, text: String) {
        if text != self.saved {
            self.undo.push(std::mem::replace(&mut self.saved, text));
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    fn step(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(text) = from.pop() else {
            return;
        };
        to.push(std::mem::replace(&mut self.saved, text.clone()));
        self.editor = Editor::parse(&text);
        let (config, diagnostics) = Config::parse(&text);
        self.config = config;
        self.diagnostics = diagnostics;
        self.theme_note = None;
        match std::fs::write(&self.path, &text) {
            Ok(()) => self.stamp = stamp(&self.path),
            Err(err) => self.error = Some(format!("could not save: {err}")),
        }
    }

    fn reload(&mut self) {
        self.stamp = stamp(&self.path);
        self.editor = Editor::load(&self.path);
        if let Ok(editor) = &self.editor {
            self.remember(editor.text());
        }
        let (config, diagnostics) = Config::load(&self.path);
        if self.editor.is_ok() {
            self.config = config;
        }
        self.diagnostics = diagnostics;
        self.niri_binds = read_niri_binds();
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            keyboard::listen().map(Message::Key),
        ])
    }

    fn palette(&self) -> Palette {
        if self.page == Page::Modes && self.selected < self.config.modes.len() {
            self.config.palette(Some(self.selected))
        } else {
            self.config.palette(None)
        }
    }

    fn view(&self) -> Element<'_, Message> {
        responsive(move |size| {
            let wide = size.width >= WIDE;
            let compact = size.width < NARROW;
            let content = match (&self.editor, self.page) {
                (Err(err), page) if page != Page::About => broken(err),
                (_, page) => self.page_view(page, wide),
            };
            let padding = if wide { [28, 36] } else { [22, 24] };
            container(row![
                self.sidebar(compact),
                container(content).padding(padding).width(Fill)
            ])
            .width(Fill)
            .height(Fill)
            .style(|_| container::background(style::color(WINDOW)))
            .into()
        })
        .into()
    }

    fn page_view(&self, page: Page, wide: bool) -> Element<'_, Message> {
        let form: Element<'_, Message> = match page {
            Page::Modes => self.modes(),
            Page::Themes => self.themes(),
            Page::Appearance => self.appearance(),
            Page::Windows => self.windows(),
            Page::Bar => self.bar(),
            Page::Lock => self.lock(),
            Page::Notifications => self.notifications(),
            Page::Keyboard => self.keyboard(),
            Page::Pointer => self.pointer(),
            Page::Shortcuts => self.shortcuts(),
            Page::About => self.about(),
            Page::Welcome => self.welcome(),
        };
        let writes = match (&self.editor, page) {
            (_, Page::About | Page::Welcome) => None,
            (Ok(editor), Page::Modes) if self.selected < self.config.modes.len() => {
                Some(editor.mode_section(self.selected))
            }
            (Ok(_), Page::Modes) => Some(String::new()),
            (Ok(editor), Page::Windows) => Some(format!(
                "{}\n{}",
                editor.sections(page.sections()),
                editor.lists("rule")
            )),
            (Ok(editor), page) => Some(editor.sections(page.sections())),
            (Err(_), _) => None,
        };
        let mut header = row![
            column![
                text(page.title()).size(26).font(style::semibold()),
                text(page.subtitle())
                    .size(14)
                    .color(style::color(style::FAINT)),
            ]
            .spacing(4)
            .width(Fill)
        ]
        .align_y(Alignment::Start);
        if self.editor.is_ok() && !matches!(page, Page::About | Page::Welcome) {
            let history = |glyph, label, enabled: bool, message| {
                let tint = if enabled { neutral::TEXT } else { style::FAINT };
                button(
                    row![
                        icon(glyph, 14.0, tint),
                        text(label).size(13).color(style::color(tint))
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([7, 12])
                .style(style::ghost(8.0))
                .on_press_maybe(enabled.then_some(message))
            };
            header = header.push(
                row![
                    history(Icon::Undo, "Undo", !self.undo.is_empty(), Message::Undo),
                    history(Icon::Redo, "Redo", !self.redo.is_empty(), Message::Redo),
                ]
                .spacing(4),
            );
        }
        let body: Element<'_, Message> = match writes {
            Some(code) if wide => row![
                scrollable(form)
                    .direction(style::thin())
                    .style(style::scroller)
                    .height(Fill)
                    .width(Fill),
                self.writes(code),
            ]
            .spacing(16)
            .into(),
            Some(code) => {
                scrollable(container(column![form, self.writes(code)].spacing(22)).padding([0, 16]))
                    .height(Fill)
                    .into()
            }
            None => scrollable(container(form).padding([0, 16]))
                .height(Fill)
                .into(),
        };
        column![header, body].spacing(22).into()
    }

    fn sidebar(&self, compact: bool) -> Element<'_, Message> {
        let palette = self.palette();
        let mut brand = row![
            container(icon(Icon::Sparkles, 16.0, palette.on_accent))
                .center(32)
                .style(style::pill(palette.accent)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if !compact {
            brand = brand.push(column![
                text("Apolline").size(15).font(style::semibold()),
                text("Settings").size(12).color(style::color(style::FAINT)),
            ]);
        }

        let nav = Column::with_children(Page::ALL.map(|page| {
            let on = self.page == page;
            let tint = if on { palette.accent } else { style::SOFT_TEXT };
            let mut label = row![icon(page.icon(), 16.0, tint)]
                .spacing(12)
                .align_y(Alignment::Center);
            if !compact {
                label = label.push(text(page.title()).size(14).font(if on {
                    style::medium()
                } else {
                    Font::with_name(font::UI)
                }));
            }
            button(label)
                .width(Fill)
                .padding([9, 12])
                .style(move |theme, status| {
                    let mut style = style::ghost(10.0)(theme, status);
                    if on {
                        style.background = Some(Background::Color(style::color(style::FILL)));
                    }
                    style.text_color =
                        style::color(if on { neutral::TEXT } else { style::SOFT_TEXT });
                    style
                })
                .on_press(Message::Page(page))
                .into()
        }))
        .spacing(2);

        let problems = self.diagnostics.len();
        let health = if compact {
            row![if problems == 0 {
                icon(Icon::Done, 14.0, style::GOOD)
            } else {
                icon(Icon::Warning, 14.0, style::WARN)
            }]
        } else if problems == 0 {
            row![
                icon(Icon::Done, 14.0, style::GOOD),
                text("Config is clean")
                    .size(12)
                    .color(style::color(style::FAINT)),
            ]
        } else {
            row![
                icon(Icon::Warning, 14.0, style::WARN),
                text(match problems {
                    1 => "1 problem in the config".to_owned(),
                    n => format!("{n} problems in the config"),
                })
                .size(12)
                .color(style::color(style::WARN)),
            ]
        };
        let footer = button(health.spacing(8).align_y(Alignment::Center))
            .padding([8, 12])
            .width(Fill)
            .style(style::ghost(10.0))
            .on_press(Message::Page(Page::About));

        container(column![brand, space().height(18), nav, space::vertical(), footer].spacing(2))
            .width(if compact { 68 } else { 240 })
            .height(Fill)
            .padding([20, 14])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(SIDEBAR))),
                border: Border {
                    color: style::color(neutral::HAIRLINE),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    fn modes(&self) -> Element<'_, Message> {
        if self.config.modes.is_empty() || self.adding {
            return self.templates();
        }
        let index = self.selected.min(self.config.modes.len() - 1);
        let mode = &self.config.modes[index];
        let palette = self.config.palette(Some(index));

        let mut tabs = Row::new().spacing(6).align_y(Alignment::Center);
        for (i, other) in self.config.modes.iter().enumerate() {
            let on = i == index;
            let tint = if on {
                self.config.palette(Some(i)).accent
            } else {
                style::FAINT
            };
            tabs = tabs.push(
                button(
                    row![
                        icon(Icon::named(&other.icon), 14.0, tint),
                        text(other.name.as_str()).size(13)
                    ]
                    .spacing(7)
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .style(style::choice(on))
                .on_press(Message::Select(i)),
            );
        }
        tabs = tabs.push(
            button(
                row![
                    icon(Icon::Plus, 14.0, style::SOFT_TEXT),
                    text("Add").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([7, 12])
            .style(style::ghost(8.0))
            .on_press(Message::Adding(true)),
        );

        let preview = self.preview(self.config.wallpaper(Some(index)), 170.0, {
            let chip = container(
                row![
                    icon(Icon::named(&mode.icon), 12.0, palette.accent),
                    text(mode.name.as_str())
                        .size(11)
                        .color(style::color(palette.accent))
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([3, 9])
            .style(style::pill(palette.accent_soft));
            let mini_bar = container(
                row![chip, space::horizontal(), text("14:32").size(11)].align_y(Alignment::Center),
            )
            .padding([5, 6])
            .width(Fill)
            .style(style::bar);
            let window =
                container(space())
                    .width(Fill)
                    .height(Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(style::color(
                            Rgba::hex(0x141619).alpha(0.92),
                        ))),
                        border: Border {
                            color: style::color(palette.accent),
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..container::Style::default()
                    });
            column![mini_bar, row![window, space().width(Fill)].spacing(8)]
                .spacing(8)
                .padding(10)
                .into()
        });

        let name = text_input("Name", self.name_draft.as_deref().unwrap_or(&mode.name))
            .on_input(Message::ModeName)
            .on_submit(Message::ModeNameDone)
            .padding([8, 12])
            .size(14)
            .width(260)
            .style(style::field(palette));
        let rename = button(text("Rename").size(13))
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(self.name_draft.is_some().then_some(Message::ModeNameDone));
        let icons = Row::with_children(MODE_ICONS.map(|(key, glyph)| {
            let on = mode.icon == key;
            button(
                container(icon(
                    glyph,
                    17.0,
                    if on {
                        palette.on_accent
                    } else {
                        style::SOFT_TEXT
                    },
                ))
                .center(34),
            )
            .padding(0)
            .style(move |theme, status| {
                let mut style = style::tile(on, palette)(theme, status);
                style.border.radius = 9.0.into();
                style
            })
            .on_press(Message::ModeIcon(key))
            .into()
        }))
        .spacing(6)
        .wrap()
        .vertical_spacing(6);

        let mut apps = Column::new().spacing(8);
        for (app, command) in mode.open.iter().enumerate() {
            apps = apps.push(
                row![
                    icon(Icon::AppWindow, 15.0, style::FAINT),
                    text(command.as_str()).font(style::mono()).size(13),
                    space::horizontal(),
                    button(icon(Icon::X, 14.0, style::FAINT))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::Remove(app)),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        if mode.open.is_empty() {
            apps = apps.push(
                text("Nothing opens yet")
                    .size(13)
                    .color(style::color(style::FAINT)),
            );
        }
        apps = apps.push(
            row![
                text_input("firefox --new-window", &self.draft)
                    .on_input(Message::Draft)
                    .on_submit(Message::Add)
                    .padding([8, 12])
                    .size(13)
                    .font(style::mono())
                    .style(style::field(palette)),
                button(
                    row![
                        icon(Icon::Plus, 14.0, palette.on_accent),
                        text("Add").size(13)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                )
                .padding([8, 14])
                .style(style::primary(palette))
                .on_press(Message::Add),
            ]
            .spacing(8),
        );

        let mut keys = vec![];
        for (k, (combo, command)) in mode.keys.iter().enumerate() {
            let taken = Shortcut::ALL
                .into_iter()
                .find(|&s| keys::same(self.config.shortcuts.get(s), combo));
            let replaced = self
                .niri_binds
                .iter()
                .find(|(bind, _)| keys::same(bind, combo));
            let about = match (taken, replaced) {
                (Some(shortcut), _) => Some(format!(
                    "Taken by “{}”, so it won't work; pick other keys",
                    fragment::action(shortcut).0
                )),
                (None, Some((_, action))) => {
                    Some(format!("Replaces {action} while this mode is on"))
                }
                _ => None,
            };
            let mut words = column![text(command.as_str()).size(13).font(style::mono())].spacing(3);
            if let Some(about) = about {
                let tint = if taken.is_some() {
                    style::WARN
                } else {
                    style::FAINT
                };
                words = words.push(text(about).size(12).color(style::color(tint)));
            }
            keys.push(
                row![
                    container(keycaps(combo)).width(Length::Fixed(200.0)),
                    words.width(Fill),
                    button(icon(Icon::X, 14.0, style::FAINT))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveModeKey(k)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 16])
                .into(),
            );
        }
        let capture: Element<'_, Message> = match (&self.key_combo, self.recording_mode_key) {
            (_, true) => container(
                text("Press the keys…")
                    .size(13)
                    .color(style::color(palette.accent)),
            )
            .padding([6, 12])
            .style(style::pill(palette.accent_soft))
            .into(),
            (Some(combo), false) => button(keycaps(combo))
                .padding(2)
                .style(style::ghost(8.0))
                .on_press(Message::RecordModeKey)
                .into(),
            (None, false) => button(text("Choose keys").size(13))
                .padding([7, 14])
                .style(style::secondary)
                .on_press(Message::RecordModeKey)
                .into(),
        };
        keys.push(
            row![
                container(capture).width(Length::Fixed(200.0)),
                text_input(
                    "command, like  steam  or  niri msg action fullscreen-window",
                    &self.key_command
                )
                .on_input(Message::KeyCommand)
                .on_submit(Message::AddModeKey)
                .padding([8, 12])
                .size(13)
                .font(style::mono())
                .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::primary(palette))
                    .on_press_maybe(
                        (self.key_combo.is_some() && !self.key_command.trim().is_empty())
                            .then_some(Message::AddModeKey)
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([12, 16])
            .into(),
        );

        let starts_here = self.config.start() == Some(index);
        let last = self.config.modes.len() - 1;
        let manage = row![
            button(
                row![
                    icon(Icon::ArrowUp, 14.0, style::SOFT_TEXT),
                    text("Earlier").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .padding([7, 12])
            .style(style::secondary)
            .on_press_maybe((index > 0).then_some(Message::MoveMode(-1))),
            button(
                row![
                    icon(Icon::ArrowDown, 14.0, style::SOFT_TEXT),
                    text("Later").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .padding([7, 12])
            .style(style::secondary)
            .on_press_maybe((index < last).then_some(Message::MoveMode(1))),
            space::horizontal(),
            button(
                text(if self.armed {
                    "Delete for good?"
                } else {
                    "Delete mode"
                })
                .size(13)
            )
            .padding([7, 14])
            .style(move |theme, status| {
                let mut style = style::danger(self.armed)(theme, status);
                style.border.radius = 10.0.into();
                style
            })
            .on_press(Message::RemoveMode),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        column![
            tabs.wrap().vertical_spacing(6),
            preview,
            self.why_on(mode, palette),
            group(
                "Name and icon",
                vec![
                    padded(row![name, rename].spacing(8).align_y(Alignment::Center)),
                    padded(icons),
                ]
            ),
            group(
                "Look",
                vec![
                    padded(self.wallpaper_picker(
                        mode.wallpaper.as_deref(),
                        true,
                        mode_wallpaper,
                        palette
                    )),
                    padded(accent_picker(
                        mode.accent,
                        auto_accent(self.config.wallpaper(Some(self.selected))),
                        true,
                        mode_accent,
                        palette
                    )),
                ]
            ),
            group(
                "Notifications",
                vec![setting(
                    "Popups",
                    "Held ones wait behind the bell on the bar",
                    rules(index, mode.notifications),
                )]
            ),
            group("Open when this mode starts", vec![padded(apps)]),
            group(
                "Behaviour",
                vec![
                    setting(
                        "Performance",
                        "Power profile to performance, games drive the refresh rate, effects off",
                        switch(mode.performance, Message::Performance, palette),
                    ),
                    setting(
                        "Keep the screen awake",
                        "No idle lock, no screen off, no sleep while this mode is on",
                        switch(mode.keep_awake, Message::KeepAwake, palette),
                    ),
                    setting(
                        "Start in this mode",
                        "Apolline switches to it when you log in",
                        switch(starts_here, Message::StartHere, palette),
                    ),
                ]
            ),
            group("Turns itself on", self.triggers(mode, palette)),
            group("Keys only in this mode", keys),
            padded(manage),
        ]
        .spacing(22)
        .into()
    }

    fn templates(&self) -> Element<'_, Message> {
        let card = |name: &'static str, template: ModeSettings, about: &'static str| {
            let accent = match template.accent {
                Some(Accent::Fixed(color)) => color,
                _ => auto_accent(
                    template
                        .wallpaper
                        .as_deref()
                        .unwrap_or(&self.config.desktop.wallpaper),
                ),
            };
            let palette = Palette::new(accent);
            button(
                column![
                    container(icon(Icon::named(&template.icon), 20.0, palette.accent))
                        .center(40)
                        .style(style::pill(palette.accent_soft)),
                    text(if name.is_empty() { "Empty" } else { name })
                        .size(15)
                        .font(style::semibold()),
                    text(about).size(12).color(style::color(style::FAINT)),
                ]
                .spacing(8),
            )
            .width(Fill)
            .padding(16)
            .style(|theme, status| {
                let mut style = style::secondary(theme, status);
                style.border = Border {
                    color: style::color(neutral::HAIRLINE),
                    width: 1.0,
                    radius: style::RADIUS_TILE.into(),
                };
                style
            })
            .on_press(Message::AddMode(name))
        };
        let about = |name: &str| match name {
            "Work" => "Calm wallpaper, low-priority popups wait",
            "Play" => "Performance, no popups, the screen stays awake",
            "Evening" => "Night wallpaper, warm accent, everything gets through",
            "Focus" => "Only urgent popups, a violet accent",
            "Presentation" => "No popups and the screen never sleeps",
            _ => "",
        };
        let mut cards: Vec<Element<'_, Message>> = TEMPLATES
            .map(|name| card(name, ModeSettings::template(name), about(name)).into())
            .into();
        cards.push(card("", ModeSettings::blank(""), "Start from nothing").into());
        let mut grid = Column::new().spacing(12);
        let mut cards = cards.into_iter();
        loop {
            let chunk: Vec<Element<'_, Message>> = cards.by_ref().take(3).collect();
            if chunk.is_empty() {
                break;
            }
            grid = grid.push(Row::with_children(chunk).spacing(12));
        }
        let intro = if self.config.modes.is_empty() {
            "A mode changes the wallpaper, accent, notifications, apps, performance and keys in one go. You don't need any; when you want one, start here."
        } else {
            "Pick a starting point. Everything in it can be changed afterwards."
        };
        let mut page = column![
            text(intro).size(14).color(style::color(style::SOFT_TEXT)),
            grid
        ]
        .spacing(18);
        if !self.config.modes.is_empty() {
            page = page.push(
                button(text("Cancel").size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::Adding(false)),
            );
        }
        page.into()
    }

    fn wallpaper_picker<'a>(
        &'a self,
        current: Option<&str>,
        inherit: bool,
        on: fn(Option<String>) -> Message,
        palette: Palette,
    ) -> Element<'a, Message> {
        let ring = move |on: bool| {
            move |_: &Theme, _| button::Style {
                background: None,
                border: Border {
                    color: style::color(if on {
                        palette.accent
                    } else {
                        neutral::HAIRLINE
                    }),
                    width: if on { 2.0 } else { 1.0 },
                    radius: 12.0.into(),
                },
                ..button::Style::default()
            }
        };
        let mut tiles: Vec<Element<'a, Message>> = Vec::new();
        if inherit {
            let picture: Element<'a, Message> = match self.thumb(&self.config.desktop.wallpaper) {
                Some(handle) => stack![
                    image(handle.clone())
                        .width(Fill)
                        .height(88)
                        .content_fit(ContentFit::Cover)
                        .border_radius(9.0)
                        .opacity(0.45_f32),
                    container(text("Desktop's").size(12).font(style::medium())).center(Fill)
                ]
                .height(88)
                .into(),
                None => container(text("Desktop's").size(12))
                    .center_x(Fill)
                    .center_y(88)
                    .into(),
            };
            tiles.push(
                button(picture)
                    .width(Fill)
                    .padding(3)
                    .style(ring(current.is_none()))
                    .on_press(on(None))
                    .into(),
            );
        }
        let names: Vec<&str> = wallpaper::bundled()
            .iter()
            .map(|(name, _)| *name)
            .chain(self.library.iter().map(String::as_str))
            .collect();
        let limit = if self.all_walls { names.len() } else { WALLS };
        for name in names.iter().take(limit) {
            let picture: Element<'a, Message> = match self.thumb(name) {
                Some(handle) => container(
                    image(handle.clone())
                        .width(Fill)
                        .height(88)
                        .content_fit(ContentFit::Cover)
                        .border_radius(9.0),
                )
                .clip(true)
                .into(),
                None => container(space())
                    .width(Fill)
                    .height(88)
                    .style(|_| container::background(style::color(style::SUNKEN)))
                    .into(),
            };
            tiles.push(
                button(picture)
                    .width(Fill)
                    .padding(3)
                    .style(ring(current == Some(*name)))
                    .on_press(on(Some((*name).to_owned())))
                    .into(),
            );
        }
        if names.len() > WALLS {
            let label = if self.all_walls {
                "Fewer".to_owned()
            } else {
                format!("+{} more", names.len() - WALLS)
            };
            tiles.push(
                button(container(text(label).size(13)).center_x(Fill).center_y(88))
                    .width(Fill)
                    .padding(3)
                    .style(ring(false))
                    .on_press(Message::AllWallpapers)
                    .into(),
            );
        }
        let mut grid = Column::new().spacing(12);
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let mut line = Row::new().spacing(12);
            let mut count = 0;
            for tile in tiles.by_ref().take(PER_ROW) {
                line = line.push(tile);
                count += 1;
            }
            for _ in count..PER_ROW {
                line = line.push(Space::new().width(Fill));
            }
            grid = grid.push(line);
        }
        let walls = grid;
        let own = row![
            text_input("A picture or a folder of them", &self.wallpaper_draft)
                .on_input(Message::WallpaperDraft)
                .on_submit(Message::UseWallpaper)
                .padding([8, 12])
                .size(13)
                .style(style::field(palette)),
            button(text("Add").size(13))
                .padding([8, 16])
                .style(style::secondary)
                .on_press(Message::UseWallpaper),
        ]
        .spacing(8);
        let mut column = column![walls, own].spacing(14);
        if !self.config.desktop.folders.is_empty() {
            let folders = Row::with_children(self.config.desktop.folders.iter().enumerate().map(
                |(index, folder)| {
                    container(
                        row![
                            icon(Icon::Folder, 13.0, style::FAINT),
                            text(folder.as_str()).size(12),
                            button(icon(Icon::X, 12.0, style::FAINT))
                                .padding(2)
                                .style(style::ghost(6.0))
                                .on_press(Message::RemoveFolder(index)),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([4, 10])
                    .style(style::pill(Rgba::hex(0xffffff).alpha(0.06)))
                    .into()
                },
            ))
            .spacing(6)
            .wrap();
            column = column.push(folders);
        }
        if let Some(name) =
            current.filter(|name| !is_bundled(name) && !self.library.iter().any(|n| n == name))
        {
            column = column.push(
                row![
                    icon(Icon::Image, 14.0, style::FAINT),
                    text(format!("Using your file {name}"))
                        .size(12)
                        .color(style::color(style::FAINT)),
                ]
                .spacing(8),
            );
        }
        column.into()
    }

    fn theme_card<'a>(&'a self, index: usize, theme: &'a Look, mine: bool) -> Element<'a, Message> {
        let applied = self.config.desktop.theme.as_deref();
        let accent = match theme.accent {
            Accent::Fixed(color) => color,
            Accent::Auto => auto_accent(&theme.wallpaper),
        };
        let look = Palette::new(accent);
        let picture: Element<'a, Message> = match self.thumb(&theme.wallpaper) {
            Some(handle) => container(
                image(handle.clone())
                    .width(Fill)
                    .height(132)
                    .content_fit(ContentFit::Cover)
                    .border_radius(10.0),
            )
            .clip(true)
            .into(),
            None => container(space())
                .width(Fill)
                .height(132)
                .style(|_| container::background(style::color(neutral::INK)))
                .into(),
        };
        let bar = container(
            row![
                container(Space::new().width(14).height(4)).style(style::pill(look.accent)),
                container(Space::new().width(4).height(4))
                    .style(style::pill(neutral::TEXT.alpha(0.6))),
                space::horizontal(),
                container(Space::new().width(22).height(4))
                    .style(style::pill(neutral::TEXT.alpha(0.7))),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding([5, 8])
        .width(Fill)
        .style(move |_| {
            let mut style = style::glass(theme.bar.opacity, theme.bar.floating)(&Theme::Dark);
            if theme.bar.floating {
                style.border.radius = 7.0.into();
            }
            style
        });
        let inset = if theme.bar.floating { 6.0 } else { 0.0 };
        let window = container(space())
            .width(Length::FillPortion(3))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x15171b).alpha(theme.opacity.unwrap_or(1.0) * 0.95),
                ))),
                border: Border {
                    color: style::color(if theme.layout.borders {
                        look.accent
                    } else {
                        Rgba::hex(0).alpha(0.0)
                    }),
                    width: 1.0,
                    radius: (f32::from(theme.layout.radius) * 0.5).into(),
                },
                ..container::Style::default()
            });
        let other = container(space())
            .width(Length::FillPortion(2))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x15171b).alpha(theme.inactive_opacity.unwrap_or(1.0) * 0.9),
                ))),
                border: Border {
                    radius: (f32::from(theme.layout.radius) * 0.5).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        let gap = f32::from(theme.layout.gaps) * 0.5;
        let windows = row![window, other].spacing(gap).padding(gap.max(2.0));
        let screen = if theme.bar.position == Edge::Top {
            column![container(bar).padding(inset), windows]
        } else {
            column![windows, container(bar).padding(inset)]
        };
        let on = applied == Some(theme.name.as_str());
        let mut caption = row![
            container(Space::new().width(10).height(10)).style(style::pill(look.accent)),
            text(theme.name.as_str()).size(14).font(style::semibold()),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if let Some(author) = theme.author.as_ref().filter(|_| !on) {
            caption = caption.push(
                text(if mine {
                    "yours".to_owned()
                } else {
                    author.clone()
                })
                .size(12)
                .color(style::color(style::FAINT)),
            );
        }
        caption = caption.push(space::horizontal());
        if on {
            caption = caption.push(
                container(
                    text("Applied")
                        .size(11)
                        .font(style::semibold())
                        .color(style::color(look.on_accent)),
                )
                .padding([3, 9])
                .style(style::pill(look.accent)),
            );
        }
        button(
            column![
                stack![picture, container(screen).height(132)].height(132),
                caption
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding(8)
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(Background::Color(style::color(match status {
                button::Status::Hovered | button::Status::Pressed => style::FILL,
                _ => style::SUNKEN,
            }))),
            text_color: style::color(neutral::TEXT),
            border: Border {
                color: style::color(if on { look.accent } else { neutral::HAIRLINE }),
                width: if on { 2.0 } else { 1.0 },
                radius: style::RADIUS_TILE.into(),
            },
            ..button::Style::default()
        })
        .on_press(Message::ApplyTheme(index))
        .into()
    }

    fn themes(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let mut gallery = Column::new().spacing(14);
        let cards: Vec<Element<'_, Message>> = self
            .themes
            .iter()
            .enumerate()
            .map(|(index, (theme, file))| self.theme_card(index, theme, file.is_some()))
            .collect();
        let mut cards = cards.into_iter();
        loop {
            let chunk: Vec<Element<'_, Message>> = cards.by_ref().take(3).collect();
            if chunk.is_empty() {
                break;
            }
            let missing = 3 - chunk.len();
            let mut line = Row::with_children(chunk).spacing(14);
            for _ in 0..missing {
                line = line.push(Space::new().width(Fill));
            }
            gallery = gallery.push(line);
        }

        let note: Element<'_, Message> = match &self.theme_note {
            Some(Ok(line)) => text(line.as_str())
                .size(12)
                .color(style::color(style::GOOD))
                .into(),
            Some(Err(line)) => text(line.as_str())
                .size(12)
                .color(style::color(style::WARN))
                .into(),
            None => space().into(),
        };
        column![
            gallery,
            group(
                "Make your own",
                vec![
                    setting(
                        "Save my look",
                        "Wallpaper, accent, corners, gaps, borders, shadows, opacity, animations and the bar",
                        row![
                            text_input("Name it", &self.theme_name)
                                .on_input(Message::ThemeName)
                                .on_submit(Message::SaveTheme)
                                .padding([8, 12])
                                .size(13)
                                .width(180)
                                .style(style::field(palette)),
                            button(text("Save").size(13))
                                .padding([8, 14])
                                .style(style::primary(palette))
                                .on_press_maybe(
                                    (!self.theme_name.trim().is_empty()).then_some(Message::SaveTheme)
                                ),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                    setting(
                        "Add someone's theme",
                        "A .toml file like the ones in the themes folder",
                        row![
                            text_input("~/Downloads/look.toml", &self.theme_path)
                                .on_input(Message::ThemePath)
                                .on_submit(Message::ImportTheme)
                                .padding([8, 12])
                                .size(13)
                                .width(220)
                                .style(style::field(palette)),
                            button(
                                row![icon(Icon::Download, 14.0, neutral::TEXT), text("Add").size(13)]
                                    .spacing(6)
                                    .align_y(Alignment::Center)
                            )
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press_maybe(
                                (!self.theme_path.trim().is_empty()).then_some(Message::ImportTheme)
                            ),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                    setting(
                        "Themes folder",
                        "Share a look by sending its file",
                        button(
                            row![icon(Icon::Folder, 14.0, neutral::TEXT), text("Open").size(13)]
                                .spacing(6)
                                .align_y(Alignment::Center)
                        )
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::OpenThemes),
                    ),
                ]
                .into_iter()
                .chain(self.theme_note.is_some().then(|| padded(note)))
                .collect()
            ),
        ]
        .spacing(22)
        .into()
    }

    fn appearance(&self) -> Element<'_, Message> {
        let layout = self.config.layout;
        let palette = self.palette();
        let desktop = group(
            "Desktop",
            vec![
                padded(self.wallpaper_picker(
                    Some(self.config.desktop.wallpaper.as_str()),
                    false,
                    desktop_wallpaper,
                    palette,
                )),
                padded(accent_picker(
                    Some(self.config.desktop.accent),
                    auto_accent(&self.config.desktop.wallpaper),
                    false,
                    desktop_accent,
                    palette,
                )),
                self.color_apps(palette),
            ],
        );
        let preview = self.preview(&self.config.desktop.wallpaper, 200.0, {
            let pane = |wide: bool| {
                let lines = column![
                    container(space())
                        .width(if wide { 140 } else { 80 })
                        .height(8)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.16))),
                    container(space())
                        .width(if wide { 220 } else { 110 })
                        .height(6)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                    container(space())
                        .width(if wide { 180 } else { 90 })
                        .height(6)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                ]
                .spacing(8);
                let focused = wide;
                container(lines)
                    .padding(14)
                    .width(if wide {
                        Length::FillPortion(3)
                    } else {
                        Length::FillPortion(2)
                    })
                    .height(Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(style::color(Rgba::hex(0x15171b)))),
                        border: Border {
                            color: style::color(match (layout.borders, focused) {
                                (true, true) => palette.accent,
                                (true, false) => Rgba::hex(0xffffff).alpha(0.08),
                                (false, _) => Rgba::hex(0).alpha(0.0),
                            }),
                            width: if layout.borders { 1.0 } else { 0.0 },
                            radius: f32::from(layout.radius).into(),
                        },
                        shadow: if layout.shadows {
                            Shadow {
                                color: iced::Color::from_rgba8(0, 0, 0, 0.45),
                                offset: Vector::new(0.0, 8.0),
                                blur_radius: 24.0,
                            }
                        } else {
                            Shadow::default()
                        },
                        ..container::Style::default()
                    })
            };
            row![pane(true), pane(false)]
                .spacing(f32::from(layout.gaps))
                .padding(f32::from(layout.gaps).max(4.0))
                .into()
        });

        column![
            preview,
            desktop,
            group(
                "Windows",
                vec![
                    setting(
                        "Gaps",
                        "Space between windows and around them",
                        value_slider(0..=32, layout.gaps, Message::Gaps, "px", palette),
                    ),
                    setting(
                        "Corner radius",
                        "How round window corners are",
                        value_slider(0..=24, layout.radius, Message::Radius, "px", palette),
                    ),
                    setting(
                        "Borders",
                        "A thin line around the focused window; a gradient flows across the workspace",
                        segmented(
                            [(0u8, "Off", None), (1, "Accent", None), (2, "Gradient", None)],
                            match (layout.borders, layout.gradient) {
                                (false, _) => 0,
                                (true, false) => 1,
                                (true, true) => 2,
                            },
                            Message::BorderStyle,
                        ),
                    ),
                    setting(
                        "Shadows",
                        "Soft shadows under windows. Play mode always turns them off",
                        switch(layout.shadows, Message::Shadows, palette),
                    ),
                ]
            ),
        ]
        .spacing(22)
        .into()
    }

    fn windows(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let windows = &self.config.windows;
        let width = windows.width.unwrap_or(Width::Half);
        let opacity = windows.opacity.unwrap_or(1.0);
        let inactive = windows.inactive_opacity.unwrap_or(opacity);
        let centered = windows.center == Some(Center::Always)
            || (windows.center == Some(Center::Overflow) && width == Width::Full);

        let pane = move |focused: bool, portion: u16| {
            let alpha = if focused { opacity } else { inactive };
            container(
                column![
                    container(space())
                        .width(Length::FillPortion(3))
                        .height(7)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.16 * alpha))),
                    container(space())
                        .width(Length::FillPortion(2))
                        .height(5)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08 * alpha))),
                ]
                .spacing(7),
            )
            .padding(12)
            .width(Length::FillPortion(portion.max(1)))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x15171b).alpha(alpha * 0.96),
                ))),
                border: Border {
                    color: style::color(if focused {
                        palette.accent
                    } else {
                        Rgba::hex(0xffffff).alpha(0.08)
                    }),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..container::Style::default()
            })
        };
        let focused = (width.proportion() * 1000.0) as u16;
        let rest = 1000 - focused.min(1000);
        let strip: Element<'_, Message> = if width == Width::Full {
            row![pane(true, 1000)].into()
        } else if centered {
            row![
                pane(false, rest / 2),
                pane(true, focused),
                pane(false, rest / 2)
            ]
            .spacing(10)
            .into()
        } else {
            row![
                pane(false, rest / 3),
                pane(true, focused),
                pane(false, rest - rest / 3)
            ]
            .spacing(10)
            .into()
        };
        let preview = self.preview(
            &self.config.desktop.wallpaper,
            170.0,
            container(strip).padding(12).into(),
        );

        let presets = windows
            .presets
            .clone()
            .unwrap_or_else(|| vec![Width::Third, Width::Half, Width::TwoThirds]);
        let preset_chips = Row::with_children(Width::ALL.map(|w| {
            button(text(w.key()).size(13))
                .padding([6, 12])
                .style(style::choice(presets.contains(&w)))
                .on_press(Message::WinPreset(w))
                .into()
        }))
        .spacing(4);

        let speed = match windows.animations {
            Some(s) if s <= 0.0 => 0,
            Some(s) if s > 1.25 => 1,
            Some(s) if s < 0.8 => 3,
            _ => 2,
        };
        let percent = |value: f32| (value * 100.0).round() as u16;

        let mut rules: Vec<Element<'_, Message>> = self
            .config
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let mut facts = Vec::new();
                match rule.floating {
                    Some(true) => facts.push("floating".to_owned()),
                    Some(false) => facts.push("tiled".to_owned()),
                    None => {}
                }
                if let Some(width) = rule.width {
                    facts.push(format!("{} wide", width.key()));
                }
                if let Some(opacity) = rule.opacity {
                    facts.push(format!("{}% opaque", percent(opacity)));
                }
                row![
                    icon(Icon::AppWindow, 15.0, style::FAINT),
                    text(rule.app.as_str()).size(13).font(style::mono()),
                    text(facts.join(" · "))
                        .size(12)
                        .color(style::color(style::FAINT))
                        .width(Fill),
                    button(icon(Icon::X, 14.0, style::FAINT))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveRule(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 16])
                .into()
            })
            .collect();
        let width_choices = Row::with_children(Width::ALL.map(|w| {
            button(text(w.key()).size(12))
                .padding([5, 9])
                .style(style::choice(self.rule.width == Some(w)))
                .on_press(Message::RuleWidth(w))
                .into()
        }))
        .spacing(2);
        rules.push(
            row![
                text_input("app id, like pavucontrol", &self.rule.app)
                    .on_input(Message::RuleApp)
                    .on_submit(Message::AddRule)
                    .padding([8, 12])
                    .size(13)
                    .font(style::mono())
                    .width(230)
                    .style(style::field(palette)),
                row![
                    text("Floating")
                        .size(12)
                        .color(style::color(style::SOFT_TEXT)),
                    switch(
                        self.rule.floating == Some(true),
                        Message::RuleFloating,
                        palette
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                width_choices,
                space::horizontal(),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::primary(palette))
                    .on_press_maybe((!self.rule.app.trim().is_empty()).then_some(Message::AddRule)),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
            .padding([12, 16])
            .into(),
        );
        rules.push(note(
            "The app id is what niri calls the window; `niri msg windows` lists them.",
        ));

        column![
            preview,
            group(
                "New windows",
                vec![
                    setting(
                        "Open at",
                        if windows.width.is_none() {
                            "Not set here, so niri's own choice applies"
                        } else {
                            "Width of a new column"
                        },
                        segmented(
                            Width::ALL.map(|w| (w, w.key(), None)),
                            windows.width,
                            Message::WinWidth,
                        ),
                    ),
                    setting("Widths Mod+R cycles through", "", preset_chips),
                ]
            ),
            group(
                "Focus",
                vec![
                    setting(
                        "Alt+Tab with previews",
                        "Recent windows first; Alt+` stays in one app; arrows, Home and End move",
                        switch(windows.switcher, Message::RecentWindows, palette),
                    ),
                    setting(
                        "Center the focused window",
                        "",
                        segmented(
                            [
                                (Center::Never, "Never", None),
                                (Center::Overflow, "When it doesn't fit", None),
                                (Center::Always, "Always", None),
                            ],
                            windows.center,
                            Message::WinCenter,
                        ),
                    ),
                    setting(
                        "Center a lone window",
                        "One window on a workspace sits in the middle",
                        switch(windows.center_lone, Message::CenterLone, palette),
                    ),
                    setting(
                        "Focus follows the mouse",
                        "Pointing at a window focuses it",
                        switch(windows.focus_follows_mouse, Message::FocusFollows, palette),
                    ),
                    setting(
                        "Pointer follows focus",
                        "Switching windows with the keyboard moves the pointer along",
                        switch(windows.warp_mouse, Message::WarpMouse, palette),
                    ),
                ]
            ),
            group(
                "Look",
                vec![
                    setting(
                        "Focused window",
                        "How opaque the window you're in is",
                        value_slider(30..=100, percent(opacity), Message::Opacity, "%", palette),
                    ),
                    setting(
                        "Other windows",
                        "Fade the rest to see what's in focus",
                        value_slider(
                            30..=100,
                            percent(inactive),
                            Message::InactiveOpacity,
                            "%",
                            palette
                        ),
                    ),
                    setting(
                        "Blur behind",
                        "Shows through see-through windows: just the wallpaper, or everything under them",
                        segmented(
                            [
                                (None, "Off", None),
                                (Some(Blur::Wallpaper), "Wallpaper", None),
                                (Some(Blur::All), "Everything", None),
                            ],
                            Some(windows.blur),
                            Message::WinBlur,
                        ),
                    ),
                ]
            ),
            group(
                "Motion",
                vec![
                    setting(
                        "Speed",
                        "Performance modes always turn animations off",
                        segmented(
                            [
                                (0u8, "Off", None),
                                (1, "Fast", None),
                                (2, "Normal", None),
                                (3, "Slow", None),
                            ],
                            speed,
                            Message::Animations,
                        ),
                    ),
                    setting(
                        "Feel",
                        "How windows and workspaces move; bouncy overshoots a little",
                        segmented(
                            [
                                (None, "niri's", None),
                                (Some(Motion::Snappy), "Snappy", None),
                                (Some(Motion::Smooth), "Smooth", None),
                                (Some(Motion::Bouncy), "Bouncy", None),
                            ],
                            Some(windows.motion),
                            Message::WinMotion,
                        ),
                    ),
                    setting(
                        "New windows",
                        "How a window shows up when it opens",
                        segmented(
                            [
                                (None, "niri's", None),
                                (Some(Appear::Pop), "Pop", None),
                                (Some(Appear::Slide), "Slide", None),
                                (Some(Appear::Fade), "Fade", None),
                            ],
                            Some(windows.appear),
                            Message::WinAppear,
                        ),
                    ),
                ]
            ),
            group("Apps", rules),
        ]
        .spacing(22)
        .into()
    }

    fn pinned_apps(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let mut rows: Vec<Element<'_, Message>> = self
            .config
            .pinned
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let name = apps::find(&self.apps, id).map_or(id.as_str(), |app| app.name.as_str());
                row![
                    crate::windows::picture(&self.apps, id, 20.0, palette).map(|_| Message::Noop),
                    text(name).size(13).width(Fill),
                    button(icon(Icon::X, 14.0, style::FAINT))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::UnpinApp(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([8, 16])
                .into()
            })
            .collect();
        let query = self.pin_query.trim();
        let mut found = Row::new().spacing(6);
        if !query.is_empty() {
            let mut matches: Vec<(u32, &App)> = self
                .apps
                .iter()
                .filter(|app| !self.config.pinned.contains(&app.id))
                .filter_map(|app| apps::score(query, app).map(|score| (score, app)))
                .collect();
            matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            for (_, app) in matches.into_iter().take(4) {
                found = found.push(
                    button(
                        row![
                            crate::windows::picture(&self.apps, &app.id, 16.0, palette)
                                .map(|_| Message::Noop),
                            text(app.name.as_str()).size(12),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 10])
                    .style(style::secondary)
                    .on_press(Message::PinApp(app.id.clone())),
                );
            }
        }
        rows.push(
            column![
                text_input("Find an app to pin", &self.pin_query)
                    .on_input(Message::PinQuery)
                    .padding([8, 12])
                    .size(13)
                    .style(style::field(palette)),
                found,
            ]
            .spacing(10)
            .padding([12, 16])
            .into(),
        );
        rows.push(note(
            "Pinned apps sit before the open windows. A click jumps to the app or starts it; right-click any icon on the bar to pin or unpin it.",
        ));
        rows
    }

    fn bar(&self) -> Element<'_, Message> {
        let look = self.config.bar;
        let palette = self.palette();
        let status = &self.sample;
        let preview = self.preview(&self.config.desktop.wallpaper, 96.0, {
            let bar = bar::view(bar::Bar {
                mode: None,
                has_modes: !self.config.modes.is_empty(),
                palette,
                active: status.active,
                status: status.clone(),
                battery: status.battery,
                track: None,
                controls: &self.controls,
                held: 2,
                do_not_disturb: self.config.alerts.do_not_disturb,
                awake: false,
                apps: &self.apps,
                now: chrono::Local::now(),
                look,
                panel_open: false,
                pinned: self
                    .config
                    .pinned
                    .iter()
                    .map(|id| bar::Pinned {
                        id: id.clone(),
                        running: false,
                        focused: false,
                    })
                    .collect(),
            })
            .map(|_| Message::Noop);
            let inset = if look.floating { 10.0 } else { 0.0 };
            container(bar)
                .padding(inset)
                .height(Fill)
                .align_y(if look.position == Edge::Bottom {
                    Alignment::End
                } else {
                    Alignment::Start
                })
                .into()
        });
        column![
            preview,
            group(
                "Place and look",
                vec![
                    setting(
                        "Edge",
                        "",
                        segmented(
                            [(Edge::Top, "Top", None), (Edge::Bottom, "Bottom", None)],
                            look.position,
                            Message::BarEdge,
                        ),
                    ),
                    setting(
                        "Style",
                        "An island with a gap around it, or flush with the screen edge",
                        segmented(
                            [(true, "Floating", None), (false, "Edge to edge", None)],
                            look.floating,
                            Message::BarFloating,
                        ),
                    ),
                    setting(
                        "Frosted",
                        "Blur what's behind the bar, like frosted glass",
                        switch(look.blur, Message::BarBlur, palette),
                    ),
                    setting(
                        "Glass",
                        "How solid the bar is over the wallpaper",
                        value_slider(
                            0..=100,
                            (look.opacity * 100.0).round() as u16,
                            Message::BarOpacity,
                            "%",
                            palette,
                        ),
                    ),
                ]
            ),
            group("Pinned apps", self.pinned_apps(palette)),
            group(
                "Calendar",
                vec![
                    setting(
                        "Weather",
                        "A city, or latitude,longitude. Open-Meteo is asked every 30 minutes; nothing is sent until you set one",
                        row![
                            text_input(
                                "Lisbon",
                                self.weather_draft
                                    .as_deref()
                                    .or(self.config.weather.as_deref())
                                    .unwrap_or(""),
                            )
                            .on_input(Message::WeatherDraft)
                            .on_submit(Message::SetWeather)
                            .padding([8, 12])
                            .size(13)
                            .width(180)
                            .style(style::field(palette)),
                            button(text("Save").size(13))
                                .padding([8, 14])
                                .style(style::secondary)
                                .on_press_maybe(self.weather_draft.is_some().then_some(Message::SetWeather)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                ]
            ),
            group(
                "Workspaces",
                vec![setting(
                    "Show workspaces as",
                    "Dots stay calm, numbers match Mod+1…9",
                    segmented(
                        [
                            (Workspaces::Dots, "Dots", None),
                            (Workspaces::Numbers, "Numbers", None)
                        ],
                        look.workspaces,
                        Message::Workspaces,
                    ),
                )]
            ),
            group(
                "Clock",
                vec![
                    setting(
                        "Date",
                        "Weekday, day and month next to the time",
                        switch(look.date, Message::Date, palette),
                    ),
                    setting(
                        "Seconds",
                        "",
                        switch(look.seconds, Message::Seconds, palette)
                    ),
                ]
            ),
            group(
                "Battery",
                vec![setting(
                    "Percentage",
                    "Next to the battery icon",
                    switch(look.battery_percent, Message::BatteryPercent, palette),
                )]
            ),
        ]
        .spacing(22)
        .into()
    }

    fn lock(&self) -> Element<'_, Message> {
        let lock = self.config.lock;
        let palette = self.palette();
        let preview = self.preview(&self.config.desktop.wallpaper, 220.0, {
            let pill = container(
                row![
                    container(text("D").size(10).color(style::color(palette.on_accent)))
                        .center(18)
                        .style(style::pill(palette.accent)),
                    text("Password").size(10).color(style::color(style::FAINT)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(150)
            .padding([4, 6])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x0c0d10).alpha(0.5),
                ))),
                border: Border {
                    color: style::color(Rgba::hex(0xffffff).alpha(0.12)),
                    width: 1.0,
                    radius: style::PILL.into(),
                },
                ..container::Style::default()
            });
            let clock = column![
                text(chrono::Local::now().format("%H:%M").to_string())
                    .size(68)
                    .font(style::light()),
                pill
            ]
            .spacing(8);
            let mut right = column![].align_x(Alignment::End);
            if lock.show_media {
                right = right.push(
                    container(
                        row![
                            icon(Icon::Music, 12.0, palette.accent),
                            text("Night Drive").size(10)
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 10])
                    .style(style::pill(Rgba::hex(0x0c0d10).alpha(0.5))),
                );
            }
            column![
                space::vertical(),
                row![clock, space::horizontal(), right].align_y(Alignment::End)
            ]
            .padding(22)
            .into()
        });
        let lock_now = button(
            row![
                icon(Icon::Lock, 14.0, palette.on_accent),
                text("Lock now").size(13)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 14])
        .style(style::primary(palette))
        .on_press(Message::LockNow);
        column![
            preview,
            group(
                "When",
                vec![
                    setting(
                        "Before sleep",
                        "Locks the moment the laptop goes to sleep",
                        switch(lock.before_sleep, Message::LockBeforeSleep, palette),
                    ),
                    setting(
                        "Lock key",
                        "Change it in Shortcuts",
                        keycaps(&self.config.shortcuts.lock),
                    ),
                    setting("Try it", "Your password unlocks it", lock_now),
                ]
            ),
            group(
                "When you step away",
                vec![
                    setting(
                        "Lock after",
                        "Video players keep the screen on while they play",
                        minutes(self.config.idle.lock_after, Message::IdleLock, palette),
                    ),
                    setting(
                        "Screen off after",
                        "Any key or touch wakes it",
                        minutes(self.config.idle.screen_off_after, Message::IdleScreen, palette),
                    ),
                    setting(
                        "Sleep after",
                        "",
                        minutes(self.config.idle.suspend_after, Message::IdleSuspend, palette),
                    ),
                    note("Modes with “keep the screen awake” on skip all of this."),
                ]
            ),
            group(
                "Shows",
                vec![
                    setting(
                        "What's playing",
                        "With a pause button",
                        switch(lock.show_media, Message::LockMedia, palette),
                    ),
                    fact(Icon::Bell, "Notifications stay hidden, only their count shows"),
                    fact(Icon::Keyboard, "The keyboard layout sits next to the password"),
                ]
            ),
            group(
                "Password",
                vec![note(
                    "Checked by PAM, the same way you log in: /etc/pam.d/apolline when it's installed, login otherwise.",
                )]
            ),
        ]
        .spacing(22)
        .into()
    }

    fn notifications(&self) -> Element<'_, Message> {
        let alerts = self.config.alerts;
        let palette = self.palette();
        let test_state: Element<'_, Message> = match &self.tested {
            Some(Ok(())) => text("Sent — look at the top right")
                .size(12)
                .color(style::color(style::GOOD))
                .into(),
            Some(Err(err)) => text(format!("Nothing shows them yet: {err}"))
                .size(12)
                .color(style::color(style::WARN))
                .into(),
            None => space().into(),
        };
        let mut per_mode: Vec<Element<'_, Message>> = self
            .config
            .modes
            .iter()
            .enumerate()
            .map(|(index, mode)| {
                setting_with_icon(
                    Icon::named(&mode.icon),
                    mode.name.as_str(),
                    "",
                    rules(index, mode.notifications),
                )
            })
            .collect();
        if per_mode.is_empty() {
            per_mode.push(note(
                "Without a mode every notification shows. Each mode can hold some back.",
            ));
        }
        column![
            group(
                "Everywhere",
                vec![
                    setting(
                        "Do not disturb",
                        "Holds everything but urgent ones, in every mode",
                        switch(alerts.do_not_disturb, Message::DoNotDisturb, palette),
                    ),
                    setting(
                        "Popups stay for",
                        "Urgent ones stay until you close them",
                        value_slider(2..=30, alerts.seconds, Message::PopupSeconds, "s", palette),
                    ),
                    setting(
                        "Try it",
                        "Sends a notification to whatever shows them",
                        row![
                            test_state,
                            button(
                                row![
                                    icon(Icon::Send, 14.0, neutral::TEXT),
                                    text("Send a test").size(13)
                                ]
                                .spacing(8)
                                .align_y(Alignment::Center)
                            )
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press(Message::Test)
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center),
                    ),
                ]
            ),
            group("In each mode", per_mode),
        ]
        .spacing(22)
        .into()
    }

    fn keyboard(&self) -> Element<'_, Message> {
        let keyboard = &self.config.keyboard;
        let palette = self.palette();

        let mut chips = Row::new().spacing(8).align_y(Alignment::Center);
        for (index, code) in keyboard.layouts.iter().enumerate() {
            let name = LAYOUTS
                .iter()
                .find(|(c, _)| c == code)
                .map_or(code.as_str(), |(_, name)| name);
            chips = chips.push(
                container(
                    row![
                        text(code.to_uppercase())
                            .size(11)
                            .font(style::semibold())
                            .color(style::color(palette.accent)),
                        text(name).size(13),
                        button(icon(Icon::X, 12.0, style::FAINT))
                            .padding(3)
                            .style(style::ghost(6.0))
                            .on_press(Message::RemoveLayout(index)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding(iced::Padding::new(4.0).left(12.0).right(6.0))
                .style(style::pill(style::FILL)),
            );
        }
        let options: Vec<Layout> = LAYOUTS
            .iter()
            .filter(|(code, _)| !keyboard.layouts.iter().any(|l| l == code))
            .map(|&(code, name)| Layout(code, name))
            .collect();
        let picker = pick_list(options, None::<Layout>, Message::AddLayout)
            .placeholder("Add a layout…")
            .text_size(13)
            .padding([8, 12])
            .width(240)
            .style(style::picker(palette))
            .menu_style(style::menu(palette));
        let layouts: Element<'_, Message> = if keyboard.layouts.is_empty() {
            column![
                text("Using the layouts from your niri config")
                    .size(13)
                    .color(style::color(style::FAINT)),
                picker
            ]
            .spacing(12)
            .into()
        } else {
            column![chips.wrap(), picker].spacing(12).into()
        };

        let mut switch_rows = vec![setting(
            "Switch layouts with",
            if keyboard.switch.is_none() {
                "Not set here yet, so niri's choice applies"
            } else {
                ""
            },
            segmented(
                [
                    (Switch::AltShift, "Alt+Shift", None),
                    (Switch::CtrlShift, "Ctrl+Shift", None),
                    (Switch::SuperSpace, "Super+Space", None),
                    (Switch::Caps, "Caps Lock", None),
                ],
                keyboard.switch,
                Message::Switch,
            ),
        )];
        if keyboard.switch == Some(Switch::SuperSpace)
            && keys::same(&self.config.shortcuts.launcher, "Mod+Space")
        {
            switch_rows.push(warning(
                "Super+Space also opens the launcher. Pick another key here or in Shortcuts.",
            ));
        }

        let delay = keyboard.repeat_delay.unwrap_or(600);
        let rate = keyboard.repeat_rate.unwrap_or(25);
        column![
            group("Layouts", vec![padded(layouts)]),
            group("Switching", switch_rows),
            group(
                "Typing",
                vec![
                    setting(
                        "Repeat after",
                        "How long a key is held before it repeats",
                        value_slider(150..=1000, delay, Message::RepeatDelay, "ms", palette),
                    ),
                    setting(
                        "Repeat speed",
                        "Characters per second while held",
                        value_slider(10..=60, rate, Message::RepeatRate, "/s", palette),
                    ),
                    padded(
                        text_input("Hold a key here to try it…", &self.typing)
                            .on_input(Message::Typing)
                            .padding([10, 12])
                            .size(14)
                            .style(style::field(palette)),
                    ),
                ]
            ),
        ]
        .spacing(22)
        .into()
    }

    fn pointer(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let touchpad = self.config.touchpad.unwrap_or_default();
        let mouse = self.config.mouse.unwrap_or_default();
        let mut form = Column::new().spacing(22);
        if self.config.touchpad.is_none() && self.config.mouse.is_none() {
            form = form.push(note(
                "Until you change something here, niri's own settings apply.",
            ));
        }
        form.push(group(
            "Touchpad",
            vec![
                setting(
                    "Tap to click",
                    "A light tap counts as a click",
                    switch(touchpad.tap, Message::Tap, palette),
                ),
                setting(
                    "Natural scrolling",
                    "Content follows your fingers",
                    switch(touchpad.natural_scroll, Message::TouchNatural, palette),
                ),
                setting(
                    "Ignore while typing",
                    "No stray clicks from your palms",
                    switch(touchpad.disable_while_typing, Message::TouchTyping, palette),
                ),
                setting(
                    "Speed",
                    "",
                    speed_slider(touchpad.speed, Message::TouchSpeed, palette),
                ),
            ],
        ))
        .push(group(
            "Mouse",
            vec![
                setting(
                    "Natural scrolling",
                    "",
                    switch(mouse.natural_scroll, Message::MouseNatural, palette),
                ),
                setting(
                    "Acceleration",
                    "Off gives the flat response many players prefer",
                    switch(mouse.acceleration, Message::MouseAcceleration, palette),
                ),
                setting(
                    "Speed",
                    "",
                    speed_slider(mouse.speed, Message::MouseSpeed, palette),
                ),
            ],
        ))
        .into()
    }

    fn shortcuts(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let defaults = apolline_config::Shortcuts::default();
        let mut rows = Vec::new();
        for shortcut in Shortcut::ALL {
            let combo = self.config.shortcuts.get(shortcut);
            let recording = self.recording == Some(shortcut);
            let keys_view: Element<'_, Message> = if recording {
                container(
                    text("Press the keys…")
                        .size(13)
                        .color(style::color(palette.accent)),
                )
                .padding([6, 12])
                .style(style::pill(palette.accent_soft))
                .into()
            } else {
                keycaps(combo)
            };
            let mut controls = row![keys_view].spacing(8).align_y(Alignment::Center);
            controls = controls.push(
                button(text(if recording { "Cancel" } else { "Change" }).size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::Record(if recording {
                        None
                    } else {
                        Some(shortcut)
                    })),
            );
            if combo != defaults.get(shortcut) && !recording {
                controls = controls.push(
                    button(icon(Icon::Restart, 14.0, style::FAINT))
                        .padding(8)
                        .style(style::ghost(8.0))
                        .on_press(Message::ResetShortcut(shortcut)),
                );
            }
            let (title, _) = fragment::action(shortcut);
            let replaced = self
                .niri_binds
                .iter()
                .find(|(bind, _)| keys::same(bind, combo))
                .map(|(_, action)| format!("Replaces {action} from your niri config"));
            rows.push(setting_owned(title, replaced, controls));
        }
        if let Some(error) = &self.shortcut_error {
            rows.push(warning(error));
        }
        rows.push(note(
            "Mod is the Super key, or Alt when niri runs inside another window.",
        ));

        let mut known = Vec::new();
        for (combo, action) in &self.niri_binds {
            known.push(
                row![
                    container(keycaps(combo)).width(Length::FillPortion(2)),
                    text(action.as_str())
                        .width(Length::FillPortion(3))
                        .size(12)
                        .font(style::mono())
                        .color(style::color(style::SOFT_TEXT)),
                ]
                .spacing(16)
                .align_y(Alignment::Center)
                .padding([6, 16])
                .into(),
            );
        }
        let mut form = column![group("Apolline", rows)].spacing(22);
        if !known.is_empty() {
            form = form.push(group(
                "Already in your niri config",
                vec![Column::with_children(known).spacing(0).into()],
            ));
        }
        form.into()
    }

    fn why_on(&self, mode: &ModeSettings, palette: Palette) -> Option<Element<'_, Message>> {
        if self.live.mode.as_deref() != Some(mode.key().as_str()) {
            return None;
        }
        let reason = self.live.reason.as_deref().and_then(Trigger::parse)?;
        let because = match reason {
            Trigger::Game => "a game is running".to_owned(),
            Trigger::Battery => "the laptop runs on battery".to_owned(),
            Trigger::Hours { from, until } => format!(
                "it's between {} and {}",
                apolline_config::clock(from),
                apolline_config::clock(until)
            ),
            Trigger::Monitor(name) => format!("{name} is plugged in"),
        };
        let back = self
            .live
            .back
            .as_deref()
            .and_then(|key| self.config.find(key))
            .map_or("the plain desktop".to_owned(), |i| self.config.modes[i].name.clone());
        Some(
            container(
                row![
                    icon(Icon::Zap, 15.0, palette.accent),
                    text(format!(
                        "On now because {because}. When that ends, {back} comes back."
                    ))
                    .size(13),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .width(Fill)
            .style(style::pill(palette.accent_soft))
            .into(),
        )
    }

    fn set_trigger(&mut self, index: usize, trigger: Trigger, on: bool) {
        let Some(mode) = self.config.modes.get(index) else {
            return;
        };
        let mut when: Vec<Trigger> = mode
            .when
            .iter()
            .filter(|t| std::mem::discriminant(*t) != std::mem::discriminant(&trigger))
            .cloned()
            .collect();
        if on {
            when.push(trigger);
        }
        self.write_triggers(index, &when);
    }

    fn write_triggers(&mut self, index: usize, when: &[Trigger]) {
        let list: Vec<String> = when.iter().map(Trigger::text).collect();
        self.edit(|e| {
            if list.is_empty() {
                e.unset_mode(index, "when");
            } else {
                e.set_mode_list(index, "when", &list);
            }
        });
    }

    fn triggers(&self, mode: &ModeSettings, palette: Palette) -> Vec<Element<'_, Message>> {
        let has = |wanted: &Trigger| {
            mode.when
                .iter()
                .any(|t| std::mem::discriminant(t) == std::mem::discriminant(wanted))
        };
        let mut rows = vec![
            setting(
                "While a game runs",
                "Steam, Proton, gamescope or Wine; it ends when the last game closes",
                switch(has(&Trigger::Game), Message::WhenGame, palette),
            ),
            setting(
                "On battery",
                "Unplugging turns it on, plugging in brings back the mode you had",
                switch(has(&Trigger::Battery), Message::WhenBattery, palette),
            ),
        ];
        let field = |placeholder: &'static str,
                     value: &str,
                     on_input: fn(String) -> Message,
                     submit: Message| {
            row![
                text_input(placeholder, value)
                    .on_input(on_input)
                    .on_submit(submit.clone())
                    .padding([8, 12])
                    .size(13)
                    .width(200)
                    .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press(submit),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };
        rows.push(setting(
            "At these hours",
            "Like 20:00-07:00; it may run past midnight",
            field(
                "20:00-07:00",
                &self.hours_draft,
                Message::HoursDraft,
                Message::SetHours,
            ),
        ));
        rows.push(setting(
            "When a monitor is connected",
            "Its name from niri msg outputs, like HDMI-A-1",
            field(
                "HDMI-A-1",
                &self.monitor_draft,
                Message::MonitorDraft,
                Message::SetMonitor,
            ),
        ));
        let listed: Vec<(usize, &Trigger)> = mode
            .when
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t, Trigger::Hours { .. } | Trigger::Monitor(_)))
            .collect();
        if !listed.is_empty() {
            let chips = Row::with_children(listed.into_iter().map(|(at, trigger)| {
                let label = match trigger {
                    Trigger::Hours { from, until } => {
                        format!(
                            "{} – {}",
                            apolline_config::clock(*from),
                            apolline_config::clock(*until)
                        )
                    }
                    Trigger::Monitor(name) => format!("{name} connected"),
                    _ => trigger.text(),
                };
                container(
                    row![
                        text(label).size(12),
                        button(icon(Icon::X, 12.0, style::FAINT))
                            .padding(2)
                            .style(style::ghost(6.0))
                            .on_press(Message::DropTrigger(at)),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([4, 10])
                .style(style::pill(Rgba::hex(0xffffff).alpha(0.06)))
                .into()
            }))
            .spacing(6)
            .wrap();
            rows.push(padded(chips));
        }
        rows.push(note(
            "When the reason ends, the mode you had before comes back. Switching by hand always wins.",
        ));
        rows
    }

    fn color_apps(&self, palette: Palette) -> Element<'_, Message> {
        let on = self.config.apps.colors;
        let mut about =
            column![
                row![
                    column![
                    text("Color apps too").size(14),
                    text("GTK and Qt apps get the accent and the dark style, and follow your modes")
                        .size(12)
                        .color(style::color(style::FAINT)),
                ]
                    .spacing(3)
                    .width(Fill),
                    switch(on, Message::ColorApps, palette),
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            ]
            .spacing(8);
        let heading = if on {
            "Writes, and removes when you turn it off:"
        } else {
            "Would write, and remove when you turn it off:"
        };
        about = about.push(text(heading).size(11).color(style::color(style::FAINT)));
        for file in crate::paint::files() {
            about = about.push(
                text(file)
                    .size(11)
                    .font(style::mono())
                    .color(style::color(style::FAINT)),
            );
        }
        padded(about)
    }

    fn welcome(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let included = std::fs::read_to_string(fragment::niri_config())
            .is_ok_and(|text| text.contains(&fragment::path().display().to_string()));
        let mut connect = Column::new().spacing(6);
        if included {
            connect = connect.push(
                row![
                    icon(Icon::Done, 16.0, style::GOOD),
                    text("Apolline is in your niri config").size(13),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        } else {
            connect = connect.push(
                button(text("Add Apolline to niri").size(13))
                    .padding([8, 16])
                    .style(style::primary(palette))
                    .on_press(Message::Setup),
            );
        }
        for line in &self.setup_log {
            connect = connect.push(
                text(line.as_str())
                    .size(12)
                    .color(style::color(style::FAINT)),
            );
        }

        let looks = Row::with_children(
            self.themes
                .iter()
                .enumerate()
                .filter(|(_, (_, file))| file.is_none())
                .map(|(index, (theme, _))| self.theme_card(index, theme, false)),
        )
        .spacing(12);

        let modes = Row::with_children(TEMPLATES.map(|name| {
            let have = self.config.find(name).is_some();
            let label = row![
                icon(
                    if have { Icon::Check } else { Icon::Plus },
                    13.0,
                    neutral::TEXT
                ),
                text(name).size(13),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            button(label)
                .padding([7, 14])
                .style(style::choice(have))
                .on_press_maybe((!have).then_some(Message::AddMode(name)))
                .into()
        }))
        .spacing(8)
        .wrap();

        let shortcuts = &self.config.shortcuts;
        let keys = Column::with_children(
            [
                (
                    shortcuts.get(Shortcut::Launcher).to_owned(),
                    "Open apps, games, windows and sums",
                ),
                (
                    shortcuts.get(Shortcut::Panel).to_owned(),
                    "Quick settings and past notifications",
                ),
                ("Alt+Tab".to_owned(), "Jump between recent windows"),
                (
                    shortcuts.get(Shortcut::Keys).to_owned(),
                    "Every other shortcut, searchable",
                ),
                (shortcuts.get(Shortcut::Lock).to_owned(), "Lock the screen"),
            ]
            .into_iter()
            .map(|(combo, what)| {
                row![
                    container(text(combo).size(12).font(style::mono()))
                        .padding([4, 10])
                        .width(150)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                    text(what).size(13).color(style::color(style::SOFT_TEXT)),
                ]
                .spacing(14)
                .align_y(Alignment::Center)
                .into()
            }),
        )
        .spacing(8);

        column![
            welcome_step(
                palette,
                1,
                "Connect to niri",
                "One include line at the end of your niri config; a copy of it is kept.",
                connect.into()
            ),
            welcome_step(
                palette,
                2,
                "Pick a look",
                "Wallpaper, accent, corners and motion in one click.",
                column![
                    looks,
                    row![
                        switch(self.config.apps.colors, Message::ColorApps, palette),
                        text("Color GTK and Qt apps with the same accent").size(13),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                ]
                .spacing(14)
                .into()
            ),
            welcome_step(
                palette,
                3,
                "Modes, if you want them",
                "A mode is one key that changes wallpaper, apps, notifications and performance.",
                modes.into()
            ),
            welcome_step(
                palette,
                4,
                "Five keys to remember",
                "Mod is the Super key.",
                keys.into()
            ),
            row![
                space::horizontal(),
                button(text("More settings").size(13))
                    .padding([9, 16])
                    .style(style::secondary)
                    .on_press(Message::Page(Page::Modes)),
                button(text("Done").size(13))
                    .padding([9, 22])
                    .style(style::primary(palette))
                    .on_press(Message::Finish),
            ]
            .spacing(8),
        ]
        .spacing(14)
        .into()
    }

    fn about(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let niri_running = std::env::var_os("NIRI_SOCKET").is_some();
        let included = std::fs::read_to_string(fragment::niri_config())
            .is_ok_and(|text| text.contains(&fragment::path().display().to_string()));

        let hero = row![
            container(icon(Icon::Sparkles, 28.0, palette.on_accent))
                .center(64)
                .style(style::pill(palette.accent)),
            column![
                text("Apolline").size(34).font(style::light()),
                text(format!(
                    "Version {} · a calm shell for niri",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(13)
                .color(style::color(style::FAINT)),
            ]
            .spacing(2),
        ]
        .spacing(18)
        .align_y(Alignment::Center);

        let status = |good: bool, yes: &str, no: &str| {
            let (glyph, tint, label) = if good {
                (Icon::Done, style::GOOD, yes.to_owned())
            } else {
                (Icon::Warning, style::WARN, no.to_owned())
            };
            row![
                icon(glyph, 15.0, tint),
                text(label).size(13).color(style::color(tint))
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        let setup: Element<'_, Message> = if included {
            status(true, "Included in your niri config", "").into()
        } else {
            row![
                status(false, "", "Not set up"),
                button(text("Set up").size(13))
                    .padding([7, 14])
                    .style(style::primary(palette))
                    .on_press(Message::Setup),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        };

        let mut rows = vec![
            setting(
                "niri",
                "",
                status(niri_running, "Running", "Not running in this session"),
            ),
            setting(
                "Apolline in niri",
                "Wallpaper, bar, keys and window style",
                setup,
            ),
            setting_owned(
                "Config file",
                Some(self.path.display().to_string()),
                button(
                    row![
                        icon(Icon::External, 14.0, neutral::TEXT),
                        text("Open").size(13)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .style(style::secondary)
                .on_press(Message::OpenFile),
            ),
        ];
        for line in &self.setup_log {
            rows.push(note(line));
        }

        let mut check = vec![setting(
            "Config check",
            "The same as running  apolline check",
            status(
                self.diagnostics.is_empty() && self.editor.is_ok(),
                "No problems",
                "Needs a look",
            ),
        )];
        if let Err(err) = &self.editor {
            check.push(warning(err));
        }
        for diagnostic in &self.diagnostics {
            let mut line = format!("line {}: {}", diagnostic.line, diagnostic.message);
            if let Some(hint) = &diagnostic.hint {
                line.push_str(&format!(" — {hint}"));
            }
            check.push(warning(&line));
        }

        column![hero, group("Status", rows), group("Health", check)]
            .spacing(26)
            .into()
    }

    fn preview<'a>(
        &'a self,
        name: &str,
        height: f32,
        overlay: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let picture: Element<'a, Message> = match self.thumb(name) {
            Some(handle) => container(
                image(handle.clone())
                    .width(Fill)
                    .height(height)
                    .content_fit(ContentFit::Cover)
                    .border_radius(14.0),
            )
            .clip(true)
            .into(),
            None => container(space())
                .width(Fill)
                .height(height)
                .style(|_| container::Style {
                    background: Some(Background::Color(style::color(neutral::INK))),
                    border: Border {
                        radius: 14.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into(),
        };
        container(stack![picture, container(overlay).height(height)])
            .padding(1)
            .style(|_| container::Style {
                border: Border {
                    color: style::color(neutral::HAIRLINE),
                    width: 1.0,
                    radius: 15.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    fn writes(&self, code: String) -> Element<'_, Message> {
        let palette = self.palette();
        let code = code.trim().to_owned();
        let code = if code.is_empty() {
            "# nothing set yet, defaults apply".to_owned()
        } else {
            code
        };
        let mut panel = column![
            row![
                icon(Icon::Code, 15.0, style::FAINT),
                text("config.toml").size(13).font(style::medium()),
                space::horizontal(),
                text("live").size(11).color(style::color(style::GOOD)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            highlight(&code, palette),
            text("Saved as you change it. Your own comments in the file stay.")
                .size(12)
                .color(style::color(style::FAINT)),
        ]
        .spacing(14);
        if let Some(error) = &self.error {
            panel = panel.push(
                text(error.as_str())
                    .size(12)
                    .color(style::color(style::WARN)),
            );
        }
        container(scrollable(panel))
            .width(Length::Fixed(340.0))
            .padding(18)
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x000000).alpha(0.3),
                ))),
                border: Border {
                    color: style::color(neutral::HAIRLINE),
                    width: 1.0,
                    radius: style::RADIUS_TILE.into(),
                },
                ..container::Style::default()
            })
            .into()
    }
}

fn accent_picker<'a>(
    current: Option<Accent>,
    auto: Rgba,
    inherit: bool,
    on: fn(Option<Accent>) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    let mut swatches = Row::new().spacing(10).align_y(Alignment::Center);
    if inherit {
        swatches = swatches.push(
            button(text("Desktop's").size(12))
                .padding([6, 14])
                .style(style::choice(current.is_none()))
                .on_press(on(None)),
        );
    }
    swatches = swatches.push(
        button(
            row![
                container(Space::new().width(10).height(10)).style(style::pill(auto)),
                text("From wallpaper").size(12),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([6, 14])
        .style(style::choice(current == Some(Accent::Auto)))
        .on_press(on(Some(Accent::Auto))),
    );
    for color in ACCENTS {
        let chosen = current == Some(Accent::Fixed(color));
        swatches = swatches.push(
            button(
                container(if chosen {
                    Element::from(icon(Icon::Check, 14.0, neutral::INK))
                } else {
                    space().into()
                })
                .center(26),
            )
            .padding(0)
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(style::color(color))),
                border: Border {
                    color: style::color(neutral::TEXT),
                    width: if chosen || status == button::Status::Hovered {
                        2.0
                    } else {
                        0.0
                    },
                    radius: style::PILL.into(),
                },
                ..button::Style::default()
            })
            .on_press(on(Some(Accent::Fixed(color)))),
        );
    }
    let (badge, tint) = if readable(palette) {
        ("Readable on any wallpaper", style::GOOD)
    } else {
        ("Low contrast", style::WARN)
    };
    column![
        swatches
            .push(space::horizontal())
            .push(text(badge).size(12).color(style::color(tint))),
        text("The color is taken from the picture and lightened until it reads on glass.")
            .size(12)
            .color(style::color(style::FAINT)),
    ]
    .spacing(10)
    .into()
}

fn desktop_wallpaper(name: Option<String>) -> Message {
    Message::DesktopWallpaper(name.unwrap_or_else(|| "fog.jpg".into()))
}

fn desktop_accent(accent: Option<Accent>) -> Message {
    Message::DesktopAccent(accent.unwrap_or(Accent::Auto))
}

fn mode_wallpaper(name: Option<String>) -> Message {
    Message::ModeWallpaper(name)
}

fn mode_accent(accent: Option<Accent>) -> Message {
    Message::ModeAccent(accent)
}

fn group<'a>(title: &'a str, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut body = Column::new();
    let count = rows.len();
    for (index, row) in rows.into_iter().enumerate() {
        body = body.push(row);
        if index + 1 < count {
            body = body.push(
                container(Space::new().height(1))
                    .width(Fill)
                    .style(|_| container::background(style::color(neutral::HAIRLINE))),
            );
        }
    }
    column![
        text(title.to_uppercase())
            .size(11)
            .font(style::semibold())
            .color(style::color(style::FAINT)),
        container(body).width(Fill).style(style::card),
    ]
    .spacing(10)
    .into()
}

fn padded<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).padding([14, 16]).width(Fill).into()
}

fn setting<'a>(
    title: &'a str,
    about: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    setting_owned(
        title,
        (!about.is_empty()).then(|| about.to_owned()),
        control,
    )
}

fn setting_owned<'a>(
    title: &'a str,
    about: Option<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut words = column![text(title).size(14).font(style::medium())].spacing(3);
    if let Some(about) = about {
        words = words.push(text(about).size(12).color(style::color(style::FAINT)));
    }
    row![words.width(Fill), control.into()]
        .spacing(20)
        .align_y(Alignment::Center)
        .padding([14, 16])
        .into()
}

fn setting_with_icon<'a>(
    glyph: Icon,
    title: &'a str,
    about: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        container(icon(glyph, 15.0, neutral::TEXT))
            .center(30)
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(style::FILL))),
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        container(setting(title, about, control)).width(Fill),
    ]
    .align_y(Alignment::Center)
    .padding(iced::Padding::new(0.0).left(16.0))
    .into()
}

fn fact<'a>(glyph: Icon, line: &'a str) -> Element<'a, Message> {
    row![
        icon(glyph, 15.0, style::SOFT_TEXT),
        text(line).size(13).color(style::color(style::SOFT_TEXT))
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

fn note<'a>(line: &str) -> Element<'a, Message> {
    row![
        icon(Icon::Info, 14.0, style::FAINT),
        text(line.to_owned())
            .size(12)
            .color(style::color(style::FAINT))
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

fn warning<'a>(line: &str) -> Element<'a, Message> {
    row![
        icon(Icon::Warning, 14.0, style::WARN),
        text(line.to_owned())
            .size(12)
            .color(style::color(style::WARN))
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

fn segmented<'a, T: Copy + PartialEq + 'a, const N: usize>(
    options: [(T, &'a str, Option<Icon>); N],
    current: impl Into<Option<T>>,
    on: fn(T) -> Message,
) -> Element<'a, Message> {
    let current = current.into();
    let buttons = Row::with_children(options.map(|(value, label, glyph)| {
        let on_now = Some(value) == current;
        let mut content = Row::new().spacing(7).align_y(Alignment::Center);
        if let Some(glyph) = glyph {
            content = content.push(icon(
                glyph,
                14.0,
                if on_now { neutral::TEXT } else { style::FAINT },
            ));
        }
        content = content.push(text(label).size(13));
        button(content)
            .padding([6, 14])
            .style(style::choice(on_now))
            .on_press(on(value))
            .into()
    }))
    .spacing(2);
    container(buttons).padding(3).style(style::segments).into()
}

fn rules<'a>(index: usize, current: Notifications) -> Element<'a, Message> {
    let on = move |n| Message::Notifications(index, n);
    let buttons = Row::with_children(
        [
            (Notifications::All, "All"),
            (Notifications::Mentions, "Hold quiet ones"),
            (Notifications::Hold, "Only urgent"),
        ]
        .map(|(value, label)| {
            button(text(label).size(13))
                .padding([6, 12])
                .style(style::choice(value == current))
                .on_press(on(value))
                .into()
        }),
    )
    .spacing(2);
    container(buttons).padding(3).style(style::segments).into()
}

fn switch<'a>(on: bool, message: fn(bool) -> Message, palette: Palette) -> Element<'a, Message> {
    toggler(on)
        .on_toggle(message)
        .size(22)
        .style(style::switch(palette))
        .into()
}

fn value_slider<'a>(
    range: std::ops::RangeInclusive<u16>,
    value: u16,
    message: fn(u16) -> Message,
    unit: &'a str,
    palette: Palette,
) -> Element<'a, Message> {
    row![
        slider(range, value, message)
            .on_release(Message::Commit)
            .width(240)
            .style(style::rail(palette, 4.0)),
        text(format!("{value} {unit}"))
            .size(12)
            .font(style::mono())
            .color(style::color(style::SOFT_TEXT))
            .width(Length::Fixed(64.0))
            .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn minutes<'a>(value: u16, message: fn(u16) -> Message, palette: Palette) -> Element<'a, Message> {
    let label = match value {
        0 => "never".to_owned(),
        minutes => format!("{minutes} min"),
    };
    row![
        slider(0..=60u16, value, message)
            .on_release(Message::Commit)
            .width(240)
            .style(style::rail(palette, 4.0)),
        text(label)
            .size(12)
            .font(style::mono())
            .color(style::color(style::SOFT_TEXT))
            .width(Length::Fixed(64.0))
            .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn speed_slider<'a>(
    value: f32,
    message: fn(f32) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    row![
        text("Slow").size(12).color(style::color(style::FAINT)),
        slider(-1.0..=1.0, value, message)
            .step(0.05_f32)
            .on_release(Message::Commit)
            .width(200)
            .style(style::rail(palette, 4.0)),
        text("Fast").size(12).color(style::color(style::FAINT)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn keycaps<'a>(combo: &str) -> Element<'a, Message> {
    Row::with_children(keys::caps(combo).into_iter().map(|cap| {
        container(text(cap).size(12).font(style::mono()))
            .padding([3, 8])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(style::FILL))),
                border: Border {
                    color: style::color(neutral::HAIRLINE),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Shadow {
                    color: iced::Color::from_rgba8(0, 0, 0, 0.5),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 0.0,
                },
                ..container::Style::default()
            })
            .into()
    }))
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

fn highlight<'a>(code: &str, palette: Palette) -> Element<'a, Message> {
    let faint = style::color(style::FAINT);
    let lines = code.lines().map(|line| {
        let mut spans: Vec<Span<'static, ()>> = Vec::new();
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            spans.push(span(line.to_owned()).color(faint));
        } else if trimmed.starts_with('[') {
            spans.push(span(line.to_owned()).color(style::color(palette.accent)));
        } else if let Some((key, rest)) = line.split_once('=') {
            let (value, comment) = split_comment(rest);
            let tint = if value.trim_start().starts_with(['"', '[']) {
                STRING
            } else {
                style::WARN
            };
            spans.push(span(key.to_owned()).color(style::color(neutral::TEXT)));
            spans.push(span("=").color(faint));
            spans.push(span(value.to_owned()).color(style::color(tint)));
            if !comment.is_empty() {
                spans.push(span(comment.to_owned()).color(faint));
            }
        } else {
            spans.push(span(line.to_owned()));
        }
        rich_text(spans)
            .size(12)
            .font(style::mono())
            .line_height(1.5)
            .into()
    });
    Column::with_children(lines).into()
}

fn split_comment(rest: &str) -> (&str, &str) {
    let mut quoted = false;
    for (index, c) in rest.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return rest.split_at(index),
            _ => {}
        }
    }
    (rest, "")
}

fn is_bundled(name: &str) -> bool {
    wallpaper::bundled().iter().any(|(n, _)| *n == name)
}

async fn thumbnails() -> Vec<(String, image::Handle)> {
    let make = || {
        wallpaper::bundled()
            .iter()
            .filter_map(|(name, bytes)| {
                let thumb = ::image::load_from_memory(bytes)
                    .ok()?
                    .thumbnail(760, 475)
                    .to_rgba8();
                let (width, height) = thumb.dimensions();
                Some((
                    (*name).to_owned(),
                    image::Handle::from_rgba(width, height, thumb.into_raw()),
                ))
            })
            .collect()
    };
    tokio::task::spawn_blocking(make).await.unwrap_or_default()
}

fn read_niri_binds() -> Vec<(String, String)> {
    std::fs::read_to_string(fragment::niri_config())
        .map(|text| keys::binds(&text))
        .unwrap_or_default()
}

fn welcome_step<'a>(
    palette: Palette,
    number: u8,
    title: &'static str,
    about: &'static str,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let badge = text(number.to_string())
        .size(13)
        .font(style::semibold())
        .color(style::color(palette.on_accent));
    container(
        row![
            container(badge)
                .center(28)
                .style(style::pill(palette.accent)),
            column![
                text(title).size(16).font(style::semibold()),
                text(about).size(13).color(style::color(style::FAINT)),
                body,
            ]
            .spacing(10)
            .width(Fill),
        ]
        .spacing(16),
    )
    .padding(20)
    .width(Fill)
    .style(style::card)
    .into()
}

fn readable(palette: Palette) -> bool {
    let panel = neutral::PANEL.over(Rgba::hex(0x808080));
    palette.on_accent.contrast(palette.accent) >= 4.5 && palette.accent.contrast(panel) >= 4.5
}

fn broken(err: &str) -> Element<'_, Message> {
    column![
        text("Your config needs a fix first")
            .size(26)
            .font(style::semibold()),
        text("Settings never rewrites a file it can't read, so nothing is lost.")
            .size(14)
            .color(style::color(style::FAINT)),
        container(
            text(err)
                .font(style::mono())
                .size(13)
                .color(style::color(style::WARN))
        )
        .padding(16)
        .width(Fill)
        .style(style::card),
        text("Fix it in the file — this window picks it up as soon as you save.").size(13),
    ]
    .spacing(16)
    .into()
}

fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolline_config::{Mouse, Touchpad};

    #[test]
    fn undo_walks_the_file_back_and_redo_forward() {
        let dir = std::env::temp_dir().join(format!("apolline-undo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\n[layout]\ngaps = 10\n").unwrap();
        let (mut settings, _) = Settings::open(path.clone(), None);

        let _ = settings.update(Message::Gaps(20));
        let _ = settings.update(Message::Commit);
        let _ = settings.update(Message::ApplyTheme(1));
        assert_eq!(settings.config.desktop.theme.as_deref(), Some("Ember"));

        let _ = settings.update(Message::Undo);
        assert_eq!(settings.config.desktop.theme, None);
        assert_eq!(settings.config.layout.gaps, 20);
        let _ = settings.update(Message::Undo);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# mine\n[layout]\ngaps = 10\n"
        );
        let _ = settings.update(Message::Undo);
        assert_eq!(settings.config.layout.gaps, 10, "nothing left to undo");

        let _ = settings.update(Message::Redo);
        assert_eq!(settings.config.layout.gaps, 20);
        let _ = settings.update(Message::Gaps(4));
        let _ = settings.update(Message::Commit);
        assert!(
            settings.redo.is_empty(),
            "a new change drops what was undone"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clicks_end_up_in_the_file_and_comments_survive() {
        let dir = std::env::temp_dir().join(format!("apolline-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, include_str!("../../../assets/config.toml")).unwrap();

        let (mut settings, _) = Settings::open(path.clone(), None);
        let _ = settings.update(Message::AddMode("Work"));
        let _ = settings.update(Message::AddMode("Evening"));
        assert_eq!(settings.selected, 1);
        let _ = settings.update(Message::ModeAccent(Some(Accent::Fixed(Rgba::hex(
            0xf472b6,
        )))));
        let _ = settings.update(Message::Notifications(1, Notifications::Hold));
        let _ = settings.update(Message::ModeName("Night Owl".into()));
        let _ = settings.update(Message::ModeNameDone);
        let _ = settings.update(Message::StartHere(true));
        let _ = settings.update(Message::RecordModeKey);
        let _ = settings.update(Message::Key(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character("n".into()),
            modified_key: keyboard::Key::Character("n".into()),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::LOGO,
            text: None,
            repeat: false,
        }));
        let _ = settings.update(Message::KeyCommand("foot -e btop".into()));
        let _ = settings.update(Message::AddModeKey);
        let _ = settings.update(Message::MoveMode(-1));
        assert_eq!(settings.selected, 0);
        let _ = settings.update(Message::Draft("mpv --shuffle ~/Music".into()));
        let _ = settings.update(Message::Add);
        let _ = settings.update(Message::Draft("foot".into()));
        let _ = settings.update(Message::Add);
        let _ = settings.update(Message::Remove(0));
        let _ = settings.update(Message::Gaps(14));
        let _ = settings.update(Message::Commit);
        let _ = settings.update(Message::AddLayout(Layout("ru", "Russian")));
        let _ = settings.update(Message::Switch(Switch::Caps));
        let _ = settings.update(Message::Tap(false));
        let _ = settings.update(Message::TouchSpeed(0.25));
        let _ = settings.update(Message::Commit);

        let text = std::fs::read_to_string(&path).unwrap();
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        let evening = &config.modes[0];
        assert_eq!(evening.name, "Night Owl");
        assert_eq!(evening.accent, Some(Accent::Fixed(Rgba::hex(0xf472b6))));
        assert_eq!(evening.notifications, Notifications::Hold);
        assert_eq!(evening.open, ["foot"]);
        assert_eq!(
            evening.keys,
            [("Mod+N".to_owned(), "foot -e btop".to_owned())]
        );
        assert_eq!(config.start(), Some(0));
        assert_eq!(config.modes[1].name, "Work");

        let _ = settings.update(Message::RemoveMode);
        let _ = settings.update(Message::RemoveMode);
        let (config, problems) = Config::load(&path);
        assert_eq!(problems, vec![]);
        assert_eq!(config.modes.len(), 1);
        assert_eq!(config.start(), None, "deleting the start mode clears it");
        assert_eq!(config.layout.gaps, 14);
        assert_eq!(config.keyboard.layouts, ["ru"]);
        assert_eq!(config.keyboard.switch, Some(Switch::Caps));
        assert_eq!(
            config.touchpad,
            Some(Touchpad {
                tap: false,
                speed: 0.25,
                ..Touchpad::default()
            })
        );
        assert_eq!(config.mouse, None::<Mouse>);
        assert!(text.contains("# Check it at any time with:  apolline check"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_shortcut_cannot_take_keys_another_one_uses() {
        let dir = std::env::temp_dir().join(format!("apolline-keys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let (mut settings, _) = Settings::open(path.clone(), None);
        settings.record(Shortcut::Panel, "Super+Space".into());
        assert!(
            settings
                .shortcut_error
                .as_deref()
                .unwrap()
                .contains("Search apps")
        );
        settings.record(Shortcut::Panel, "Mod+Shift+A".into());
        assert_eq!(settings.shortcut_error, None);
        let (config, _) = Config::load(&path);
        assert_eq!(config.shortcuts.panel, "Mod+Shift+A");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn comments_inside_strings_are_not_comments() {
        assert_eq!(
            split_comment(r##" "#86bdf0" # a color"##),
            (r##" "#86bdf0" "##, "# a color")
        );
        assert_eq!(split_comment(" 10"), (" 10", ""));
    }
}

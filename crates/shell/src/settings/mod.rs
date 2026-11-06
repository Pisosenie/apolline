mod appearance;
mod bar_layout;
mod catalog;
mod colors;
mod desk_widgets;
mod displays;
mod dock;
mod file;
mod find;
mod frame;
mod handle;
mod input;
mod launcher_finds;
mod lock;
mod looks;
mod modes;
mod notifications;
mod panel_layout;
mod pickers;
mod plugins;
#[cfg(test)]
mod tests;
mod tweaks;
mod welcome;
mod widgets;
mod windows;

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::lit::button;
use crate::style::text;
use apolline_config::{
    Accent, Appear, Blur, Calendar, Center, Config, Corner, Diagnostic, Edge, Editor, ModeSettings,
    Motion, Notifications, Piece, Rule, Shortcut, Switch, TEMPLATES, Theme as Look, TrayItem,
    Trigger, Width, Workspaces, auto_accent,
};
use apolline_theme::{ACCENTS, Palette, Rgba, neutral};
use iced::keyboard;
use iced::widget::text::Span;
use iced::widget::{
    Column, Row, Space,
    button::{self},
    container, image, pick_list, responsive, rich_text, row, scrollable, slider, space, span,
    stack, text_input, toggler,
};
use iced::{
    Alignment, Background, Border, ContentFit, Element, Fill, Font, Length, Shadow, Subscription,
    Task, Theme, Vector, window,
};

use crate::apps::{self, App};
use crate::board;
use crate::controls::Controls;
use crate::events::{self, Heard};
use crate::fragment::{self, Fragment};
use crate::icon::{Icon, MODE_ICONS, icon};
use crate::status::Status;
use crate::{bar, caldav, fonts, keys, notify, secrets, shot, style, themes, wallpaper};
use appearance::*;
use bar_layout::*;
use displays::*;
use dock::DockSet;
use file::*;
use input::*;
use modes::*;
use pickers::*;
use widgets::*;

const HISTORY: usize = 100;
const SYSTEM_CURSOR: &str = "System's";
const CLOCK_SPOTS: [(&str, &str); 8] = [
    ("off", "Off"),
    ("center", "In the middle"),
    ("top-left", "Top left"),
    ("top", "Top"),
    ("top-right", "Top right"),
    ("bottom-left", "Bottom left"),
    ("bottom", "Bottom"),
    ("bottom-right", "Bottom right"),
];
const BUILT_IN_FONT: &str = "Geist, built in";
const SAME_FONT: &str = "The desktop's font";
const WALLS: usize = 11;
const PER_ROW: usize = 4;
const WIDE: f32 = 1120.0;
const NARROW: f32 = 860.0;

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
    .theme(|_: &Settings| style::theme())
    .default_font(Font::with_name(crate::style::family()))
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
    Displays,
    Bar,
    Lock,
    Notifications,
    Keyboard,
    Pointer,
    Shortcuts,
    Plugins,
    About,
    Welcome,
}

impl Page {
    const ALL: [Page; 13] = [
        Page::Modes,
        Page::Themes,
        Page::Appearance,
        Page::Windows,
        Page::Displays,
        Page::Bar,
        Page::Lock,
        Page::Notifications,
        Page::Keyboard,
        Page::Pointer,
        Page::Shortcuts,
        Page::Plugins,
        Page::About,
    ];

    fn key(self) -> &'static str {
        match self {
            Page::Modes => "modes",
            Page::Themes => "themes",
            Page::Appearance => "appearance",
            Page::Windows => "windows",
            Page::Displays => "displays",
            Page::Bar => "bar",
            Page::Lock => "lock",
            Page::Notifications => "notifications",
            Page::Keyboard => "keyboard",
            Page::Pointer => "pointer",
            Page::Shortcuts => "shortcuts",
            Page::Plugins => "plugins",
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
            Page::Displays => "Displays",
            Page::Bar => "Bar",
            Page::Lock => "Lock screen",
            Page::Notifications => "Notifications",
            Page::Keyboard => "Keyboard",
            Page::Pointer => "Mouse & touchpad",
            Page::Shortcuts => "Shortcuts",
            Page::Plugins => "Plugins",
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
            Page::Displays => "Resolution, refresh rate, scale and where each screen sits.",
            Page::Bar => "Where the bar and the dock sit and what they show.",
            Page::Lock => "What the screen shows while you're away, and when it locks.",
            Page::Notifications => "When popups show up and how long they stay.",
            Page::Keyboard => {
                "The language, keyboard layouts, how to switch them and how keys repeat."
            }
            Page::Pointer => "Tapping, scrolling and pointer speed.",
            Page::Shortcuts => "Keys that open Apolline, and what they replace.",
            Page::Plugins => {
                "Bar pieces, tiles, widgets and searches others wrote, in any language."
            }
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
            Page::Displays => Icon::Monitor,
            Page::Bar => Icon::PanelTop,
            Page::Lock => Icon::Lock,
            Page::Notifications => Icon::Bell,
            Page::Keyboard => Icon::Keyboard,
            Page::Pointer => Icon::Pointer,
            Page::Shortcuts => Icon::Command,
            Page::Plugins => Icon::Zap,
            Page::About => Icon::Info,
            Page::Welcome => Icon::Sparkles,
        }
    }

    fn sections(self) -> &'static [&'static str] {
        match self {
            Page::Modes | Page::About | Page::Welcome => &[],
            Page::Themes => &[
                "desktop", "colors", "layout", "windows", "bar", "lock", "style",
            ],
            Page::Appearance => &["desktop", "colors", "layout", "apps", "style", "launcher"],
            Page::Windows => &["windows"],
            Page::Displays => &["night", "bar.skip", "bar.screens"],
            Page::Bar => &["bar", "style.bar", "dock", "style.dock", "clock", "panel"],
            Page::Lock => &["lock", "idle"],
            Page::Notifications => &["notifications"],
            Page::Keyboard => &["keyboard"],
            Page::Pointer => &["touchpad", "mouse"],
            Page::Shortcuts => &["shortcuts"],
            Page::Plugins => &["plugins"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout(&'static str, &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleField {
    Name,
    Every,
}

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
    Palette(String),
    DesktopClock(String),
    DesktopWidget(String, bool),
    WallpaperElsewhere(bool),
    WallpaperDraft(String),
    UseWallpaper,
    AllWallpapers,
    ColorApps(bool),
    Cursor(String),
    Icons(String),
    FontFamily(String),
    GtkTheme(String),
    CursorSize(u16),
    DisplayMode(String, String),
    DisplayScale(String, u16),
    DisplayTurn(String, &'static str),
    DisplayVrr(String, bool),
    DisplayBar(String, bool),
    DisplayX(String, String),
    DisplayY(String, String),
    DisplayMove(String),
    DisplayPlace(String, i64, i64),
    KeepDisplays,
    RevertDisplays,
    ForMonitor(Option<String>),
    Rotate(u32),
    Shuffle(bool),
    Apps(Vec<App>),
    PinQuery(String),
    AppDraft(&'static str, String),
    AppListAdd(&'static str),
    AppListRemove(&'static str, usize),
    ModuleDraft(String),
    AddModule,
    RemoveModule(usize),
    ModuleField(usize, ModuleField, String),
    ModuleFieldDone,
    WeatherDraft(String),
    SetWeather,
    CalendarDraft(String),
    AddCalendar,
    RemoveCalendar(usize),
    CalDavUrl(String),
    CalDavUser(String),
    CalDavPassword(String),
    AddCalDav,
    CalDavAdded(Result<Calendar, String>),
    Forgotten(Result<(), String>),
    Keyring(Result<(), String>),
    PinApp(String),
    UnpinApp(usize),
    RemoveFolder(usize),
    Notifications(usize, Notifications),
    Performance(bool),
    KeepAwake(bool),
    Replay(u16),
    ModeTheme(Option<String>),
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
    Stat(&'static str, bool),
    NightSchedule(bool),
    NightDraft(String),
    SetNight,
    NightWarmth(u16),
    Style(Part, Knob, u16),
    SurfaceFont(Part, String),
    LauncherGrid(bool),
    CardsCorner(Corner),
    Radius(u16),
    BorderStyle(u8),
    Shadows(bool),
    BorderWidth(u16),
    Workspaces(Workspaces),
    Seconds(bool),
    BarEdge(Edge),
    // A piece of the bar, or an icon of the quick settings button, picked up and let go.
    BarHold(Option<board::Held>),
    TrayHold(Option<board::Held>),
    BoardMove(iced::Point),
    BarDrop(iced::Point, f32),
    TilesHold(Option<board::Held>),
    TilesDrop(iced::Point, f32),
    SlidersHold(Option<board::Held>),
    SlidersDrop(iced::Point, f32),
    PartsHold(Option<board::Held>),
    PartsDrop(iced::Point, f32),
    PanelLyrics(bool),
    TrayDrop(iced::Point, f32),
    BarFloating(bool),
    BarAutohide(bool),
    BarShape(apolline_config::Shape),
    BarOpacity(u16),
    BarBlur(bool),
    Dock(DockSet),
    Date(bool),
    BatteryPercent(bool),
    DoNotDisturb(bool),
    PopupSeconds(u16),
    Cards(u16),
    CardWidth(u16),
    SoundKind(&'static str),
    SoundDraft(String),
    SoundSet,
    QuietDraft(String),
    QuietSet,
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
    LockWeather(bool),
    KeepHistory(bool),
    LockNext(bool),
    LockCount(bool),
    LockCentered(bool),
    LockBlur(bool),
    LockFingerprint(bool),
    LockNow,
    Language(&'static str),
    Find(String),
    ColorDraft(&'static str, String),
    ClockDraft(&'static str, String),
    ClockSet(&'static str),
    LauncherFinds(&'static str, bool),
    WebDraft(String),
    WebSet,
    Terminal(String),
    ColorSet(&'static str),
    ColorReset(&'static str),
    FoundSetting(Page, String),
    Machine(Vec<crate::doctor::Line>),
    CheckMachine,
    Browse,
    Catalog(Result<Vec<crate::catalog::Entry>, String>),
    CatalogPicture(String, Option<image::Handle>),
    CatalogInstall(usize),
    CatalogInstalled(Result<String, String>),
    GreetPreview,
    GreetInstall,
    GreetDone(Result<(), String>),
    IdleLock(u16),
    IdleScreen(u16),
    IdleSuspend(u16),
    BatteryLock(u16),
    BatteryScreen(u16),
    BatterySuspend(u16),
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
    // Wallpaper Engine's scenes and videos, found off the UI thread.
    LiveWalls(Vec<String>),
    Undo,
    Redo,
    Tick,
    Shot(window::Screenshot),
    Plugin(plugins::Ask),
    Noop,
}

// The password stays here only until it's in the keyring.
#[derive(Default)]
struct CalDavForm {
    url: String,
    user: String,
    password: String,
    checking: bool,
    error: Option<String>,
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
    // None until asked; an empty error while it's on its way.
    catalog: Option<Result<Vec<crate::catalog::Entry>, String>>,
    catalog_pictures: std::collections::HashMap<String, image::Handle>,
    catalog_busy: Option<String>,
    // The search over every page, and the row it last led to.
    query: String,
    spot: Option<(Page, String)>,
    // A color being typed, by role, until Enter.
    color_drafts: std::collections::HashMap<&'static str, String>,
    clock_drafts: std::collections::HashMap<&'static str, String>,
    web_draft: Option<String>,
    sound_draft: Option<String>,
    quiet_draft: Option<String>,
    // What `apolline doctor` says, once the About page has asked.
    machine: Option<Vec<crate::doctor::Line>>,
    theme_name: String,
    theme_path: String,
    theme_note: Option<Result<String, String>>,
    draft: String,
    wallpaper_draft: String,
    // What's being dragged on the bar's layout, and on the quick settings button's.
    bar_held: Option<board::Held>,
    tray_held: Option<board::Held>,
    // The same for what quick settings shows.
    tiles_held: Option<board::Held>,
    sliders_held: Option<board::Held>,
    parts_held: Option<board::Held>,
    library: Vec<String>,
    // The library's live wallpapers, marked in the gallery.
    live_walls: std::collections::HashSet<String>,
    apps: Vec<App>,
    pin_query: String,
    app_draft: (&'static str, String),
    module_draft: String,
    // A module's name or rerun as it's typed, written on Enter.
    module_field: Option<(usize, ModuleField, String)>,
    weather_draft: Option<String>,
    calendar_draft: String,
    caldav: CalDavForm,
    keyring: Option<Result<(), String>>,
    heard: std::collections::HashMap<u64, Heard>,
    night_draft: Option<String>,
    live: crate::control::Shared,
    monitors: Vec<String>,
    monitor: Option<String>,
    outputs: Vec<apolline_niri::Output>,
    cursors: Vec<String>,
    icon_themes: Vec<String>,
    families: Vec<String>,
    gtk_themes: Vec<String>,
    place: (String, String, String),
    revert: Option<(std::time::Instant, String)>,
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
    greet_note: Option<Result<(), String>>,
    error: Option<String>,
    screenshot: Option<PathBuf>,
    shooting: bool,
    // The Plugins page.
    shelf: plugins::Shelf,
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
            catalog: None,
            catalog_pictures: std::collections::HashMap::new(),
            catalog_busy: None,
            query: String::new(),
            spot: None,
            color_drafts: std::collections::HashMap::new(),
            clock_drafts: std::collections::HashMap::new(),
            web_draft: None,
            sound_draft: None,
            quiet_draft: None,
            machine: None,
            theme_name: String::new(),
            theme_path: String::new(),
            theme_note: None,
            config,
            diagnostics,
            page: Page::Modes,
            draft: String::new(),
            wallpaper_draft: String::new(),
            bar_held: None,
            tray_held: None,
            tiles_held: None,
            sliders_held: None,
            parts_held: None,
            library: Vec::new(),
            live_walls: std::collections::HashSet::new(),
            apps: Vec::new(),
            pin_query: String::new(),
            app_draft: ("", String::new()),
            module_draft: String::new(),
            module_field: None,
            weather_draft: None,
            calendar_draft: String::new(),
            caldav: CalDavForm::default(),
            keyring: None,
            heard: events::heard(),
            night_draft: None,
            live: crate::control::shared(),
            monitors: wallpaper::monitors(),
            monitor: None,
            outputs: if crate::niri::connected() {
                crate::niri::outputs()
            } else {
                sample_outputs()
            },
            cursors: crate::paint::cursors(),
            icon_themes: apps::icon_themes(),
            families: crate::paint::font_families(),
            gtk_themes: crate::paint::gtk_themes(),
            place: (String::new(), String::new(), String::new()),
            revert: None,
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
            greet_note: None,
            error: None,
            screenshot,
            shooting: false,
            shelf: plugins::Shelf::new(),
        };
        let mut settings = settings;
        let custom = settings.custom_thumb();
        let library = settings.scan_library();
        let apps = Task::perform(
            apps::scan(settings.config.desktop.icons.clone()),
            Message::Apps,
        );
        (
            settings,
            Task::batch([
                Task::perform(thumbnails(), Message::Thumbs),
                custom,
                library,
                apps,
                Task::perform(secrets::here(), Message::Keyring),
            ]),
        )
    }

    // Every change shows at once, the colors too.
    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        let scheme = self.config.scheme(None);
        if scheme != apolline_theme::scheme() {
            apolline_theme::set_scheme(scheme);
        }
        crate::i18n::choose(self.config.language.as_deref());
        task
    }

    fn record(&mut self, shortcut: Shortcut, combo: String) {
        let taken = Shortcut::ALL.into_iter().find(|&other| {
            other != shortcut && keys::same(self.config.shortcuts.get(other), &combo)
        });
        match taken {
            Some(other) => {
                self.shortcut_error = Some(crate::i18n::fill(
                    "{combo} already opens “{action}”",
                    &[
                        ("combo", &combo),
                        ("action", &crate::i18n::t(fragment::action(other).0)),
                    ],
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
        // The live ones found last time stay listed, in their order, until the new look is in.
        let known: Vec<String> = self
            .library
            .iter()
            .filter(|name| self.live_walls.contains(*name))
            .cloned()
            .collect();
        self.library = wallpaper::library(&self.config.desktop.folders);
        self.library.extend(known);
        let live = Task::perform(
            async {
                tokio::task::spawn_blocking(crate::live::projects)
                    .await
                    .unwrap_or_default()
            },
            Message::LiveWalls,
        );
        Task::batch([self.queue_thumbs(), live])
    }

    fn queue_thumbs(&mut self) -> Task<Message> {
        // Themes from folders bring their own wallpapers, which the gallery shows too.
        let mut wanted = self.library.clone();
        for (theme, _) in &self.themes {
            if !wanted.contains(&theme.wallpaper) {
                wanted.push(theme.wallpaper.clone());
            }
        }
        self.queue = wanted
            .into_iter()
            .rev()
            .filter(|name| self.thumb(name).is_none())
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

    fn add_caldav(&mut self) -> Task<Message> {
        let form = &mut self.caldav;
        let (url, user) = (form.url.trim().to_owned(), form.user.trim().to_owned());
        let calendar = Calendar::CalDav {
            url: url.clone(),
            user: user.clone(),
        };
        form.error = match caldav::secure(&url) {
            Err(error) => Some(error),
            Ok(()) if self.config.calendars.contains(&calendar) => {
                Some("That calendar is here already".into())
            }
            Ok(()) if user.is_empty() || form.password.is_empty() || form.checking => None,
            Ok(()) => {
                form.checking = true;
                let password = form.password.clone();
                return Task::perform(caldav::add(url, user, password), Message::CalDavAdded);
            }
        };
        Task::none()
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
}

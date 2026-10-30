use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use apolline_config::{
    Accent, Config, Diagnostic, Editor, Notifications, Shortcut, Switch, Workspaces,
};
use apolline_theme::{Mode, Palette, Rgba, font, neutral};
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

use crate::controls::Controls;
use crate::fragment::{self, Fragment};
use crate::icon::{Icon, icon};
use crate::status::Status;
use crate::{bar, fonts, keys, notify, shot, style, wallpaper};

const SWATCHES: [u32; 6] = [0x86bdf0, 0xff6a3d, 0xf2b35c, 0xc084fc, 0x5dd39e, 0xf472b6];
const WINDOW: Rgba = Rgba::hex(0x111316);
const SIDEBAR: Rgba = Rgba::hex(0x16181c);
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
    Appearance,
    Bar,
    Notifications,
    Keyboard,
    Pointer,
    Shortcuts,
    About,
}

impl Page {
    const ALL: [Page; 8] = [
        Page::Modes,
        Page::Appearance,
        Page::Bar,
        Page::Notifications,
        Page::Keyboard,
        Page::Pointer,
        Page::Shortcuts,
        Page::About,
    ];

    fn key(self) -> &'static str {
        match self {
            Page::Modes => "modes",
            Page::Appearance => "appearance",
            Page::Bar => "bar",
            Page::Notifications => "notifications",
            Page::Keyboard => "keyboard",
            Page::Pointer => "pointer",
            Page::Shortcuts => "shortcuts",
            Page::About => "about",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Page::Modes => "Modes",
            Page::Appearance => "Appearance",
            Page::Bar => "Bar",
            Page::Notifications => "Notifications",
            Page::Keyboard => "Keyboard",
            Page::Pointer => "Mouse & touchpad",
            Page::Shortcuts => "Shortcuts",
            Page::About => "About",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Page::Modes => "One key changes wallpaper, apps, notifications and performance.",
            Page::Appearance => "Gaps, corners, borders and shadows of every window.",
            Page::Bar => "What the bar at the top shows.",
            Page::Notifications => "When popups show up and how long they stay.",
            Page::Keyboard => "Layouts, how to switch them and how keys repeat.",
            Page::Pointer => "Tapping, scrolling and pointer speed.",
            Page::Shortcuts => "Keys that open Apolline, and what they replace.",
            Page::About => "Version, setup and a health check of your config.",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Modes => Icon::Layers,
            Page::Appearance => Icon::Palette,
            Page::Bar => Icon::PanelTop,
            Page::Notifications => Icon::Bell,
            Page::Keyboard => Icon::Keyboard,
            Page::Pointer => Icon::Pointer,
            Page::Shortcuts => Icon::Command,
            Page::About => Icon::Info,
        }
    }

    fn sections(self) -> &'static [&'static str] {
        match self {
            Page::Modes | Page::About => &[],
            Page::Appearance => &["layout"],
            Page::Bar => &["bar"],
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
    Mode(Mode),
    Wallpaper(String),
    WallpaperDraft(String),
    UseWallpaper,
    Accent(Accent),
    Notifications(Mode, Notifications),
    Draft(String),
    Add,
    Remove(usize),
    Gaps(u16),
    Radius(u16),
    Borders(bool),
    Shadows(bool),
    Workspaces(Workspaces),
    Seconds(bool),
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
    Setup,
    OpenFile,
    Commit,
    Thumbs(Vec<(String, image::Handle)>),
    Thumb(String, Option<image::Handle>),
    Tick,
    Shot(window::Screenshot),
    Noop,
}

struct Settings {
    path: PathBuf,
    stamp: Option<SystemTime>,
    editor: Result<Editor, String>,
    config: Config,
    diagnostics: Vec<Diagnostic>,
    page: Page,
    mode: Mode,
    draft: String,
    wallpaper_draft: String,
    typing: String,
    thumbs: Vec<(String, image::Handle)>,
    recording: Option<Shortcut>,
    shortcut_error: Option<String>,
    niri_binds: Vec<(String, String)>,
    samples: [Status; 3],
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
        if let Some(page) = page.and_then(|p| Page::ALL.into_iter().find(|page| page.key() == p)) {
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
            editor,
            mode: config.mode,
            config,
            diagnostics,
            page: Page::Modes,
            draft: String::new(),
            wallpaper_draft: String::new(),
            typing: String::new(),
            thumbs: Vec::new(),
            recording: None,
            shortcut_error: None,
            niri_binds: read_niri_binds(),
            samples: Mode::ALL.map(Status::sample),
            controls: Controls::sample(),
            setup_log: Vec::new(),
            tested: None,
            error: None,
            screenshot,
            shooting: false,
        };
        let custom = settings.custom_thumb();
        (
            settings,
            Task::batch([Task::perform(thumbnails(), Message::Thumbs), custom]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let mode = self.mode;
        match message {
            Message::Page(page) => {
                self.page = page;
                self.recording = None;
            }
            Message::Mode(mode) => {
                self.mode = mode;
                return self.custom_thumb();
            }
            Message::Wallpaper(name) => {
                self.edit(|e| e.set_wallpaper(mode, &name));
                return self.custom_thumb();
            }
            Message::WallpaperDraft(draft) => self.wallpaper_draft = draft,
            Message::UseWallpaper => {
                let path = self.wallpaper_draft.trim().to_owned();
                match wallpaper::resolve(&path) {
                    wallpaper::Source::File(file) if !file.exists() => {
                        self.error = Some(format!("{} does not exist", file.display()));
                    }
                    _ if path.is_empty() => {}
                    _ => {
                        self.edit(|e| e.set_wallpaper(mode, &path));
                        self.wallpaper_draft.clear();
                        return self.custom_thumb();
                    }
                }
            }
            Message::Accent(accent) => self.edit(|e| e.set_accent(mode, accent)),
            Message::Notifications(mode, n) => self.edit(|e| e.set_notifications(mode, n)),
            Message::Draft(draft) => self.draft = draft,
            Message::Add => {
                let command = self.draft.trim().to_owned();
                if !command.is_empty() {
                    let mut open = self.config.mode(mode).open.clone();
                    open.push(command);
                    self.edit(|e| e.set_open(mode, &open));
                    self.draft.clear();
                }
            }
            Message::Remove(index) => {
                let mut open = self.config.mode(mode).open.clone();
                if index < open.len() {
                    open.remove(index);
                    self.edit(|e| e.set_open(mode, &open));
                }
            }
            Message::Gaps(v) => self.stage(|e| e.set("layout", "gaps", i64::from(v))),
            Message::Radius(v) => self.stage(|e| e.set("layout", "radius", i64::from(v))),
            Message::Borders(on) => self.edit(|e| e.set("layout", "borders", on)),
            Message::Shadows(on) => self.edit(|e| e.set("layout", "shadows", on)),
            Message::Workspaces(style) => {
                let value = match style {
                    Workspaces::Dots => "dots",
                    Workspaces::Numbers => "numbers",
                };
                self.edit(|e| e.set("bar", "workspaces", value));
            }
            Message::Seconds(on) => self.edit(|e| e.set("bar", "seconds", on)),
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
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(shortcut) = self.recording {
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        self.recording = None;
                    } else if let Some(combo) = keys::combo(&key, modifiers) {
                        self.record(shortcut, combo);
                    }
                }
            }
            Message::ResetShortcut(shortcut) => {
                let combo = apolline_config::Shortcuts::default()
                    .get(shortcut)
                    .to_owned();
                self.record(shortcut, combo);
            }
            Message::Setup => {
                let look = Fragment {
                    config: &self.config,
                    mode: self.config.mode,
                    outputs: &[],
                };
                self.setup_log =
                    match fragment::setup(&fragment::path(), &fragment::niri_config(), &look) {
                        Ok(done) => done,
                        Err(err) => vec![format!("could not set up: {err}")],
                    };
            }
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
            Message::Thumbs(thumbs) => self.thumbs = thumbs,
            Message::Thumb(name, Some(handle)) => {
                self.thumbs.retain(|(n, _)| *n != name);
                self.thumbs.push((name, handle));
            }
            Message::Tick => {
                if stamp(&self.path) != self.stamp {
                    self.reload();
                }
                if self.screenshot.is_some() && !self.shooting && self.thumbs.len() >= 3 {
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
            Message::Thumb(_, None) | Message::Key(_) | Message::Noop => {}
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
        let name = self.config.mode(self.mode).wallpaper.clone();
        if self.thumb(&name).is_some() || is_bundled(&name) {
            return Task::none();
        }
        let source = wallpaper::resolve(&name);
        Task::perform(wallpaper::thumbnail(source), move |thumb| {
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
            }
            Err(err) => self.error = Some(format!("could not save: {err}")),
        }
    }

    fn reload(&mut self) {
        self.stamp = stamp(&self.path);
        self.editor = Editor::load(&self.path);
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
        self.config.palette(self.mode)
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
            Page::Appearance => self.appearance(),
            Page::Bar => self.bar(),
            Page::Notifications => self.notifications(),
            Page::Keyboard => self.keyboard(),
            Page::Pointer => self.pointer(),
            Page::Shortcuts => self.shortcuts(),
            Page::About => self.about(),
        };
        let writes = match (&self.editor, page) {
            (_, Page::About) => None,
            (Ok(editor), Page::Modes) => Some(editor.section(self.mode)),
            (Ok(editor), page) => Some(editor.sections(page.sections())),
            (Err(_), _) => None,
        };
        let header = column![
            text(page.title()).size(26).font(style::semibold()),
            text(page.subtitle())
                .size(14)
                .color(style::color(style::FAINT)),
        ]
        .spacing(4);
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
            container(icon(Icon::of(self.config.mode), 16.0, palette.on_accent))
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
        let current = self.config.mode(self.mode);
        let palette = self.palette();

        let tabs = segmented(
            Mode::ALL.map(|mode| (mode, mode.name(), Some(Icon::of(mode)))),
            self.mode,
            Message::Mode,
        );

        let preview = self.preview(self.mode, 180.0, {
            let chip = container(
                row![
                    icon(Icon::of(self.mode), 12.0, palette.accent),
                    text(self.mode.name())
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

        let mut walls = Row::new().spacing(12);
        for (name, _) in wallpaper::bundled() {
            let Some(handle) = self.thumb(name) else {
                continue;
            };
            let on = current.wallpaper == *name;
            let ring = if on {
                palette.accent
            } else {
                neutral::HAIRLINE
            };
            walls = walls.push(
                button(
                    image(handle.clone())
                        .width(Fill)
                        .height(88)
                        .content_fit(ContentFit::Cover)
                        .border_radius(9.0),
                )
                .width(Fill)
                .padding(3)
                .style(move |_: &Theme, _| button::Style {
                    background: None,
                    border: Border {
                        color: style::color(ring),
                        width: if on { 2.0 } else { 1.0 },
                        radius: 12.0.into(),
                    },
                    ..button::Style::default()
                })
                .on_press(Message::Wallpaper((*name).to_owned())),
            );
        }
        let own = row![
            text_input("~/Pictures/my-wallpaper.jpg", &self.wallpaper_draft)
                .on_input(Message::WallpaperDraft)
                .on_submit(Message::UseWallpaper)
                .padding([8, 12])
                .size(13)
                .style(style::field(palette)),
            button(text("Use").size(13))
                .padding([8, 16])
                .style(style::secondary)
                .on_press(Message::UseWallpaper),
        ]
        .spacing(8);
        let mut wallpaper = column![walls, own].spacing(14);
        if !is_bundled(&current.wallpaper) {
            wallpaper = wallpaper.push(
                row![
                    icon(Icon::Image, 14.0, style::FAINT),
                    text(format!("Using your file {}", current.wallpaper))
                        .size(12)
                        .color(style::color(style::FAINT)),
                ]
                .spacing(8),
            );
        }

        let mut swatches = Row::new().spacing(10).align_y(Alignment::Center).push(
            button(text("Auto").size(12))
                .padding([6, 14])
                .style(style::choice(current.accent == Accent::Auto))
                .on_press(Message::Accent(Accent::Auto)),
        );
        for hex in SWATCHES {
            let color = Rgba::hex(hex);
            let on = current.accent == Accent::Fixed(color);
            swatches = swatches.push(
                button(
                    container(if on {
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
                        width: if on || status == button::Status::Hovered {
                            2.0
                        } else {
                            0.0
                        },
                        radius: style::PILL.into(),
                    },
                    ..button::Style::default()
                })
                .on_press(Message::Accent(Accent::Fixed(color))),
            );
        }
        let (badge, tint) = if readable(palette) {
            ("Readable on any wallpaper", style::GOOD)
        } else {
            ("Low contrast", style::WARN)
        };
        swatches = swatches
            .push(space::horizontal())
            .push(text(badge).size(12).color(style::color(tint)));

        let mut apps = Column::new().spacing(8);
        for (index, command) in current.open.iter().enumerate() {
            apps = apps.push(
                row![
                    icon(Icon::AppWindow, 15.0, style::FAINT),
                    text(command.as_str()).font(style::mono()).size(13),
                    space::horizontal(),
                    button(icon(Icon::X, 14.0, style::FAINT))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::Remove(index)),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        if current.open.is_empty() {
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

        let mut form = column![
            tabs,
            preview,
            group("Wallpaper", vec![padded(wallpaper)]),
            group(
                "Accent",
                vec![padded(
                    column![
                        swatches,
                        text("Auto picks a color that matches the wallpaper.")
                            .size(12)
                            .color(style::color(style::FAINT))
                    ]
                    .spacing(10)
                )]
            ),
            group(
                "Notifications",
                vec![setting(
                    "Popups",
                    "Held ones wait behind the bell on the bar",
                    rules(self.mode, current.notifications),
                )]
            ),
            group("Open when this mode starts", vec![padded(apps)]),
        ]
        .spacing(22);
        if self.mode == Mode::Play {
            form = form.push(group(
                "Performance",
                vec![
                    fact(Icon::Zap, "Power profile switches to performance"),
                    fact(Icon::Monitor, "Games drive the refresh rate (VRR)"),
                    fact(Icon::Layers, "Animations and window shadows are off"),
                ],
            ));
        }
        form.into()
    }

    fn appearance(&self) -> Element<'_, Message> {
        let layout = self.config.layout;
        let palette = self.palette();
        let preview = self.preview(self.mode, 200.0, {
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
                        "A thin line in the accent color around the focused window",
                        switch(layout.borders, Message::Borders, palette),
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

    fn bar(&self) -> Element<'_, Message> {
        let look = self.config.bar;
        let palette = self.palette();
        let status = &self.samples[self.mode as usize];
        let preview = self.preview(self.mode, 96.0, {
            let bar = bar::view(bar::Bar {
                mode: self.mode,
                palette,
                active: status.active,
                status: status.clone(),
                battery: status.battery,
                track: None,
                controls: &self.controls,
                held: 2,
                do_not_disturb: self.config.alerts.do_not_disturb,
                now: chrono::Local::now(),
                look,
                panel_open: false,
            })
            .map(|_| Message::Noop);
            container(bar).padding(10).into()
        });
        column![
            preview,
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
        let per_mode: Vec<Element<'_, Message>> = Mode::ALL
            .map(|mode| {
                setting_with_icon(
                    Icon::of(mode),
                    mode.name(),
                    "",
                    rules(mode, self.config.mode(mode).notifications),
                )
            })
            .into();
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

    fn about(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let niri_running = std::env::var_os("NIRI_SOCKET").is_some();
        let included = std::fs::read_to_string(fragment::niri_config())
            .is_ok_and(|text| text.contains(&fragment::path().display().to_string()));

        let hero = row![
            container(icon(Icon::of(self.config.mode), 28.0, palette.on_accent))
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
        mode: Mode,
        height: f32,
        overlay: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let name = &self.config.mode(mode).wallpaper;
        let picture: Element<'a, Message> = match self.thumb(name) {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(height)
                .content_fit(ContentFit::Cover)
                .border_radius(14.0)
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

fn rules<'a>(mode: Mode, current: Notifications) -> Element<'a, Message> {
    let on = move |n| Message::Notifications(mode, n);
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
    fn clicks_end_up_in_the_file_and_comments_survive() {
        let dir = std::env::temp_dir().join(format!("apolline-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, include_str!("../../../assets/config.toml")).unwrap();

        let (mut settings, _) = Settings::open(path.clone(), None);
        let _ = settings.update(Message::Mode(Mode::Evening));
        let _ = settings.update(Message::Accent(Accent::Fixed(Rgba::hex(0xf472b6))));
        let _ = settings.update(Message::Notifications(Mode::Evening, Notifications::Hold));
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
        let evening = config.mode(Mode::Evening);
        assert_eq!(evening.accent, Accent::Fixed(Rgba::hex(0xf472b6)));
        assert_eq!(evening.notifications, Notifications::Hold);
        assert_eq!(evening.open, ["foot"]);
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

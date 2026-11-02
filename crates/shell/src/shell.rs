use apolline_config::{Config, Edge, Editor, ModeSettings, Notifications, Trigger};
use apolline_theme::{metrics, neutral};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{column, container, image, space, stack, text};
use iced::{Alignment, ContentFit, Element, Fill, Padding, Task, window};

use crate::apps::{self, App};
use crate::bar::{self, Bar};
use crate::clipboard::{self, Clip};
use crate::clips::Picker;
use crate::control::{self, Command, Shared, me};
use crate::controls::{self, Change, Controls, Nudge, Osd};
use crate::fragment::{self, Fragment};
use crate::idle::{self, Stage};
use crate::keymap::{self, Entry};
use crate::launcher::{Launcher, OpenWindow, Target};
use crate::logind;
use crate::media::{self, Action, Track};
use crate::niri;
use crate::notify::{self, Closed, Notification};
use crate::osd;
use crate::panel::{self, Panel, Power};
use crate::polkit;
use crate::popup;
use crate::power::{self, Battery};
use crate::profile;
use crate::status::Status;
use crate::switcher;
use crate::{gsettings, paint};
use crate::{strip, style, wallpaper};

const FADE: Duration = Duration::from_millis(450);
const QUIET_AFTER_TOUCH: Duration = Duration::from_millis(1500);

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message(multi))]
#[derive(Clone, Debug)]
pub enum Message {
    Tick,
    Key(keyboard::Event, bool),
    Mode(Option<usize>),
    Workspace(usize, Option<u64>),
    Captured(window::Screenshot),
    Niri(apolline_niri::Event),
    NiriReply(Result<(), String>),
    Control(Command),
    Wallpaper(String, Result<image::Handle, String>),
    Battery(Option<Battery>),
    Media(Option<Track>),
    Boosted(Option<String>),
    Notify(Notification),
    Unnotify(u32),
    Dismiss(u32),
    Activate(u32),
    Invoke(u32, String),
    Mark(crate::marks::Event),
    MarksOpened(Result<std::sync::Arc<crate::marks::Marks>, String>),
    MarksDone(Result<String, String>),
    CloseMarks,
    Forget(u32),
    ClearAll,
    Idle(Stage, bool),
    KeepAwake(bool),
    Osd(Option<Osd>),
    OsdDone(u64),
    Clip(Clip),
    FocusWindow(u64),
    OpenPinned(String),
    Pin(String),
    CloseWindow(u64),
    ClipQuery(String),
    ClipCopy(Option<usize>),
    ClipPin(usize),
    ClipRemove(usize),
    KeysQuery(String),
    Overlay(Option<Overlay>),
    Opened,
    ConfigChanged,
    Picked(Option<String>),
    StopRecording,
    Module(usize, String),
    ModuleClick(usize),
    NightLight(bool),
    Frame,
    CalendarMonth(i32),
    CalendarDay(chrono::NaiveDate),
    Stats(crate::stats::Stats),
    Rates(Result<std::sync::Arc<crate::rates::Rates>, String>),
    Agenda(std::sync::Arc<crate::events::Agenda>),
    Weather(Option<crate::weather::Weather>),
    Files(Vec<std::path::PathBuf>),
    Detail(Option<panel::Detail>),
    Volume(Option<controls::Volume>),
    Profiles(Option<crate::profile::Profiles>),
    PickProfile(String),
    ProfileSet(String, Result<(), String>),
    OpenLink(String),
    HistoryGroup(String),
    Quiet(panel::Quiet),
    Tray(crate::tray::Event),
    Tunnels(Vec<crate::vpn::Tunnel>),
    Vpn(crate::vpn::Tunnel, bool),
    VpnSwitched(Result<(), String>),
    TrayPress(usize, crate::tray::Press),
    TrayMenu(usize),
    TrayMenuRead(usize, Vec<crate::tray::MenuEntry>),
    TrayChoose(i32),
    // A drag on the bar's pinned apps: what was picked up (the app, and whether it's pinned).
    StripStart(Option<(String, bool)>, f32),
    StripMove(f32),
    StripDrop(f32),
    ReplyOpen(u32),
    // The answer lost the keyboard: one with nothing typed closes, a draft stays.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ReplyUnfocused,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    ReplyRefocus,
    // Keyboard focus and pointer news from a layer window.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    LayerEvent(iced::window::Id, iced::Event),
    ReplyTyped(String),
    ReplySend,
    Wifi(crate::wifi::Seen),
    Bluetooth(crate::bluetooth::Seen),
    PickNetwork(crate::wifi::Network),
    NetPassword(String),
    JoinNetwork,
    Joined(crate::wifi::Network, Result<(), crate::wifi::Failed>),
    ForgetNetwork(crate::wifi::Network),
    Gadget(crate::bluetooth::Device),
    ForgetGadget(crate::bluetooth::Device),
    BluetoothAsk(crate::bluetooth::Ask),
    BluetoothDone(u64),
    Switched(Change, Result<(), String>),
    PinTyped(String),
    PairReply(bool),
    RadioDone(Result<(), String>),
    Thumb(String, Option<image::Handle>),
    Polkit(polkit::Request),
    PolkitGone(String),
    PolkitUser(usize),
    PolkitPassword(String),
    PolkitSubmit,
    PolkitSaid(String, polkit::Said),
    PolkitCancel,
    SettingsPage(&'static str),
    Apps(Vec<App>),
    Query(String),
    Submit,
    Run(Target),
    Controls(Controls),
    Change(Change),
    Player(Action),
    DoNotDisturb(bool),
    Power(Power),
    PowerMenu(bool),
    OpenSettings,
    Screenshot,
    LockNow,
    Sleeping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Switcher,
    Launcher,
    Panel,
    Clipboard,
    Keys,
    Polkit,
    Calendar,
    Marks,
    // An app's tray menu.
    TrayMenu,
}

struct Shown {
    output: Option<String>,
    name: String,
    from: Option<(image::Handle, Instant)>,
}

#[derive(Default)]
struct System {
    battery: Option<Battery>,
    media: Option<Track>,
}

pub struct Shell {
    config: Config,
    config_path: PathBuf,
    config_stamp: Option<SystemTime>,
    demo: bool,
    mode: Option<usize>,
    status: Status,
    niri: Option<apolline_niri::State>,
    outputs: Vec<String>,
    fired: Vec<String>,
    auto: Option<(String, Option<String>)>,
    reason: Option<Trigger>,
    rotated: Option<(String, Instant)>,
    picked: String,
    plain: Config,
    handed: Vec<(&'static str, Option<String>)>,
    started: Instant,
    system: Option<System>,
    boosted: Option<String>,
    popups: Vec<(Notification, Instant)>,
    held: Vec<Notification>,
    history: Vec<Notification>,
    awake: bool,
    osd: Option<(Osd, u64)>,
    clips: Vec<Clip>,
    picker: Picker,
    keymap: Vec<Entry>,
    keys_query: String,
    polkit: Option<polkit::Dialog>,
    painted: Option<Option<(apolline_theme::Rgba, apolline_theme::Scheme)>>,
    // What the templates were last filled with.
    templated: Option<(
        apolline_theme::Rgba,
        apolline_theme::Scheme,
        Vec<apolline_config::Template>,
    )>,
    month: i32,
    day: Option<chrono::NaiveDate>,
    stats: crate::stats::Stats,
    profiles: Option<crate::profile::Profiles>,
    agenda: std::sync::Arc<crate::events::Agenda>,
    weather: Option<crate::weather::Weather>,
    detail: Option<panel::Detail>,
    networks: Vec<crate::wifi::Network>,
    gadgets: Vec<crate::bluetooth::Device>,
    joining: Option<(crate::wifi::Network, String)>,
    asking: Option<crate::bluetooth::Ask>,
    pin: String,
    // An answer being typed on a notification card, and to which.
    replying: Option<(u32, String)>,
    // The low battery mark already warned about, until the charger goes in.
    warned: Option<u8>,
    // Apps whose past notifications are opened up in quick settings.
    opened: Vec<String>,
    // When do not disturb, set for a while, turns itself off.
    quiet_until: Option<DateTime<Local>>,
    // Calendar reminders already shown, by when, what and which alarm, so each comes once.
    reminded: Vec<(
        chrono::NaiveDateTime,
        std::sync::Arc<str>,
        chrono::TimeDelta,
    )>,
    // How many reminders so far, for their cards' ids.
    reminders: u32,
    net_note: Option<String>,
    net_busy: bool,
    apps: Vec<App>,
    overlay: Option<Overlay>,
    launcher: Launcher,
    controls: Controls,
    touched: Option<Instant>,
    power_menu: bool,
    armed: Option<Power>,
    thumbs: HashMap<String, image::Handle>,
    workspace: usize,
    wallpapers: Vec<(String, image::Handle)>,
    shown: Vec<Shown>,
    usage: crate::usage::Usage,
    tray: Vec<crate::tray::Item>,
    tunnels: Vec<crate::vpn::Tunnel>,
    // The open tray menu: whose, and its entries.
    tray_menu: Option<(crate::tray::Item, Vec<crate::tray::MenuEntry>)>,
    // An app being dragged on the bar: which, whether it was pinned, and where it is now.
    dragging: Option<(String, bool, f32)>,
    night: (bool, Option<bool>),
    module_text: Vec<(usize, String)>,
    recording: Option<crate::capture::Recording>,
    marks: Option<crate::marks::Marks>,
    shot: Option<String>,
    ticks: u64,
    published: Option<Shared>,
    now: DateTime<Local>,
}

impl Shell {
    pub fn new(mode: Option<String>, demo: bool) -> (Self, Task<Message>) {
        let config_path = apolline_config::default_path();
        let (config, diagnostics) = Config::load(&config_path);

        let mode = match mode {
            Some(name) => config.find(&name),
            None => config.start(),
        };
        let niri = niri::connected().then(apolline_niri::State::default);
        let sample = demo && niri.is_none();
        let status = if sample {
            Status::sample()
        } else {
            Status::default()
        };
        let mut shell = Self {
            config_stamp: stamp(&config_path),
            config_path,
            demo,
            mode,
            workspace: status.active,
            status,
            niri,
            outputs: Vec::new(),
            fired: Vec::new(),
            auto: None,
            reason: None,
            rotated: None,
            picked: String::new(),
            started: Instant::now(),
            system: None,
            boosted: None,
            popups: Vec::new(),
            held: Vec::new(),
            history: Vec::new(),
            awake: false,
            osd: None,
            clips: if sample {
                clipboard::sample()
            } else {
                clipboard::pinned()
            },
            picker: Picker::default(),
            keymap: Vec::new(),
            keys_query: String::new(),
            polkit: None,
            painted: None,
            templated: None,
            month: 0,
            day: None,
            stats: crate::stats::Stats::default(),
            profiles: demo.then(|| crate::profile::Profiles {
                active: "balanced".into(),
                available: ["power-saver", "balanced", "performance"]
                    .map(String::from)
                    .into(),
            }),
            agenda: std::sync::Arc::default(),
            weather: None,
            detail: None,
            networks: Vec::new(),
            gadgets: Vec::new(),
            asking: None,
            pin: String::new(),
            replying: None,
            warned: None,
            reminded: Vec::new(),
            reminders: 0,
            opened: Vec::new(),
            quiet_until: None,
            joining: None,
            net_note: None,
            net_busy: false,
            apps: Vec::new(),
            overlay: None,
            launcher: Launcher::default(),
            controls: if sample {
                Controls::sample()
            } else {
                Controls::default()
            },
            touched: None,
            power_menu: false,
            armed: None,
            thumbs: HashMap::new(),
            plain: config.clone(),
            handed: Vec::new(),
            config,
            wallpapers: Vec::new(),
            shown: Vec::new(),
            tray: Vec::new(),
            tunnels: Vec::new(),
            tray_menu: None,
            usage: if demo {
                crate::usage::Usage::default()
            } else {
                crate::usage::Usage::load()
            },
            dragging: None,
            night: (false, None),
            module_text: Vec::new(),
            recording: None,
            marks: None,
            shot: None,
            ticks: 0,
            published: None,
            now: Local::now(),
        };
        shell.report(&diagnostics);
        shell.picked = shell.config.desktop.wallpaper.clone();
        shell.dress();
        shell.night = (shell.night_scheduled(), None);
        shell.sync();
        shell.launcher.set_modes(shell.mode_names());
        shell.launcher.set_usage(shell.usage.clone());
        let welcome = if !demo && !shell.config_path.exists() {
            shell.first_run()
        } else {
            Task::none()
        };
        let load = Task::batch([
            shell.load_wallpaper(),
            shell.load_thumbs(),
            shell.read_controls(),
            Task::perform(
                apps::scan(shell.config.desktop.icons.clone()),
                Message::Apps,
            ),
            welcome,
        ]);
        (shell, load)
    }

    fn sync(&mut self) {
        let theme = self
            .current()
            .and_then(|m| m.theme.as_deref())
            .or(self.config.desktop.theme.as_deref())
            .and_then(crate::themes::niri_part);
        fragment::sync(&self.look(), theme.as_deref());
        if self.demo {
            return;
        }
        let desktop = &self.config.desktop;
        let font = desktop.font.as_ref().map(|family| format!("{family} 11"));
        let size = desktop.cursor_size.map(|size| size.to_string());
        let wanted = [
            ("gtk-theme", desktop.gtk.as_ref()),
            ("font-name", font.as_ref()),
            ("icon-theme", desktop.icons.as_ref()),
            ("cursor-theme", desktop.cursor.as_ref()),
            ("cursor-size", size.as_ref()),
        ];
        for (key, value) in wanted {
            let had = self.handed.iter().find(|(k, _)| *k == key).map(|(_, v)| v);
            if had == Some(&value.cloned()) {
                continue;
            }
            match value {
                Some(value) => gsettings::set(key, value),
                None => gsettings::restore(key),
            }
            self.handed.retain(|(k, _)| *k != key);
            self.handed.push((key, value.cloned()));
        }
        let palette = self.config.palette(self.mode);
        let scheme = self.config.scheme(self.mode);
        if !self.config.apps.colors {
            if self.painted != Some(None) {
                paint::remove();
                self.painted = Some(None);
            }
        } else if self.painted != Some(Some((palette.accent, scheme))) {
            for note in paint::apply(palette, &scheme) {
                eprintln!("apolline: {note}");
            }
            self.painted = Some(Some((palette.accent, scheme)));
        }
        let wanted = (palette.accent, scheme, self.config.templates.clone());
        if self.templated.as_ref() != Some(&wanted) {
            let (ran, problems) = crate::templates::write(&self.config.templates, &scheme, palette);
            for problem in problems {
                eprintln!("apolline: template: {problem}");
            }
            for command in ran {
                let _ = std::process::Command::new("sh")
                    .args(["-c", &command])
                    .stdin(std::process::Stdio::null())
                    .spawn();
            }
            self.templated = Some(wanted);
        }
    }

    fn first_run(&mut self) -> Task<Message> {
        let written = self
            .config_path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(
                    &self.config_path,
                    include_str!("../../../assets/config.toml"),
                )
            });
        if let Err(err) = written {
            eprintln!(
                "apolline: cannot write {}: {err}",
                self.config_path.display()
            );
            return Task::none();
        }
        self.config_stamp = stamp(&self.config_path);
        self.spawn(&format!("{} settings --page welcome", me()))
    }

    fn reload_config(&mut self) -> Task<Message> {
        let (mut config, diagnostics) = Config::load(&self.config_path);
        let key = self.current().map(ModeSettings::key);
        if config.desktop.rotate == 0 || config.desktop.wallpaper != self.picked {
            self.rotated = None;
        }
        self.picked = config.desktop.wallpaper.clone();
        if let Some((name, _)) = &self.rotated {
            config.desktop.wallpaper = name.clone();
        }
        self.mode = key.and_then(|key| config.find(&key));
        self.plain = config;
        self.dress();
        let scheduled = self.night_scheduled();
        if scheduled != self.night.0 {
            self.night = (scheduled, None);
        }
        self.launcher.set_modes(self.mode_names());
        self.report(&diagnostics);
        self.sync();
        self.release_held();
        let wallpaper = self.load_wallpaper();
        Task::batch([wallpaper, self.load_thumbs()])
    }

    fn current(&self) -> Option<&ModeSettings> {
        self.mode.and_then(|i| self.config.modes.get(i))
    }

    fn mode_names(&self) -> Vec<(String, String)> {
        self.config
            .modes
            .iter()
            .map(|m| (m.name.clone(), m.icon.clone()))
            .collect()
    }

    fn load_thumbs(&self) -> Task<Message> {
        let mut names: Vec<String> = (0..self.config.modes.len())
            .map(|i| self.config.wallpaper(Some(i)).to_owned())
            .collect();
        names.push(self.config.desktop.wallpaper.clone());
        names.sort();
        names.dedup();
        Task::batch(
            names
                .into_iter()
                .filter(|name| !self.thumbs.contains_key(name))
                .map(|name| {
                    Task::perform(wallpaper::thumbnail(name.clone()), move |thumb| {
                        Message::Thumb(name.clone(), thumb)
                    })
                }),
        )
    }

    fn report(&mut self, diagnostics: &[apolline_config::Diagnostic]) {
        for diagnostic in diagnostics {
            eprintln!(
                "{}:{}: {diagnostic}",
                self.config_path.display(),
                diagnostic.line
            );
        }
        self.forget(notify::CONFIG_CARD);
        if let Some(card) = notify::config_card(&self.config_path, diagnostics) {
            self.popups.push((card, Instant::now()));
        }
    }

    pub fn is_ready(&self) -> bool {
        let wanted = self.wallpaper_name(None);
        self.wallpapers.iter().any(|(name, _)| name == wanted)
    }

    fn wallpaper_name(&self, output: Option<&str>) -> &str {
        if let Some(own) = self.current().and_then(|m| m.wallpaper.as_deref()) {
            return own;
        }
        output
            .and_then(|output| {
                self.config
                    .desktop
                    .outputs
                    .iter()
                    .find(|(name, _)| name == output)
            })
            .map_or(self.config.desktop.wallpaper.as_str(), |(_, picture)| {
                picture.as_str()
            })
    }

    fn wanted_wallpapers(&self) -> Vec<String> {
        let mut names: Vec<String> = std::iter::once(None)
            .chain(self.outputs.iter().map(|o| Some(o.as_str())))
            .map(|output| self.wallpaper_name(output).to_owned())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.publish();
        let scheme = self.config.scheme(self.mode);
        if scheme != apolline_theme::scheme() {
            apolline_theme::set_scheme(scheme);
        }
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.now = Local::now();
                if self.recording.as_mut().is_some_and(|r| r.quit()) {
                    self.recording = None;
                    return self.show(Osd::Note("The recording stopped: wf-recorder quit".into()));
                }
                self.ticks += 1;
                let controls = if self.overlay == Some(Overlay::Panel) {
                    self.read_controls()
                } else {
                    Task::none()
                };
                let scheduled = self.night_scheduled();
                if scheduled != self.night.0 {
                    self.night = (scheduled, None);
                }
                // Do not disturb set for a while ends on its own.
                if self.quiet_until.is_some_and(|until| until <= self.now) {
                    return Task::batch([self.update(Message::DoNotDisturb(false)), self.expire()]);
                }
                let triggers = self.triggers();
                let rotate = self.rotate();
                let remind = self.remind();
                self.publish();
                return Task::batch([self.expire(), controls, triggers, rotate, remind]);
            }
            Message::CalendarMonth(month) => {
                self.month = month;
                self.day = None;
            }
            Message::CalendarDay(day) => self.day = Some(day),
            Message::Stats(stats) => self.stats = stats,
            Message::Rates(Ok(rates)) => self.launcher.set_rates(rates),
            Message::Rates(Err(err)) => eprintln!("apolline: exchange rates: {err}"),
            Message::Agenda(agenda) => self.agenda = agenda,
            Message::Files(files) => self.launcher.set_files(files),
            Message::Detail(detail) => {
                self.detail = detail;
                self.leave_detail();
                self.net_note = None;
                if self.sample() {
                    self.networks = crate::wifi::sample();
                    self.gadgets = crate::bluetooth::sample();
                }
            }
            Message::Wifi(seen) => {
                self.controls.wifi = seen.radio;
                self.networks = seen.networks;
            }
            Message::Bluetooth(seen) => {
                self.controls.bluetooth = seen.radio;
                self.gadgets = seen.devices;
            }
            Message::PickNetwork(network) => {
                self.net_note = None;
                if network.asks_password() {
                    self.joining = Some((network, String::new()));
                    return iced::widget::operation::focus(panel::PASSWORD);
                }
                self.joining = None;
                self.net_busy = true;
                return Task::perform(crate::wifi::join(network.clone(), None), move |result| {
                    Message::Joined(network, result)
                });
            }
            Message::NetPassword(password) => {
                if let Some((_, typed)) = &mut self.joining {
                    *typed = password;
                }
            }
            Message::JoinNetwork => {
                if let Some((network, password)) = self.joining.take()
                    && !password.is_empty()
                {
                    // The row stays open, empty, while it joins.
                    self.joining = Some((network.clone(), String::new()));
                    self.net_busy = true;
                    self.net_note = None;
                    return Task::perform(
                        crate::wifi::join(network.clone(), Some(password)),
                        move |result| Message::Joined(network, result),
                    );
                }
            }
            Message::Joined(network, result) => {
                self.net_busy = false;
                self.joining = None;
                if let Err(failed) = result {
                    self.net_note = Some(failed.said);
                    // A typo, or a saved password that stopped working: ask for it again.
                    if failed.password {
                        self.joining = Some((network, String::new()));
                        return iced::widget::operation::focus(panel::PASSWORD);
                    }
                }
            }
            Message::ForgetNetwork(network) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::wifi::forget(network), Message::RadioDone);
            }
            Message::Gadget(device) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::bluetooth::toggle(device), Message::RadioDone);
            }
            Message::ForgetGadget(device) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::bluetooth::forget(device), Message::RadioDone);
            }
            Message::BluetoothAsk(ask) => {
                let typing = matches!(
                    ask.what,
                    crate::bluetooth::Asking::Pin | crate::bluetooth::Asking::Passkey
                );
                self.asking = Some(ask);
                self.pin.clear();
                if typing {
                    return iced::widget::operation::focus(panel::PIN);
                }
            }
            Message::BluetoothDone(id) => {
                if self.asking.as_ref().is_some_and(|ask| ask.is(id)) {
                    self.asking = None;
                    self.pin.clear();
                }
            }
            Message::PinTyped(pin) => self.pin = pin,
            Message::PairReply(yes) => {
                if let Some(ask) = self.asking.take() {
                    let pin = std::mem::take(&mut self.pin);
                    return Task::future(crate::bluetooth::reply(ask, yes.then_some(pin)))
                        .discard();
                }
            }
            Message::Switched(_, Ok(())) => {}
            // It didn't take: the tile goes back and says why.
            Message::Switched(change, Err(err)) => {
                let (back, name) = match change {
                    Change::Wifi(on) => (Change::Wifi(!on), "Wi-Fi"),
                    Change::Bluetooth(on) => (Change::Bluetooth(!on), "Bluetooth"),
                    _ => return Task::none(),
                };
                self.controls.apply(&back);
                return self.show(Osd::Note(format!("{name}: {err}")));
            }
            Message::RadioDone(result) => {
                self.net_busy = false;
                self.asking = None;
                self.net_note = result.err();
            }
            Message::Weather(weather) => {
                if weather.is_some() || self.config.weather.is_none() {
                    self.weather = weather;
                }
            }
            Message::NightLight(on) => self.night.1 = Some(on),
            Message::Picked(Some(hex)) => {
                let content = clipboard::Content::Text(hex.clone());
                clipboard::copy(&content);
                clipboard::remember(&mut self.clips, Clip::new(content));
                return self.show(Osd::Note(format!("Copied {hex}")));
            }
            Message::Picked(None) => {}
            Message::StopRecording => return self.stop_recording(),
            Message::Module(index, text) => {
                self.module_text.retain(|(i, _)| *i != index);
                if !text.is_empty() {
                    self.module_text.push((index, text));
                    self.module_text.sort_by_key(|(i, _)| *i);
                }
            }
            Message::ModuleClick(index) => {
                if let Some(click) = self.config.modules.get(index).and_then(|m| m.click.clone()) {
                    return self.spawn(&click);
                }
            }
            Message::Frame => {
                for shown in &mut self.shown {
                    if shown
                        .from
                        .as_ref()
                        .is_some_and(|(_, at)| at.elapsed() >= FADE)
                    {
                        shown.from = None;
                    }
                }
            }
            Message::ConfigChanged => {
                let now = stamp(&self.config_path);
                if now != self.config_stamp {
                    self.config_stamp = now;
                    let reload = self.reload_config();
                    return Task::batch([reload, self.triggers()]);
                }
            }
            Message::Key(
                keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                },
                _,
            ) if self.overlay == Some(Overlay::Marks) => {
                return self.marks_key(key, modifiers, text.as_deref());
            }
            Message::Key(keyboard::Event::KeyPressed { key, text, .. }, captured) => {
                return self.key(key, text.as_deref(), captured);
            }
            Message::Control(Command::Switcher) => return self.toggle(Overlay::Switcher),
            Message::Control(Command::Launcher) => return self.toggle(Overlay::Launcher),
            Message::Control(Command::Panel) => return self.toggle(Overlay::Panel),
            Message::Control(Command::Clipboard) => return self.toggle(Overlay::Clipboard),
            Message::Control(Command::Keys) => return self.toggle(Overlay::Keys),
            Message::KeysQuery(query) => self.keys_query = query,
            Message::FocusWindow(id) => {
                if self.niri.is_some() {
                    return niri::send(apolline_niri::Request::FocusWindow(id));
                }
                for window in &mut self.status.windows {
                    window.2 = window.0 == id;
                }
            }
            Message::OpenPinned(id) => return self.open_pinned(&id),
            Message::Tray(crate::tray::Event::Items(items)) => self.tray = items,
            Message::Tunnels(tunnels) => self.tunnels = tunnels,
            Message::Vpn(tunnel, on) => {
                self.net_note = None;
                return Task::perform(crate::vpn::switch(tunnel, on), Message::VpnSwitched);
            }
            Message::VpnSwitched(Err(err)) => self.net_note = Some(err),
            Message::VpnSwitched(Ok(())) => {}
            Message::TrayPress(index, press) => {
                if let Some(item) = self.tray.get(index).cloned() {
                    return Task::future(crate::tray::press(item, press, (0, 0))).discard();
                }
            }
            Message::TrayMenu(index) => {
                let Some(item) = self.tray.get(index).cloned() else {
                    return Task::none();
                };
                // Without a menu of its own, the right click is the app's to handle.
                if item.menu.is_none() {
                    return self.update(Message::TrayPress(index, crate::tray::Press::Secondary));
                }
                return Task::perform(crate::tray::menu(item), move |entries| {
                    Message::TrayMenuRead(index, entries)
                });
            }
            Message::TrayMenuRead(index, entries) => {
                if let Some(item) = self.tray.get(index).cloned()
                    && !entries.is_empty()
                {
                    self.tray_menu = Some((item, entries));
                    return self.set_overlay(Some(Overlay::TrayMenu));
                }
            }
            Message::TrayChoose(id) => {
                let close = self.set_overlay(None);
                if let Some((item, _)) = self.tray_menu.take() {
                    let press = crate::tray::press(item, crate::tray::Press::Menu(id), (0, 0));
                    return Task::batch([close, Task::future(press).discard()]);
                }
                return close;
            }
            Message::StripStart(picked, x) => {
                self.dragging = picked.map(|(app, was_pinned)| (app, was_pinned, x));
            }
            Message::StripMove(x) => {
                if let Some((_, _, at)) = &mut self.dragging {
                    *at = x;
                }
            }
            Message::StripDrop(x) => {
                if let Some((app, was_pinned, _)) = self.dragging.take() {
                    let pinned = strip::landing(&self.config.pinned, &app, was_pinned, x);
                    if pinned != self.config.pinned {
                        self.save(|e| e.set_list("bar", "pinned", &pinned));
                        return self.reload_config();
                    }
                }
            }
            Message::Pin(id) => {
                let mut pinned = self.config.pinned.clone();
                match pinned.iter().position(|p| *p == id) {
                    Some(index) => {
                        pinned.remove(index);
                    }
                    None => pinned.push(id),
                }
                self.save(|e| e.set_list("bar", "pinned", &pinned));
                return self.reload_config();
            }
            Message::CloseWindow(id) => {
                if self.niri.is_some() {
                    return niri::send(apolline_niri::Request::CloseWindow(id));
                }
                self.status.windows.retain(|window| window.0 != id);
            }
            Message::Clip(clip) => clipboard::remember(&mut self.clips, clip),
            Message::ClipQuery(query) => return self.picker.search(query),
            Message::ClipCopy(index) => {
                if let Some(index) = index.or_else(|| self.picker.chosen(&self.clips))
                    && let Some(clip) = self.clips.get(index)
                {
                    let content = clip.content.clone();
                    clipboard::copy(&content);
                    clipboard::remember(&mut self.clips, Clip::new(content));
                    return self.set_overlay(None);
                }
            }
            Message::ClipPin(index) => {
                if let Some(clip) = self.clips.get_mut(index) {
                    clip.pinned = !clip.pinned;
                    if !self.sample() {
                        return clipboard::keep(&self.clips);
                    }
                }
            }
            Message::ClipRemove(index) => {
                if index < self.clips.len() {
                    let removed = self.clips.remove(index);
                    let count = self.picker.visible(&self.clips).len();
                    self.picker.selected = self.picker.selected.min(count.saturating_sub(1));
                    if removed.pinned && !self.sample() {
                        return clipboard::keep(&self.clips);
                    }
                }
            }
            Message::Control(Command::Nudge(nudge)) => return self.nudge(nudge),
            Message::Osd(Some(osd)) => return self.show(osd),
            Message::OsdDone(serial) => {
                if self.osd.as_ref().is_some_and(|(_, shown)| *shown == serial) {
                    self.osd = None;
                }
            }
            Message::Overlay(overlay) => return self.set_overlay(overlay),
            Message::Opened if self.overlay == Some(Overlay::Launcher) => {
                return self.launcher.focus();
            }
            Message::Opened if self.overlay == Some(Overlay::Clipboard) => {
                return self.picker.focus();
            }
            Message::Opened if self.overlay == Some(Overlay::Keys) => {
                return iced::widget::operation::focus(keymap::SEARCH);
            }
            Message::Opened if self.overlay == Some(Overlay::Polkit) => {
                return iced::widget::operation::focus(polkit::FIELD);
            }
            Message::Polkit(request) => {
                if let Some(old) = self.polkit.replace(polkit::Dialog::new(request)) {
                    polkit::finish(&old.request.cookie, false);
                }
                self.overlay = None;
                return Task::batch([
                    self.set_overlay(Some(Overlay::Polkit)),
                    iced::widget::operation::focus(polkit::FIELD),
                    self.ask_polkit(None),
                ]);
            }
            Message::PolkitGone(cookie) => {
                if self
                    .polkit
                    .as_ref()
                    .is_some_and(|d| d.request.cookie == cookie)
                {
                    self.polkit = None;
                    return self.set_overlay(None);
                }
            }
            Message::PolkitUser(index) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.user = index;
                    dialog.error = None;
                    return self.ask_polkit(None);
                }
            }
            Message::PolkitPassword(password) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.password = password;
                }
            }
            Message::PolkitSubmit => {
                if let Some(dialog) = &mut self.polkit
                    && !dialog.busy
                    && !dialog.password.is_empty()
                {
                    let password = std::mem::take(&mut dialog.password);
                    dialog.error = None;
                    // Into the running conversation, or a new one when that one is over.
                    let sent = dialog
                        .answers
                        .as_mut()
                        .is_some_and(|answers| answers.try_send(password.clone()).is_ok());
                    if !sent {
                        return self.ask_polkit(Some(password));
                    }
                    dialog.busy = true;
                }
            }
            Message::PolkitSaid(cookie, _)
                if self
                    .polkit
                    .as_ref()
                    .is_none_or(|d| d.request.cookie != cookie) => {}
            Message::PolkitSaid(_, polkit::Said::Info(info)) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.info = Some(info);
                }
            }
            // Asked for more after an answer, a code or a key's PIN: the field is free again.
            Message::PolkitSaid(_, polkit::Said::Asks(asks)) => {
                if let Some(dialog) = &mut self.polkit {
                    let asks = asks.trim().trim_end_matches(':').trim().to_owned();
                    let plain = asks.is_empty() || asks.eq_ignore_ascii_case("password");
                    dialog.asks = (!plain).then_some(asks);
                    dialog.busy = false;
                    return iced::widget::operation::focus(polkit::FIELD);
                }
            }
            Message::PolkitSaid(_, polkit::Said::Done(Ok(()))) => {
                if let Some(dialog) = self.polkit.take() {
                    polkit::finish(&dialog.request.cookie, true);
                }
                return self.set_overlay(None);
            }
            Message::PolkitSaid(_, polkit::Said::Done(Err(error))) => {
                if let Some(dialog) = &mut self.polkit {
                    let tried = std::mem::replace(&mut dialog.busy, false);
                    dialog.error = Some(error);
                    dialog.answers = None;
                    dialog.check = None;
                    // After a wrong password the helper starts over, so a finger works again;
                    // a helper that fails by itself waits for the next try instead.
                    let again = if tried {
                        self.ask_polkit(None)
                    } else {
                        Task::none()
                    };
                    return Task::batch([again, iced::widget::operation::focus(polkit::FIELD)]);
                }
            }
            Message::PolkitCancel => return self.set_overlay(None),
            Message::Thumb(name, Some(thumb)) => {
                self.thumbs.insert(name, thumb);
            }
            Message::SettingsPage(page) => {
                let close = self.set_overlay(None);
                return Task::batch([
                    close,
                    self.spawn(&format!("{} settings --page {page}", me())),
                ]);
            }
            Message::Control(Command::Mode(name)) => match self.config.find(&name) {
                Some(index) => return self.switch(Some(index)),
                None => eprintln!("apolline: there is no mode called “{name}”"),
            },
            Message::Control(Command::NoMode) => return self.switch(None),
            Message::Mode(mode) => {
                return self.switch(mode);
            }
            Message::Control(Command::NextMode) => {
                let next = match self.mode {
                    None if !self.config.modes.is_empty() => Some(0),
                    Some(i) if i + 1 < self.config.modes.len() => Some(i + 1),
                    _ => None,
                };
                return self.switch(next);
            }
            Message::Workspace(_, Some(id)) => {
                return niri::send(apolline_niri::Request::FocusWorkspace(id));
            }
            Message::Workspace(index, None) => self.workspace = index,
            Message::Niri(apolline_niri::Event::ScreenshotCaptured { path: Some(path) }) => {
                self.popups.retain(|(n, _)| n.id != notify::SHOT_CARD);
                self.popups.push((notify::shot_card(&path), Instant::now()));
                self.shot = Some(path);
            }
            Message::Niri(event) => {
                let switched = matches!(event, apolline_niri::Event::KeyboardLayoutSwitched { .. });
                let state = self.niri.get_or_insert_default();
                state.apply(event);
                if switched && let Some(name) = state.layout().map(str::to_owned) {
                    return self.show(Osd::Layout(name));
                }
                let state = self.niri.get_or_insert_default();
                let outputs = state.outputs();
                let mut tasks = vec![];
                if outputs != self.outputs {
                    self.outputs = outputs;
                    self.sync();
                    tasks.push(self.load_wallpaper());
                }
                tasks.push(self.triggers());
                return Task::batch(tasks);
            }
            Message::NiriReply(Err(err)) => eprintln!("apolline: niri: {err}"),
            Message::Wallpaper(name, Ok(handle)) if self.wanted_wallpapers().contains(&name) => {
                self.wallpapers.retain(|(n, _)| *n != name);
                self.wallpapers.push((name, handle));
                self.show_wallpapers();
                self.sync();
            }
            Message::Wallpaper(_, Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => {
                self.system.get_or_insert_default().battery = battery;
                let low = power::low(battery);
                let warn = match (low, battery) {
                    (Some(mark), Some(battery)) if self.warned.is_none_or(|w| mark < w) => {
                        self.update(Message::Notify(notify::battery_card(mark, battery)))
                    }
                    _ => Task::none(),
                };
                // Plugged in: the warning goes, and comes back on the next way down. A reading
                // that wobbles over the mark on battery doesn't count.
                let plugged = battery.is_none_or(|b| b.charging);
                if plugged && self.warned.is_some() {
                    self.forget(notify::BATTERY_CARD);
                    self.history.retain(|n| n.id != notify::BATTERY_CARD);
                }
                self.warned = match (plugged, self.warned, low) {
                    (true, _, _) => None,
                    (false, Some(warned), Some(low)) => Some(warned.min(low)),
                    (false, warned, low) => warned.or(low),
                };
                return Task::batch([warn, self.triggers()]);
            }
            Message::Media(media) => self.system.get_or_insert_default().media = media,
            Message::Boosted(previous) if self.current().is_some_and(|m| m.performance) => {
                self.boosted = previous
            }
            Message::Boosted(Some(previous)) => {
                return Task::future(profile::restore(previous)).discard();
            }
            Message::Notify(mut notification) => {
                // A new version of a card being answered keeps what's typed, if it can still be
                // answered.
                self.popups.retain(|(n, _)| n.id != notification.id);
                self.held.retain(|n| n.id != notification.id);
                if notification.reply.is_none()
                    && self
                        .replying
                        .as_ref()
                        .is_some_and(|(id, _)| *id == notification.id)
                {
                    self.replying = None;
                }
                if notification.timeout == Some(notify::SHOWN_FOR) {
                    let seconds = u64::from(self.config.alerts.seconds);
                    notification.timeout = Some(Duration::from_secs(seconds));
                }
                self.history.retain(|n| n.id != notification.id);
                let mut dropped = Vec::new();
                if notification.id != notify::CONFIG_CARD && !notification.transient {
                    self.history.insert(0, notification.clone());
                    dropped = self.history.split_off(self.history.len().min(50));
                }
                let dropped = Task::batch(
                    dropped
                        .into_iter()
                        .filter(|n| {
                            !self.popups.iter().any(|(p, _)| p.id == n.id)
                                && !self.held.iter().any(|h| h.id == n.id)
                        })
                        .map(|n| notify::closed(n.id, Closed::Expired)),
                );
                // Into the history and nowhere else; the app isn't told it was closed,
                // since some take that as read.
                if self.config.alerts.muted(&notification.app) {
                    // Kept nowhere, so the app hears it's gone.
                    if notification.transient {
                        return Task::batch([
                            dropped,
                            notify::closed(notification.id, Closed::Expired),
                        ]);
                    }
                    return dropped;
                }
                if self.config.alerts.always(&notification.app)
                    || notify::lets_through(self.rule(), notification.urgency)
                {
                    self.popups.push((notification, Instant::now()));
                } else {
                    self.held.push(notification);
                }
                return dropped;
            }
            Message::Unnotify(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::closed(id, Closed::Requested);
            }
            Message::Forget(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::closed(id, Closed::Dismissed);
            }
            Message::ClearAll => {
                self.held.clear();
                let gone: Vec<u32> = self.history.drain(..).map(|n| n.id).collect();
                return Task::batch(
                    gone.into_iter()
                        .map(|id| notify::closed(id, Closed::Dismissed)),
                );
            }
            Message::KeepAwake(on) => self.awake = on,
            Message::Idle(stage, true) => return self.idle(stage),
            Message::Dismiss(id) => {
                self.forget(id);
                // Still in the history, where a click has to reach the app.
                if self.history.iter().any(|n| n.id == id) {
                    return Task::none();
                }
                return notify::closed(id, Closed::Dismissed);
            }
            Message::Activate(notify::SHOT_CARD) => {
                self.forget(notify::SHOT_CARD);
                return self.open_marks();
            }
            Message::Activate(id) => {
                let Some(notification) = self
                    .popups
                    .iter()
                    .map(|(n, _)| n)
                    .chain(&self.history)
                    .find(|n| n.id == id)
                    .cloned()
                else {
                    return Task::none();
                };
                if !notification.resident {
                    self.forget(id);
                    self.history.retain(|n| n.id != id);
                }
                let close = if self.overlay == Some(Overlay::Panel) {
                    self.set_overlay(None)
                } else {
                    Task::none()
                };
                let focus = self
                    .app_window(&notification)
                    .map_or_else(Task::none, |window| {
                        niri::send(apolline_niri::Request::FocusWindow(window))
                    });
                // A resident card stays: it hears the click and nothing closes.
                let told = match (notification.default_action(), notification.resident) {
                    (true, true) => notify::act(id, "default".into()),
                    (true, false) => notify::invoke(id, "default".into()),
                    (false, true) => Task::none(),
                    (false, false) => notify::closed(id, Closed::Dismissed),
                };
                return Task::batch([close, focus, told]);
            }
            Message::Quiet(choice) => {
                let now = Local::now();
                self.detail = None;
                self.quiet_until = match choice {
                    panel::Quiet::For(minutes) => {
                        Some(now + chrono::TimeDelta::minutes(i64::from(minutes)))
                    }
                    panel::Quiet::Morning => {
                        let eight = chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap_or_default();
                        let day = if now.time() < eight {
                            now.date_naive()
                        } else {
                            now.date_naive() + chrono::TimeDelta::days(1)
                        };
                        chrono::TimeZone::from_local_datetime(&Local, &day.and_time(eight))
                            .earliest()
                    }
                    panel::Quiet::Always | panel::Quiet::Off => None,
                };
                return self.update(Message::DoNotDisturb(choice != panel::Quiet::Off));
            }
            Message::HistoryGroup(app) => {
                if self.opened.contains(&app) {
                    self.opened.retain(|o| *o != app);
                } else {
                    self.opened.push(app);
                }
            }
            // A link from a calendar, so given to the opener as it is, never to a shell.
            Message::OpenLink(link) => {
                if !link.starts_with("https://") {
                    return Task::none();
                }
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let open = Task::future(async move {
                    let mut command = tokio::process::Command::new(opener);
                    command
                        .arg(link)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    if let Ok(mut child) = command.spawn() {
                        let _ = child.wait().await;
                    }
                })
                .discard();
                return Task::batch([self.set_overlay(None), open]);
            }
            Message::ReplyOpen(id) => {
                self.replying = Some((id, String::new()));
                return iced::widget::operation::focus(popup::REPLY);
            }
            Message::ReplyRefocus if self.replying.is_some() => {
                return iced::widget::operation::is_focused(popup::REPLY).then(|focused| {
                    if focused {
                        Task::none()
                    } else {
                        iced::widget::operation::focus(popup::REPLY)
                    }
                });
            }
            Message::ReplyUnfocused => {
                if self
                    .replying
                    .as_ref()
                    .is_some_and(|(_, typed)| typed.trim().is_empty())
                {
                    self.replying = None;
                }
            }
            Message::ReplyTyped(typed) => {
                if let Some((_, text)) = &mut self.replying {
                    *text = typed;
                }
            }
            Message::ReplySend => {
                if let Some((id, typed)) = self.replying.take()
                    && !typed.trim().is_empty()
                {
                    self.forget(id);
                    self.history.retain(|n| n.id != id);
                    return notify::replied(id, typed);
                }
            }
            // A calendar reminder's Join button carries its link as the key.
            Message::Invoke(id, key) if notify::is_reminder(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return self.update(Message::OpenLink(key));
            }
            Message::Invoke(id, key) => {
                let resident = self.popups.iter().any(|(n, _)| n.id == id && n.resident);
                if resident {
                    return notify::act(id, key);
                }
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::invoke(id, key);
            }
            Message::MarksOpened(Ok(marks)) => {
                // Never over a password prompt, nor over marks not saved yet.
                if self.overlay == Some(Overlay::Polkit) {
                    return Task::none();
                }
                if self.marks.as_ref().is_some_and(|m| !m.is_empty()) {
                    return self.show(Osd::Note("Save or close these marks first".into()));
                }
                self.marks = Some(std::sync::Arc::unwrap_or_clone(marks));
                return self.set_overlay(Some(Overlay::Marks));
            }
            Message::MarksOpened(Err(err)) => return self.show(Osd::Note(err)),
            Message::Mark(event) => return self.mark(event),
            Message::MarksDone(Ok(said) | Err(said)) => return self.show(Osd::Note(said)),
            Message::CloseMarks => return self.set_overlay(None),
            Message::Apps(apps) => {
                let apps = if apps.is_empty() && self.demo {
                    apps::sample()
                } else {
                    apps
                };
                self.apps = apps.clone();
                self.launcher.set_apps(apps);
            }
            Message::Query(query) => return self.search(query),
            Message::Submit => {
                if let Some(target) = self.launcher.chosen() {
                    return self.run(target);
                }
            }
            Message::Run(target) => return self.run(target),
            Message::Controls(controls) => {
                if self
                    .touched
                    .is_none_or(|at| at.elapsed() > QUIET_AFTER_TOUCH)
                {
                    self.controls = Controls {
                        wifi: self.controls.wifi.take(),
                        bluetooth: self.controls.bluetooth.take(),
                        ..controls
                    };
                }
            }
            Message::Profiles(profiles) => self.profiles = profiles,
            Message::PickProfile(profile) => {
                if let Some(profiles) = &mut self.profiles {
                    let previous = std::mem::replace(&mut profiles.active, profile.clone());
                    if !self.sample() {
                        return Task::perform(crate::profile::choose(profile), move |result| {
                            Message::ProfileSet(previous, result)
                        });
                    }
                }
            }
            Message::ProfileSet(_, Ok(())) => {}
            Message::ProfileSet(previous, Err(err)) => {
                if let Some(profiles) = &mut self.profiles {
                    profiles.active = previous;
                }
                return self.show(Osd::Note(format!("Power: {err}")));
            }
            Message::Volume(volume) => {
                if self
                    .touched
                    .is_none_or(|at| at.elapsed() > QUIET_AFTER_TOUCH)
                {
                    self.controls.volume = volume;
                }
            }
            Message::Change(change) => {
                self.controls.apply(&change);
                self.touched = Some(Instant::now());
                if !self.sample() {
                    return match change {
                        Change::Wifi(on) => Task::perform(crate::wifi::power(on), move |result| {
                            Message::Switched(change, result)
                        }),
                        Change::Bluetooth(on) => {
                            Task::perform(crate::bluetooth::power(on), move |result| {
                                Message::Switched(change, result)
                            })
                        }
                        change => Task::future(controls::change(change)).discard(),
                    };
                }
            }
            Message::Player(action) => return self.player(action),
            Message::DoNotDisturb(on) => {
                if !on {
                    self.quiet_until = None;
                }
                self.plain.alerts.do_not_disturb = on;
                self.config.alerts.do_not_disturb = on;
                if !self.demo {
                    self.save(|editor| editor.set("notifications", "do_not_disturb", on));
                }
                self.release_held();
            }
            Message::Power(power) => {
                if power.needs_second_click() && self.armed != Some(power) {
                    self.armed = Some(power);
                    return Task::none();
                }
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn(&power.command())]);
            }
            Message::PowerMenu(open) => {
                self.power_menu = open;
                self.armed = None;
            }
            Message::OpenSettings => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn(&format!("{} settings", me()))]);
            }
            Message::Screenshot => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn("niri msg action screenshot")]);
            }
            Message::LockNow => return self.lock(),
            Message::Sleeping if self.config.lock.before_sleep => return self.lock(),
            _ => {}
        }
        Task::none()
    }

    fn key(&mut self, key: Key, text: Option<&str>, captured: bool) -> Task<Message> {
        // Escape drops the answer only while it's being typed; one kept as a draft while an
        // overlay is open is the overlay's to close.
        if self.replying.is_some() && self.overlay.is_none() && key == Key::Named(Named::Escape) {
            self.replying = None;
            return Task::none();
        }
        let launcher = self.overlay == Some(Overlay::Launcher);
        if self.overlay == Some(Overlay::Clipboard) {
            return self.clipboard_key(key, text, captured);
        }
        if self.overlay == Some(Overlay::Polkit) {
            if key == Key::Named(Named::Escape) {
                return self.set_overlay(None);
            }
            return Task::none();
        }
        if self.overlay == Some(Overlay::Keys) {
            match key.as_ref() {
                Key::Named(Named::Escape) => return self.set_overlay(None),
                _ if captured => {}
                Key::Named(Named::Backspace) => {
                    self.keys_query.pop();
                }
                _ => {
                    if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                        self.keys_query.push_str(typed);
                    }
                }
            }
            return Task::none();
        }
        match key.as_ref() {
            Key::Named(Named::Escape) if self.overlay.is_some() => return self.set_overlay(None),
            Key::Named(Named::Escape) if self.demo => return iced::exit(),
            Key::Named(Named::ArrowDown) if launcher => {
                let by = if self.launcher.grid() {
                    crate::launcher::COLUMNS
                } else {
                    1
                };
                return self.launcher.step(by as isize);
            }
            Key::Named(Named::ArrowUp) if launcher => {
                let by = if self.launcher.grid() {
                    crate::launcher::COLUMNS
                } else {
                    1
                };
                return self.launcher.step(-(by as isize));
            }
            Key::Named(Named::ArrowRight) if launcher && self.launcher.grid() => {
                return self.launcher.step(1);
            }
            Key::Named(Named::ArrowLeft) if launcher && self.launcher.grid() => {
                return self.launcher.step(-1);
            }
            _ if captured => {}
            Key::Named(Named::Backspace) if launcher => {
                let mut query = self.launcher.query.clone();
                query.pop();
                return self.search(query);
            }
            Key::Named(Named::Enter) if launcher => return self.update(Message::Submit),
            _ if launcher => {
                if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                    let query = format!("{}{typed}", self.launcher.query);
                    return self.search(query);
                }
            }
            Key::Character("0") => return self.switch(None),
            Key::Character(digit @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")) => {
                let index = digit.parse::<usize>().unwrap_or(1) - 1;
                if index < self.config.modes.len() {
                    return self.switch(Some(index));
                }
            }
            Key::Character("m") if self.demo => return self.toggle(Overlay::Switcher),
            Key::Character("=" | "+") if self.demo => return self.nudge(Nudge::VolumeUp),
            Key::Character("-") if self.demo => return self.nudge(Nudge::VolumeDown),
            Key::Character("q") if self.demo => return self.toggle(Overlay::Panel),
            Key::Named(Named::Space) if self.demo => return self.toggle(Overlay::Launcher),
            Key::Character("/") if self.demo => return self.toggle(Overlay::Keys),
            Key::Named(Named::ArrowLeft) => self.workspace = self.workspace.saturating_sub(1),
            Key::Named(Named::ArrowRight) => {
                self.workspace = (self.workspace + 1).min(self.status.workspaces.saturating_sub(1));
            }
            _ => {}
        }
        Task::none()
    }

    fn clipboard_key(&mut self, key: Key, text: Option<&str>, captured: bool) -> Task<Message> {
        match key.as_ref() {
            Key::Named(Named::Escape) => self.set_overlay(None),
            Key::Named(Named::ArrowDown) => self.picker.step(1, &self.clips),
            Key::Named(Named::ArrowUp) => self.picker.step(-1, &self.clips),
            Key::Named(Named::Delete) => match self.picker.chosen(&self.clips) {
                Some(index) => self.update(Message::ClipRemove(index)),
                None => Task::none(),
            },
            _ if captured => Task::none(),
            Key::Named(Named::Enter) => self.update(Message::ClipCopy(None)),
            Key::Named(Named::Backspace) => {
                let mut query = self.picker.query.clone();
                query.pop();
                self.picker.search(query)
            }
            _ => match text.filter(|t| t.chars().all(|c| !c.is_control())) {
                Some(typed) => {
                    let query = format!("{}{typed}", self.picker.query);
                    self.picker.search(query)
                }
                None => Task::none(),
            },
        }
    }

    fn toggle(&mut self, overlay: Overlay) -> Task<Message> {
        if self.overlay == Some(overlay) {
            self.set_overlay(None)
        } else {
            self.set_overlay(Some(overlay))
        }
    }

    fn set_overlay(&mut self, overlay: Option<Overlay>) -> Task<Message> {
        if self.overlay == overlay {
            return Task::none();
        }
        if self.overlay == Some(Overlay::Polkit)
            && let Some(dialog) = self.polkit.take()
        {
            polkit::finish(&dialog.request.cookie, false);
        }
        if self.overlay == Some(Overlay::Marks) {
            self.marks = None;
        }
        if self.overlay == Some(Overlay::Panel) {
            self.leave_detail();
        }
        self.overlay = overlay;
        self.power_menu = false;
        self.armed = None;
        match overlay {
            Some(Overlay::Launcher) => Task::batch([
                {
                    self.launcher.set_windows(self.open_windows());
                    self.launcher
                        .set_look(self.config.style.launcher, self.config.style.grid);
                    self.launcher.open()
                },
                Task::perform(apps::scan(self.config.desktop.icons.clone()), Message::Apps),
                Task::perform(crate::files::index(), Message::Files),
            ]),
            Some(Overlay::Panel) => {
                self.detail = None;
                self.leave_detail();
                self.held.clear();
                self.read_controls()
            }
            Some(Overlay::Clipboard) => self.picker.open(),
            Some(Overlay::Calendar) => {
                self.month = 0;
                self.day = None;
                self.now = Local::now();
                Task::none()
            }
            Some(Overlay::Keys) => {
                let niri = std::fs::read_to_string(fragment::niri_config()).unwrap_or_default();
                self.keymap = keymap::entries(&self.config, self.mode, &niri);
                self.keys_query.clear();
                iced::widget::operation::focus(keymap::SEARCH)
            }
            _ => Task::none(),
        }
    }

    fn run(&mut self, target: Target) -> Task<Message> {
        let close = self.set_overlay(None);
        let task = match target {
            Target::App(command) => {
                self.usage.note(&command, crate::usage::now());
                self.usage.save();
                self.launcher.set_usage(self.usage.clone());
                self.spawn(&command)
            }
            Target::Run(command) => self.spawn(&command),
            Target::Web(query) => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                self.spawn(&format!(
                    "{opener} '{}'",
                    crate::launcher::web_address(&query)
                ))
            }
            Target::Window(id) => {
                return Task::batch([close, self.update(Message::FocusWindow(id))]);
            }
            Target::Open(path) => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let quoted = format!("'{}'", path.display().to_string().replace('\'', r"'\''"));
                self.spawn(&format!("{opener} {quoted}"))
            }
            Target::Capture(capture) => match capture.command() {
                Some(command) => self.spawn(command),
                None if capture == crate::capture::Capture::Color => {
                    Task::perform(crate::capture::pick_color(), Message::Picked)
                }
                None if capture == crate::capture::Capture::Mark => {
                    return Task::batch([close, self.open_marks()]);
                }
                None => return Task::batch([close, self.toggle_recording()]),
            },
            Target::Copy(text) => {
                let content = clipboard::Content::Text(text);
                clipboard::copy(&content);
                clipboard::remember(&mut self.clips, Clip::new(content));
                Task::none()
            }
            Target::Mode(mode) => self.switch(mode),
            Target::Settings => self.spawn(&format!("{} settings", me())),
            Target::Keys => {
                return Task::batch([close, self.set_overlay(Some(Overlay::Keys))]);
            }
            Target::Clipboard => {
                return Task::batch([close, self.set_overlay(Some(Overlay::Clipboard))]);
            }
            Target::Power(power) => self.spawn(&power.command()),
        };
        Task::batch([close, task])
    }

    fn pinned(&self) -> Vec<bar::Pinned> {
        let windows: Vec<(String, bool)> = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .map(|w| (w.app_id.clone().unwrap_or_default(), w.is_focused))
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .map(|(_, app, focused)| (app.clone(), *focused))
                .collect(),
        };
        self.config
            .pinned
            .iter()
            .map(|id| {
                let mine: Vec<bool> = windows
                    .iter()
                    .filter(|(app, _)| bar::app_key(&self.apps, app) == id)
                    .map(|(_, focused)| *focused)
                    .collect();
                bar::Pinned {
                    id: id.clone(),
                    running: !mine.is_empty(),
                    focused: mine.contains(&true),
                }
            })
            .collect()
    }

    fn open_pinned(&mut self, id: &str) -> Task<Message> {
        let windows: Vec<(u64, bool)> = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .filter(|w| bar::app_key(&self.apps, w.app_id.as_deref().unwrap_or_default()) == id)
                .map(|w| (w.id, w.is_focused))
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .filter(|(_, app, _)| bar::app_key(&self.apps, app) == id)
                .map(|(window, _, focused)| (*window, *focused))
                .collect(),
        };
        let next = match windows.iter().position(|(_, focused)| *focused) {
            Some(at) => windows.get(at + 1).or(windows.first()),
            None => windows.first(),
        };
        if let Some((window, _)) = next {
            return self.update(Message::FocusWindow(*window));
        }
        match apps::find(&self.apps, id) {
            Some(app) => {
                let exec = app.exec.clone();
                self.spawn(&exec)
            }
            None => Task::none(),
        }
    }

    fn open_windows(&self) -> Vec<OpenWindow> {
        match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .filter(|window| !window.is_focused)
                .map(|window| OpenWindow {
                    id: window.id,
                    title: window.title.clone().unwrap_or_default(),
                    app: window.app_id.clone().unwrap_or_default(),
                })
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .filter(|(_, _, focused)| !focused)
                .map(|(id, app, _)| OpenWindow {
                    id: *id,
                    title: app.clone(),
                    app: app.clone(),
                })
                .collect(),
        }
    }

    fn spawn(&self, command: &str) -> Task<Message> {
        if self.niri.is_some() {
            return niri::send(apolline_niri::Request::Spawn(command.to_owned()));
        }
        if self.demo && !command.ends_with(" settings") {
            println!("apolline: would run: {command}");
            return Task::none();
        }
        let line = format!("{command} &");
        Task::future(async move {
            if let Err(err) = tokio::process::Command::new("sh")
                .args(["-c", &line])
                .status()
                .await
            {
                eprintln!("apolline: {err}");
            }
        })
        .discard()
    }

    fn nudge(&mut self, nudge: Nudge) -> Task<Message> {
        if self.sample() {
            let osd = nudge.sample(&mut self.controls);
            return self.show(osd);
        }
        Task::perform(controls::nudge(nudge), Message::Osd)
    }

    fn show(&mut self, osd: Osd) -> Task<Message> {
        match &osd {
            Osd::Volume(volume) => self.controls.volume = Some(*volume),
            Osd::Brightness(level) => self.controls.brightness = Some(*level),
            _ => {}
        }
        let serial = self.osd.as_ref().map_or(0, |(_, serial)| serial + 1);
        self.osd = Some((osd, serial));
        Task::perform(tokio::time::sleep(osd::SHOWN_FOR), move |()| {
            Message::OsdDone(serial)
        })
    }

    pub fn bar_at_bottom(&self) -> bool {
        self.config.bar.position == Edge::Bottom
    }

    pub fn osd_view(&self) -> Option<Element<'_, Message>> {
        self.osd
            .as_ref()
            .map(|(osd, _)| osd::view(osd, self.config.palette(self.mode), self.config.style.osd))
    }

    fn idle(&mut self, stage: Stage) -> Task<Message> {
        match stage {
            Stage::Lock => self.lock(),
            Stage::ScreenOff => self.spawn("niri msg action power-off-monitors"),
            Stage::Suspend => self.spawn("systemctl suspend"),
        }
    }

    fn lock(&mut self) -> Task<Message> {
        self.publish();
        let close = self.set_overlay(None);
        Task::batch([close, self.spawn(&Power::Lock.command())])
    }

    fn publish(&mut self) {
        if self.demo {
            return;
        }
        let current = self.current().map(ModeSettings::key);
        let auto = self
            .auto
            .as_ref()
            .filter(|(on, _)| current.as_ref() == Some(on));
        let shared = Shared {
            mode: current.clone(),
            waiting: self.held.len() + self.popups.len(),
            reason: auto.and(self.reason.as_ref()).map(Trigger::text),
            back: auto.and_then(|(_, back)| back.clone()),
            wallpaper: self.rotated.as_ref().map(|(name, _)| name.clone()),
            weather: self.weather.as_ref().map(|w| {
                let (_, words) = crate::weather::describe(w.code);
                (w.code, format!("{:.0}° {words} · {}", w.now, w.place))
            }),
            next: self
                .agenda
                .soon(self.now.naive_local())
                .map(|(o, wait)| crate::events::when(&o.summary, wait)),
        };
        if self.published.as_ref() != Some(&shared) {
            control::publish(&shared);
            self.published = Some(shared);
        }
    }

    fn player(&mut self, action: Action) -> Task<Message> {
        if self.system.is_none() {
            if let (Some(track), Action::Toggle) = (&mut self.status.media, action) {
                track.playing = !track.playing;
            }
            return Task::none();
        }
        match self.track() {
            Some(track) if !track.player.is_empty() => {
                Task::future(media::control(track.player.clone(), action)).discard()
            }
            _ => Task::none(),
        }
    }

    // Nothing typed or asked outlives the list it was typed in.
    fn leave_detail(&mut self) {
        self.joining = None;
        self.asking = None;
        self.pin.clear();
        crate::bluetooth::refuse();
    }

    // A new conversation with polkit's helper, with a password already in when it was typed first.
    fn ask_polkit(&mut self, password: Option<String>) -> Task<Message> {
        if self.sample() {
            return Task::none();
        }
        let Some(dialog) = &mut self.polkit else {
            return Task::none();
        };
        let (mut answer, answers) = iced::futures::channel::mpsc::channel(1);
        dialog.busy = password.is_some_and(|password| answer.try_send(password).is_ok());
        dialog.asks = None;
        dialog.answers = Some(answer);
        dialog.info = None;
        let user = dialog
            .request
            .users
            .get(dialog.user)
            .cloned()
            .unwrap_or_default();
        let cookie = dialog.request.cookie.clone();
        let (talk, handle) = Task::run(
            polkit::conversation(user, cookie.clone(), answers),
            move |said| Message::PolkitSaid(cookie.clone(), said),
        )
        .abortable();
        dialog.check = Some(handle.abort_on_drop());
        talk
    }

    fn listing(&self, detail: panel::Detail) -> bool {
        self.overlay == Some(Overlay::Panel) && self.detail == Some(detail)
    }

    fn read_controls(&self) -> Task<Message> {
        if self.sample() {
            return Task::none();
        }
        Task::perform(controls::read(), Message::Controls)
    }

    fn sample(&self) -> bool {
        self.demo && self.niri.is_none()
    }

    fn save(&mut self, change: impl FnOnce(&mut Editor)) {
        let result = Editor::load(&self.config_path).and_then(|mut editor| {
            change(&mut editor);
            editor
                .save(&self.config_path)
                .map_err(|err| err.to_string())
        });
        match result {
            Ok(()) => self.config_stamp = stamp(&self.config_path),
            Err(err) => eprintln!("apolline: could not save the config: {err}"),
        }
    }

    fn rule(&self) -> Notifications {
        if self.config.alerts.do_not_disturb {
            Notifications::Hold
        } else {
            self.config.notifications(self.mode)
        }
    }

    fn rotate(&mut self) -> Task<Message> {
        let every = self.config.desktop.rotate;
        if self.demo || every == 0 {
            return Task::none();
        }
        let since = self.rotated.as_ref().map_or(self.started, |(_, at)| *at);
        if since.elapsed() < Duration::from_secs(u64::from(every) * 60) {
            return Task::none();
        }
        let library = wallpaper::library(&self.config.desktop.folders);
        let current = &self.plain.desktop.wallpaper;
        let next = if self.config.desktop.shuffle && library.len() > 1 {
            let others: Vec<&String> = library.iter().filter(|name| *name != current).collect();
            let pick = Instant::now().elapsed().as_nanos() as usize
                ^ (Local::now().timestamp_subsec_nanos() as usize);
            others
                .get(pick % others.len().max(1))
                .map(|name| (*name).clone())
        } else {
            let at = library.iter().position(|name| name == current);
            library
                .get(at.map_or(0, |i| (i + 1) % library.len().max(1)))
                .cloned()
        };
        let Some(next) = next.filter(|next| next != current) else {
            self.rotated = Some((current.clone(), Instant::now()));
            return Task::none();
        };
        self.plain.desktop.wallpaper = next.clone();
        self.dress();
        self.rotated = Some((next, Instant::now()));
        self.sync();
        self.load_wallpaper()
    }

    fn open_marks(&mut self) -> Task<Message> {
        let last = self.shot.as_ref().map(std::path::PathBuf::from);
        match last
            .filter(|file| file.is_file())
            .or_else(crate::marks::latest)
        {
            Some(file) => Task::perform(crate::marks::open(file), Message::MarksOpened),
            None => self.show(Osd::Note("No screenshot to mark yet".into())),
        }
    }

    fn search(&mut self, query: String) -> Task<Message> {
        let found = self.launcher.search(query);
        if self.launcher.wants_rates() {
            let rates = Task::perform(crate::rates::load(), Message::Rates);
            return Task::batch([found, rates]);
        }
        found
    }

    fn marks_key(
        &mut self,
        key: Key,
        modifiers: keyboard::Modifiers,
        text: Option<&str>,
    ) -> Task<Message> {
        use crate::marks::{Event, Tool};
        let typing = self.marks.as_ref().is_some_and(|m| m.typing());
        if typing && !modifiers.command() {
            let event = match key.as_ref() {
                Key::Named(Named::Escape) => Event::Drop,
                Key::Named(Named::Enter) => Event::Place,
                Key::Named(Named::Backspace) => Event::Erase,
                _ => match text.filter(|t| t.chars().all(|c| !c.is_control())) {
                    Some(typed) => Event::Typed(typed.to_owned()),
                    None => return Task::none(),
                },
            };
            return self.mark(event);
        }
        let event = match key.as_ref() {
            Key::Named(Named::Escape) => return self.set_overlay(None),
            Key::Named(Named::Enter) => Event::Both,
            Key::Character("z") if modifiers.command() => Event::Undo,
            Key::Character("c") if modifiers.command() => Event::Copy,
            Key::Character("s") if modifiers.command() => Event::Save,
            Key::Character("p") => Event::Tool(Tool::Pen),
            Key::Character("a") => Event::Tool(Tool::Arrow),
            Key::Character("b") => Event::Tool(Tool::Box),
            Key::Character("m") => Event::Tool(Tool::Marker),
            Key::Character("h") => Event::Tool(Tool::Hide),
            Key::Character("t") => Event::Tool(Tool::Text),
            Key::Character("n") => Event::Tool(Tool::Step),
            _ => return Task::none(),
        };
        self.mark(event)
    }

    fn mark(&mut self, event: crate::marks::Event) -> Task<Message> {
        use crate::marks::Event;
        let Some(marks) = self.marks.as_mut() else {
            return Task::none();
        };
        if !matches!(event, Event::Copy | Event::Save | Event::Both) {
            marks.update(event);
            return Task::none();
        }
        let marks = marks.clone();
        // niri copied the shot as it was; with parts hidden, that copy must not stay around.
        let hides = marks.hides();
        if hides && let Ok(original) = std::fs::read(&marks.file) {
            self.clips.retain(
                |clip| !matches!(&clip.content, clipboard::Content::Png(png) if **png == original),
            );
        }
        let close = if matches!(event, Event::Both) {
            self.set_overlay(None)
        } else {
            Task::none()
        };
        let finish = Task::perform(
            async move {
                let done = tokio::task::spawn_blocking(move || {
                    let png = marks.render()?;
                    let file = marks.marked();
                    let mut said = Vec::new();
                    if hides || matches!(event, Event::Copy | Event::Both) {
                        let content = clipboard::Content::Png(std::sync::Arc::new(png.clone()));
                        clipboard::copy(&content);
                        said.push("Copied".to_owned());
                    }
                    if matches!(event, Event::Save | Event::Both) {
                        crate::marks::save(&file, &png)?;
                        let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
                        said.push(format!("Saved {}", name.unwrap_or_default()));
                    }
                    Ok::<_, String>(said.join(" · "))
                })
                .await;
                done.unwrap_or_else(|err| Err(err.to_string()))
            },
            Message::MarksDone,
        );
        Task::batch([close, finish])
    }

    fn toggle_recording(&mut self) -> Task<Message> {
        if self.recording.is_some() {
            return self.stop_recording();
        }
        let output = self.niri.as_ref().and_then(|n| n.focused_output());
        match crate::capture::Recording::start(output) {
            Ok(recording) => {
                self.recording = Some(recording);
                Task::none()
            }
            Err(err) => self.show(Osd::Note(err)),
        }
    }

    fn stop_recording(&mut self) -> Task<Message> {
        let Some(recording) = self.recording.take() else {
            return Task::none();
        };
        let file = recording.stop();
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.show(Osd::Note(format!("Saved {name}")))
    }

    fn night_scheduled(&self) -> bool {
        use chrono::Timelike;
        let now = (self.now.hour() * 60 + self.now.minute()) as u16;
        self.config
            .night
            .hours
            .is_some_and(|(from, until)| Trigger::covers(from, until, now))
    }

    fn night_light(&self) -> Option<u32> {
        let on = self.night.1.unwrap_or(self.night.0);
        (on && !self.demo).then_some(self.config.night.temperature)
    }

    fn triggers(&mut self) -> Task<Message> {
        if self.demo {
            return Task::none();
        }
        let game = self.niri.as_ref().is_some_and(|state| {
            state.recent().iter().any(|w| {
                w.app_id.as_deref().is_some_and(|app| {
                    app.starts_with("steam_app_") || app == "gamescope" || app.ends_with(".exe")
                })
            })
        });
        let battery = self
            .system
            .as_ref()
            .and_then(|s| s.battery)
            .is_some_and(|b| !b.charging);
        let now = {
            use chrono::Timelike;
            (self.now.hour() * 60 + self.now.minute()) as u16
        };
        let why: Vec<Option<Trigger>> = self
            .config
            .modes
            .iter()
            .map(|mode| {
                mode.when
                    .iter()
                    .find(|trigger| match trigger {
                        Trigger::Game => game,
                        Trigger::Battery => battery,
                        Trigger::Hours { from, until } => Trigger::covers(*from, *until, now),
                        Trigger::Monitor(name) => self.outputs.contains(name),
                    })
                    .cloned()
            })
            .collect();
        let keys: Vec<String> = self.config.modes.iter().map(ModeSettings::key).collect();
        let active: Vec<String> = keys
            .iter()
            .zip(&why)
            .filter(|(_, why)| why.is_some())
            .map(|(key, _)| key.clone())
            .collect();
        let current = self.current().map(ModeSettings::key);
        let plan = plan(&active, &self.fired, current.as_deref(), self.auto.as_ref());
        let mut task = Task::none();
        if let Some(back) = plan.back {
            task = self.enter(back.and_then(|key| self.config.find(&key)));
        }
        if let Some(index) = plan.start.as_deref().and_then(|key| self.config.find(key)) {
            task = Task::batch([task, self.enter(Some(index))]);
            self.reason = why[index].clone();
        }
        self.auto = plan.auto;
        self.fired = active;
        task
    }

    fn switch(&mut self, mode: Option<usize>) -> Task<Message> {
        let close = self.set_overlay(None);
        Task::batch([close, self.enter(mode)])
    }

    fn enter(&mut self, mode: Option<usize>) -> Task<Message> {
        let fast = |mode: Option<usize>| {
            mode.and_then(|i| self.config.modes.get(i))
                .is_some_and(|m| m.performance)
        };
        let profile = match (fast(self.mode), fast(mode)) {
            (false, true) => Task::perform(profile::boost(), Message::Boosted),
            (true, false) => match self.boosted.take() {
                Some(previous) => Task::future(profile::restore(previous)).discard(),
                None => Task::none(),
            },
            _ => Task::none(),
        };
        self.mode = mode;
        self.dress();
        self.sync();
        self.release_held();
        Task::batch([self.load_wallpaper(), self.open_apps(), profile])
    }

    fn dress(&mut self) {
        let mut config = self.plain.clone();
        let theme = self
            .mode
            .and_then(|i| self.plain.modes.get(i))
            .and_then(|m| m.theme.as_deref())
            .map(apolline_config::slug);
        if let Some(wanted) = theme
            && let Some((theme, _)) = crate::themes::all()
                .into_iter()
                .find(|(t, _)| apolline_config::slug(&t.name) == wanted)
        {
            theme.dress(&mut config);
        }
        self.config = config;
    }

    fn release_held(&mut self) {
        let rule = self.rule();
        let now = Instant::now();
        let (show, keep) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|n| notify::lets_through(rule, n.urgency));
        self.held = keep;
        self.popups
            .extend(show.into_iter().map(|n: Notification| (n, now)));
    }

    // The window of the app a notification came from, to bring forward on a click.
    fn app_window(&self, notification: &notify::Notification) -> Option<u64> {
        let state = self.niri.as_ref()?;
        let names = [
            notification.desktop.as_deref(),
            Some(notification.app.as_str()),
        ];
        state
            .recent()
            .into_iter()
            .find(|window| {
                let Some(app) = window.app_id.as_deref() else {
                    return false;
                };
                names.iter().flatten().any(|name| {
                    !name.is_empty()
                        && (app.eq_ignore_ascii_case(name)
                            || app
                                .rsplit('.')
                                .next()
                                .is_some_and(|last| last.eq_ignore_ascii_case(name)))
                })
            })
            .map(|window| window.id)
    }

    fn remind(&mut self) -> Task<Message> {
        let now = self.now.naive_local();
        self.reminded
            .retain(|(begins, _, _)| *begins > now - chrono::TimeDelta::hours(1));
        let due: Vec<(crate::events::Occurrence, chrono::TimeDelta)> = self
            .agenda
            .due(now)
            .filter(|(o, lead)| {
                !self.reminded.iter().any(|(begins, what, at)| {
                    Some(*begins) == o.begins() && *what == o.summary && at == lead
                })
            })
            .map(|(o, lead)| (o.clone(), lead))
            .collect();
        let mut cards = Vec::new();
        for (occurrence, lead) in due {
            self.reminded.push((
                occurrence.begins().unwrap_or(now),
                occurrence.summary.clone(),
                lead,
            ));
            self.reminders = self.reminders.wrapping_add(1);
            let card = notify::reminder_card(&occurrence, now, self.reminders);
            cards.push(self.update(Message::Notify(card)));
        }
        Task::batch(cards)
    }

    fn forget(&mut self, id: u32) {
        self.popups.retain(|(n, _)| n.id != id);
        self.held.retain(|n| n.id != id);
        self.replying.take_if(|(replying, _)| *replying == id);
    }

    // The card being answered needs the keyboard until the answer is sent or dropped.
    #[cfg(target_os = "linux")]
    pub fn replying(&self) -> bool {
        self.replying.is_some()
    }

    fn expire(&mut self) -> Task<Message> {
        let now = Instant::now();
        let answering = self.replying.as_ref().map(|(id, _)| *id);
        let (gone, live): (Vec<_>, Vec<_>) = std::mem::take(&mut self.popups)
            .into_iter()
            .partition(|(n, shown)| {
                answering != Some(n.id)
                    && n.timeout.is_some_and(|t| now.duration_since(*shown) >= t)
            });
        self.popups = live;
        // A card that slips into the history isn't closed for the app: a click there
        // still has to reach it. Only what isn't kept is.
        Task::batch(
            gone.into_iter()
                .filter(|(n, _)| !self.history.iter().any(|kept| kept.id == n.id))
                .map(|(n, _)| notify::closed(n.id, Closed::Expired)),
        )
    }

    #[cfg(target_os = "linux")]
    pub fn popup_height(&self) -> u32 {
        popup::height(&self.shown_cards())
    }

    #[cfg(target_os = "linux")]
    pub fn answerable(&self) -> bool {
        self.shown_cards().iter().any(|n| n.reply.is_some())
    }

    fn shown_cards(&self) -> Vec<&notify::Notification> {
        popup::shown(
            self.popups.iter().rev().map(|(n, _)| n),
            self.replying.as_ref().map(|(id, _)| *id),
        )
    }

    #[cfg(target_os = "linux")]
    pub fn size_of(&self, overlay: Overlay) -> f32 {
        let style = &self.config.style;
        match overlay {
            Overlay::Launcher => style.launcher.size,
            Overlay::Panel => style.panel.size,
            _ => None,
        }
        .unwrap_or(1.0)
    }

    pub fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    pub fn overlay_view(&self) -> Element<'_, Message> {
        let palette = self.config.palette(self.mode);
        match self.overlay {
            Some(Overlay::Switcher) => switcher::view(self.mode, &self.config, &self.thumbs),
            Some(Overlay::Launcher) => self.launcher.view(
                palette,
                self.current()
                    .map(|m| (m.name.as_str(), crate::icon::Icon::named(&m.icon))),
            ),
            Some(Overlay::Clipboard) => self.picker.view(&self.clips, palette),
            Some(Overlay::Keys) => keymap::view(&self.keymap, &self.keys_query, palette),
            Some(Overlay::Marks) => match &self.marks {
                Some(marks) => crate::marks::view(marks, palette),
                None => space().into(),
            },
            Some(Overlay::TrayMenu) => match &self.tray_menu {
                Some((item, entries)) => crate::tray_menu::view(
                    item,
                    entries,
                    palette,
                    self.config.bar.position == Edge::Bottom,
                    style::under_bar(&self.config.bar, &self.config.style, 1.0),
                ),
                None => space().into(),
            },
            Some(Overlay::Calendar) => crate::calendar::view(
                self.now,
                self.month,
                self.day,
                &self.agenda,
                self.weather.as_ref(),
                palette,
                self.config.bar.position == Edge::Bottom,
                style::under_bar(&self.config.bar, &self.config.style, 1.0),
                self.config.calendars.len(),
            ),
            Some(Overlay::Polkit) => match &self.polkit {
                Some(dialog) => polkit::view(dialog, palette),
                None => space().into(),
            },
            Some(Overlay::Panel) => panel::view(Panel {
                at_bottom: self.config.bar.position == Edge::Bottom,
                gap: style::under_bar(
                    &self.config.bar,
                    &self.config.style,
                    self.config.style.panel.size.unwrap_or(1.0),
                ),
                modes: &self.config.modes,
                mode: self.mode,
                palette,
                controls: &self.controls,
                battery: self.battery(),
                track: self.track(),
                do_not_disturb: self.config.alerts.do_not_disturb,
                power_menu: self.power_menu,
                armed: self.armed,
                awake: self.awake,
                look: self.config.style.panel,
                night: self.night.1.unwrap_or(self.night.0),
                profiles: self.profiles.as_ref(),
                quiet_until: self.quiet_until,
                history: &self.history,
                opened: &self.opened,
                apps: &self.apps,
                detail: self.detail,
                networks: &self.networks,
                gadgets: &self.gadgets,
                joining: self.joining.as_ref(),
                asking: self.asking.as_ref(),
                pin: &self.pin,
                note: self.net_note.as_deref(),
                busy: self.net_busy,
                tunnels: &self.tunnels,
            }),
            None => space().into(),
        }
    }

    // At the bottom of the screen the newest card is the one nearest the edge.
    pub fn popups(&self) -> Element<'_, Message> {
        let mut cards = self.shown_cards();
        if self.config.alerts.position.bottom() {
            cards.reverse();
        }
        popup::stack(
            cards,
            &self.apps,
            self.config.palette(self.mode),
            self.replying.as_ref(),
            self.config.style.cards,
        )
    }

    #[cfg(target_os = "linux")]
    pub fn cards_place(&self) -> (apolline_config::Corner, f32) {
        let size = self.config.style.cards.size.unwrap_or(1.0);
        (self.config.alerts.position, size)
    }

    #[cfg(target_os = "linux")]
    pub fn osd_size(&self) -> f32 {
        self.config.style.osd.size.unwrap_or(1.0)
    }

    fn look(&self) -> Fragment<'_> {
        Fragment {
            config: &self.config,
            mode: self.mode,
            outputs: &self.outputs,
        }
    }

    fn open_apps(&self) -> Task<Message> {
        let (Some(state), Some(mode)) = (&self.niri, self.current()) else {
            return Task::none();
        };
        Task::batch(
            mode.open
                .iter()
                .filter(|command| !state.is_running(command))
                .map(|command| niri::send(apolline_niri::Request::Spawn(command.clone()))),
        )
    }

    // Each screen keeps its old picture until the new one has loaded, then fades over.
    fn show_wallpapers(&mut self) {
        let screens: Vec<Option<String>> = std::iter::once(None)
            .chain(self.outputs.iter().cloned().map(Some))
            .collect();
        let mut shown = Vec::with_capacity(screens.len());
        for output in screens {
            let wanted = self.wallpaper_name(output.as_deref()).to_owned();
            let before = self
                .shown
                .iter()
                .position(|s| s.output == output)
                .map(|i| self.shown.swap_remove(i));
            let loaded = self.wallpapers.iter().any(|(n, _)| *n == wanted);
            match before {
                Some(before) if !loaded || before.name == wanted => shown.push(before),
                before => {
                    let from = before
                        .filter(|_| !self.demo)
                        .and_then(|b| self.picture(&b.name).cloned())
                        .map(|old| (old, Instant::now()));
                    shown.push(Shown {
                        output,
                        name: wanted,
                        from,
                    });
                }
            }
        }
        self.shown = shown;
        let wanted = self.wanted_wallpapers();
        let shown = &self.shown;
        self.wallpapers
            .retain(|(n, _)| wanted.contains(n) || shown.iter().any(|s| s.name == *n));
    }

    fn picture(&self, name: &str) -> Option<&image::Handle> {
        self.wallpapers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, handle)| handle)
    }

    fn load_wallpaper(&mut self) -> Task<Message> {
        self.show_wallpapers();
        let wanted = self.wanted_wallpapers();
        let missing: Vec<String> = wanted
            .into_iter()
            .filter(|name| !self.wallpapers.iter().any(|(n, _)| n == name))
            .collect();
        Task::batch(missing.into_iter().map(|name| {
            Task::perform(wallpaper::load(name.clone()), move |result| {
                Message::Wallpaper(name.clone(), result)
            })
        }))
    }

    pub fn wallpaper(&self, output: Option<&str>) -> Element<'_, Message> {
        let picture = self.wallpaper_picture(output);
        match self.config.desktop.clock {
            Some(spot) => stack![picture, self.desktop_clock(spot)].into(),
            None => picture,
        }
    }

    // The time and the date, big and light, on the wallpaper.
    fn desktop_clock(&self, spot: apolline_config::Spot) -> Element<'_, Message> {
        use apolline_config::{Corner, Spot};
        let ink = apolline_theme::Scheme::DARK.text;
        let (x, y) = match spot {
            Spot::Center => (Alignment::Center, Alignment::Center),
            Spot::Corner(corner) => (
                match corner {
                    Corner::TopLeft | Corner::BottomLeft => Alignment::Start,
                    Corner::Top | Corner::Bottom => Alignment::Center,
                    Corner::TopRight | Corner::BottomRight => Alignment::End,
                },
                if corner.bottom() {
                    Alignment::End
                } else {
                    Alignment::Start
                },
            ),
        };
        let clock = column![
            text(self.now.format("%H:%M").to_string())
                .size(120)
                .font(style::light())
                .line_height(0.95)
                .color(style::color(ink)),
            text(self.now.format("%A, %-d %B").to_string())
                .size(18)
                .font(style::medium())
                .color(style::color(ink.alpha(0.8))),
        ]
        .spacing(2)
        .align_x(x);
        // Clear of the bar on its side, and of the screen's edges.
        let bar = style::under_bar(&self.config.bar, &self.config.style, 1.0) + 32.0;
        let padding = match self.config.bar.position {
            Edge::Top => Padding::new(56.0).top(bar),
            Edge::Bottom => Padding::new(56.0).bottom(bar),
        };
        container(clock)
            .width(Fill)
            .height(Fill)
            .padding(padding)
            .align_x(x)
            .align_y(y)
            .into()
    }

    fn wallpaper_picture(&self, output: Option<&str>) -> Element<'_, Message> {
        let shown = self
            .shown
            .iter()
            .find(|s| s.output.as_deref() == output)
            .filter(|s| self.picture(&s.name).is_some())
            .or_else(|| self.shown.iter().find(|s| s.output.is_none()));
        let picture = |handle: &image::Handle, opacity: f32| {
            image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .opacity(opacity)
        };
        match shown.and_then(|s| Some((s, self.picture(&s.name)?))) {
            Some((shown, handle)) => match &shown.from {
                Some((old, at)) => {
                    let t = (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0);
                    let eased = 1.0 - (1.0 - t).powi(3);
                    stack![picture(old, 1.0), picture(handle, eased)].into()
                }
                None => picture(handle, 1.0).into(),
            },
            None => container(space())
                .width(Fill)
                .height(Fill)
                .style(|_| container::background(style::color(neutral::ink())))
                .into(),
        }
    }

    #[cfg(target_os = "linux")]
    // Where the bar is, whether it floats, and how much bigger it is drawn.
    pub fn bar_look(&self) -> (Edge, bool, u16) {
        let size = self.config.style.bar.size.unwrap_or(1.0);
        (
            self.config.bar.position,
            self.config.bar.floating,
            (size * 100.0).round() as u16,
        )
    }

    #[cfg(target_os = "linux")]
    pub fn outputs(&self) -> Option<&[String]> {
        self.niri.as_ref().map(|_| self.outputs.as_slice())
    }

    fn battery(&self) -> Option<Battery> {
        match &self.system {
            Some(system) => system.battery,
            None => self.status.battery,
        }
    }

    fn track(&self) -> Option<&Track> {
        match &self.system {
            Some(system) => system.media.as_ref(),
            None => self.status.media.as_ref(),
        }
    }

    pub fn bar(&self, output: Option<&str>) -> Element<'_, Message> {
        let (status, active) = match &self.niri {
            Some(state) => {
                let status = Status::from_niri(state, output);
                let active = status.active;
                (status, active)
            }
            None => (self.status.clone(), self.workspace),
        };
        bar::view(Bar {
            mode: self.current(),
            has_modes: !self.config.modes.is_empty(),
            palette: self.config.palette(self.mode),
            active,
            status,
            battery: self.battery(),
            track: self.track(),
            controls: &self.controls,
            held: self.held.len(),
            do_not_disturb: self.config.alerts.do_not_disturb,
            awake: self.awake,
            now: self.now,
            look: &self.config.bar,
            radius: style::bar_size(&self.config.style).1,
            panel_open: self.overlay == Some(Overlay::Panel),
            apps: &self.apps,
            pinned: self.pinned(),
            tray: &self.tray,
            icons: self.config.desktop.icons.as_deref(),
            dragging: self
                .dragging
                .as_ref()
                .map(|(app, was_pinned, x)| (app.as_str(), *was_pinned, *x)),
            recording: self.recording.as_ref().map(|r| r.started.elapsed()),
            stats: self.stats,
            next: self
                .agenda
                .soon(self.now.naive_local())
                .map(|(o, wait)| (&*o.summary, wait)),
            modules: self
                .module_text
                .iter()
                .filter(|(i, _)| *i < self.config.modules.len())
                .map(|(i, text)| bar::Module {
                    index: *i,
                    name: self.config.modules[*i].name.as_deref(),
                    text,
                    clickable: self.config.modules[*i].click.is_some(),
                })
                .collect(),
        })
    }

    pub fn bar_margin(&self) -> Padding {
        let look = &self.config.bar;
        let inset = if look.floating {
            metrics::BAR_INSET
        } else {
            0.0
        };
        match look.position {
            Edge::Top => Padding::new(inset).bottom(0),
            Edge::Bottom => Padding::new(inset).top(0),
        }
    }
}

fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Shell {
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let busy = self.demo
            || self.recording.is_some()
            || self.config.bar.seconds
            || self.overlay.is_some()
            || !self.popups.is_empty();
        let resting = self.awake || self.sample() || self.current().is_some_and(|m| m.keep_awake);
        let stages = if resting {
            Vec::new()
        } else {
            idle::stages(&self.config.idle)
        };
        let clipboard = if self.sample() {
            iced::Subscription::none()
        } else {
            clipboard::subscription().map(Message::Clip)
        };
        let fading = if self.shown.iter().any(|s| s.from.is_some()) {
            iced::window::frames().map(|_| Message::Frame)
        } else {
            iced::Subscription::none()
        };
        let watched = if self.demo {
            iced::Subscription::none()
        } else {
            iced::Subscription::batch([
                iced::Subscription::run(watch_config),
                crate::weather::subscription(self.config.weather.clone()),
                crate::events::subscription(&self.config.calendars),
                crate::stats::subscription(self.config.bar.stats.any()),
                if self.overlay == Some(Overlay::Panel) {
                    iced::Subscription::none()
                } else {
                    controls::volume().map(Message::Volume)
                },
                crate::profile::subscription().map(Message::Profiles),
                crate::wifi::subscription(self.listing(panel::Detail::Wifi)),
                crate::bluetooth::subscription(self.listing(panel::Detail::Bluetooth)),
                crate::tray::subscription().map(Message::Tray),
                crate::vpn::subscription().map(Message::Tunnels),
            ])
        };
        iced::Subscription::batch([
            subscription(),
            fading,
            crate::night::subscription(self.night_light(), &self.outputs).map(|()| Message::Frame),
            crate::modules::subscription(&self.config.modules),
            iced::Subscription::run_with(busy, ticks),
            watched,
            clipboard,
            idle::subscription(stages).map(|(stage, idle)| Message::Idle(stage, idle)),
        ])
    }
}

struct Plan {
    back: Option<Option<String>>,
    start: Option<String>,
    auto: Option<(String, Option<String>)>,
}

fn plan(
    active: &[String],
    fired: &[String],
    current: Option<&str>,
    auto: Option<&(String, Option<String>)>,
) -> Plan {
    let mut current = current.map(str::to_owned);
    let mut auto = auto.cloned();
    let mut back = None;
    if let Some((on, before)) = auto.clone()
        && !active.contains(&on)
    {
        auto = None;
        if current.as_ref() == Some(&on) {
            back = Some(before.clone());
            current = before;
        }
    }
    let start = active
        .iter()
        .find(|key| !fired.contains(key) && current.as_ref() != Some(*key))
        .cloned();
    if let Some(key) = &start {
        let before = match &auto {
            Some((on, before)) if current.as_ref() == Some(on) => before.clone(),
            _ => current,
        };
        auto = Some((key.clone(), before));
    }
    Plan { back, start, auto }
}

fn ticks(busy: &bool) -> impl iced::futures::Stream<Item = Message> + use<> {
    let busy = *busy;
    iced::stream::channel(1, async move |mut output| {
        use chrono::Timelike;
        use iced::futures::SinkExt;
        loop {
            let now = Local::now();
            let millis = u64::from(now.timestamp_subsec_millis().min(999));
            let wait = if busy {
                1000 - millis
            } else {
                u64::from(60 - now.second()) * 1000 - millis
            };
            tokio::time::sleep(Duration::from_millis(wait.max(5))).await;
            if output.send(Message::Tick).await.is_err() {
                return;
            }
        }
    })
}

fn watch_config() -> impl iced::futures::Stream<Item = Message> {
    let path = apolline_config::default_path();
    iced::stream::channel(1, async move |mut output| {
        use iced::futures::SinkExt;
        let mut seen = stamp(&path);
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let now = stamp(&path);
            if now != seen {
                seen = now;
                if output.send(Message::ConfigChanged).await.is_err() {
                    return;
                }
            }
        }
    })
}

fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::window::open_events().map(|_| Message::Opened),
        iced::event::listen_with(|event, status, _| match event {
            iced::Event::Keyboard(key @ keyboard::Event::KeyPressed { .. }) => {
                Some(Message::Key(key, status == iced::event::Status::Captured))
            }
            _ => None,
        }),
        niri::events(),
        control::listen(),
        power::subscription(),
        media::subscription(),
        notify::subscription(),
        logind::subscription(),
        polkit::subscription(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|k| (*k).to_owned()).collect()
    }

    #[test]
    fn triggers_hand_back_what_you_chose_and_never_fire_twice() {
        let on_battery = plan(&keys(&["saver"]), &[], None, None);
        assert_eq!(on_battery.start.as_deref(), Some("saver"));
        let auto = on_battery.auto;
        assert_eq!(auto, Some(("saver".into(), None)));

        let picked_focus = plan(
            &keys(&["saver"]),
            &keys(&["saver"]),
            Some("focus"),
            auto.as_ref(),
        );
        assert!(picked_focus.start.is_none() && picked_focus.back.is_none());

        let night = plan(
            &keys(&["saver", "night"]),
            &keys(&["saver"]),
            Some("focus"),
            picked_focus.auto.as_ref(),
        );
        assert_eq!(night.start.as_deref(), Some("night"));
        assert_eq!(night.auto, Some(("night".into(), Some("focus".into()))));

        let morning = plan(
            &keys(&["saver"]),
            &keys(&["saver", "night"]),
            Some("night"),
            night.auto.as_ref(),
        );
        assert_eq!(
            morning.back,
            Some(Some("focus".into())),
            "back to the mode you picked"
        );
        assert!(morning.start.is_none(), "battery already had its turn");
        assert!(morning.auto.is_none());
    }
}

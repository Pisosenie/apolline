use apolline_config::{Config, Edge, Editor, ModeSettings, Notifications, Trigger};
mod actions;
mod cards;
mod config_watch;
mod desk;
mod handle;
mod handle_more;
mod input;
mod modes;
mod views;
mod widgets;

use apolline_theme::{metrics, neutral};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use crate::style::text;
use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, image, space, stack};
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
    Lyrics(String, Option<crate::lyrics::Lyrics>),
    AskPosition,
    Position(Option<std::time::Duration>),
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
    Module(usize, crate::modules::Said),
    ModuleAct(usize, crate::modules::Act),
    ModuleScroll(usize, f32),
    // Starts a module's command again: the one at that index, or every one.
    ModuleRerun(Option<usize>),
    StartTimer(Duration, Option<String>),
    StopTimer(u64),
    NightLight(bool),
    Frame,
    CalendarMonth(i32),
    CalendarDay(chrono::NaiveDate),
    Stats(crate::stats::Stats),
    Machine(Box<crate::machine::Machine>),
    Listening(Vec<String>),
    CheckUpdates,
    Updates(Option<crate::updates::Updates>),
    Upgrade,
    MachineSort(bool),
    EndProcess(u32),
    Rates(Result<std::sync::Arc<crate::rates::Rates>, String>),
    Agenda(std::sync::Arc<crate::events::Agenda>),
    Weather(Option<crate::weather::Weather>),
    Files(Vec<crate::files::Found>),
    Detail(Option<panel::Detail>),
    Volume(Option<controls::Volume>),
    Sound(controls::Heard),
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
    // A hidden bar's pointer left a while ago; the number tells a later leaving from this one.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    HideBar(iced::window::Id, u64),
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
    // What the machine is doing, from the bar's numbers.
    Machine,
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
    // The enabled plugins, read with the config.
    plugins: Vec<crate::plugins::Loaded>,
    handed: Vec<(&'static str, Option<String>)>,
    started: Instant,
    system: Option<System>,
    boosted: Option<String>,
    popups: Vec<(Notification, Instant)>,
    held: Vec<Notification>,
    history: Vec<Notification>,
    // What of the history is on disk, as kept::marks has it.
    kept: Vec<(u32, std::time::SystemTime)>,
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
    machine: Option<Box<crate::machine::Machine>>,
    listening: Vec<String>,
    // The words of what's playing, by the track they're for, and how far into it the player is.
    lyrics: Option<(String, Option<crate::lyrics::Lyrics>)>,
    position: Option<std::time::Duration>,
    updates: Option<crate::updates::Updates>,
    by_memory: bool,
    ending: Option<u32>,
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
    // The quiet hours turned off by hand, until they would have ended.
    quiet_off_until: Option<DateTime<Local>>,
    was_quiet: bool,
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
    // What the sound server said while a slider moved, read once things are quiet.
    sound_held: Option<controls::Heard>,
    // The icon theme and the app folders as of the last scan, to scan again only when they change.
    apps_seen: Option<(Option<String>, Vec<Option<SystemTime>>)>,
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
    module_text: Vec<(usize, crate::modules::Said)>,
    module_runs: Vec<u32>,
    module_scrolled: Option<(usize, std::time::Instant)>,
    timers: Vec<crate::timers::Timer>,
    // How many timers have rung, for their cards' ids.
    rung: u32,
    timers_started: u64,
    recording: Option<crate::capture::Recording>,
    replay: Option<crate::capture::Replay>,
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
            kept: Vec::new(),
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
            machine: None,
            listening: Vec::new(),
            lyrics: None,
            position: None,
            updates: None,
            by_memory: false,
            ending: None,
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
            quiet_off_until: None,
            was_quiet: false,
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
            sound_held: None,
            apps_seen: None,
            power_menu: false,
            armed: None,
            thumbs: HashMap::new(),
            plain: config.clone(),
            plugins: crate::plugins::load(&config),
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
            module_runs: Vec::new(),
            module_scrolled: None,
            timers: Vec::new(),
            rung: 0,
            timers_started: 0,
            recording: None,
            replay: None,
            marks: None,
            shot: None,
            ticks: 0,
            published: None,
            now: Local::now(),
        };
        shell.report(&diagnostics);
        // What came in before this start is back in quick settings, and written again at the
        // first change without what got too old. Turned off, nothing is left on disk.
        if !demo && shell.config.alerts.keep {
            shell.history = crate::kept::load();
        } else if !demo && let Err(err) = crate::kept::forget() {
            eprintln!("apolline: notification history: {err}");
        }
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
            shell.scan_apps(),
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
            if let Err(err) = crate::terminal::write_colors(&scheme, palette) {
                eprintln!("apolline: terminal colors: {err}");
            }
            let (ran, problems) = crate::templates::write(&self.config.templates, &scheme, palette);
            for problem in problems {
                eprintln!("apolline: template: {problem}");
            }
            for command in ran {
                let started = std::process::Command::new("sh")
                    .args(["-c", &command])
                    .stdin(std::process::Stdio::null())
                    .spawn();
                // Waited on aside, so a finished one doesn't linger as a zombie.
                if let Ok(mut child) = started {
                    std::thread::spawn(move || child.wait());
                }
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
                    include_str!("../../../../assets/config.toml"),
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
        let icons = self.config.desktop.icons.clone();
        if config.desktop.rotate == 0 || config.desktop.wallpaper != self.picked {
            self.rotated = None;
        }
        self.picked = config.desktop.wallpaper.clone();
        if let Some((name, _)) = &self.rotated {
            config.desktop.wallpaper = name.clone();
        }
        self.mode = key.and_then(|key| config.find(&key));
        self.plugins = crate::plugins::load(&config);
        self.plain = config;
        self.dress();
        if self.config.desktop.icons != icons {
            let tray = std::mem::take(&mut self.tray);
            self.tray = crate::tray::draw(tray, &[], self.config.desktop.icons.as_deref());
        }
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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.publish();
        self.keep_history();
        let scheme = self.config.scheme(self.mode);
        if scheme != apolline_theme::scheme() {
            apolline_theme::set_scheme(scheme);
        }
        crate::i18n::choose(self.config.language.as_deref());
        task
    }

    // The history on disk follows the one in quick settings; turned off, it goes.
    fn keep_history(&mut self) {
        if self.demo {
            return;
        }
        let marks = if self.config.alerts.keep {
            crate::kept::marks(&self.history)
        } else {
            Vec::new()
        };
        if marks == self.kept {
            return;
        }
        let wrote = if marks.is_empty() {
            crate::kept::forget()
        } else {
            crate::kept::save(&self.history)
        };
        if let Err(err) = wrote {
            eprintln!("apolline: notification history: {err}");
        }
        self.kept = marks;
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

    fn look(&self) -> Fragment<'_> {
        Fragment {
            config: &self.config,
            mode: self.mode,
            outputs: &self.outputs,
        }
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

    // The line being sung, with the next, while quick settings can show it.
    fn sung(&self) -> Option<(&str, Option<&str>)> {
        let track = self.track()?;
        let (key, lyrics) = self.lyrics.as_ref()?;
        if *key != crate::lyrics::key(track) {
            return None;
        }
        lyrics.as_ref()?.at(self.position?)
    }

    fn listening_for_words(&self) -> bool {
        self.overlay == Some(Overlay::Panel)
            && self.track().is_some_and(|t| t.playing)
            && self.lyrics.as_ref().is_some_and(|(_, l)| l.is_some())
    }
}

fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Shell {
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let busy = self.demo
            || self.recording.is_some()
            || !self.timers.is_empty()
            || self.config.bar.seconds
            || self.overlay.is_some()
            || !self.popups.is_empty();
        let resting = self.awake || self.sample() || self.current().is_some_and(|m| m.keep_awake);
        let stages = if resting {
            Vec::new()
        } else {
            let on_battery = self.battery().is_some_and(|b| !b.charging);
            let stages = if on_battery {
                idle::stages(&self.config.idle.on_battery())
            } else {
                idle::stages(&self.config.idle)
            };
            // A running timer has to be awake to ring: the screen may go, the machine stays.
            stages
                .into_iter()
                .filter(|(stage, _)| self.timers.is_empty() || *stage != idle::Stage::Suspend)
                .collect()
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
                iced::Subscription::run(config_watch::watch_config),
                crate::weather::subscription(self.config.weather.clone()),
                crate::events::subscription(&self.config.calendars),
                crate::stats::subscription(self.config.bar.stats.any()),
                crate::machine::subscription(self.overlay == Some(Overlay::Machine)),
                if self.listening_for_words() {
                    iced::time::every(std::time::Duration::from_millis(400))
                        .map(|_| Message::AskPosition)
                } else {
                    iced::Subscription::none()
                },
                crate::updates::subscription(self.config.bar.has(&apolline_config::Piece::Updates)),
                controls::heard().map(Message::Sound),
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
            crate::modules::subscription(&self.config.modules, &self.module_runs, &self.plugins),
            iced::Subscription::run_with(busy, ticks),
            watched,
            clipboard,
            idle::subscription(stages).map(|(stage, idle)| Message::Idle(stage, idle)),
        ])
    }
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

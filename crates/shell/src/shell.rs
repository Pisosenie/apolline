use apolline_config::{Config, Edge, Editor, ModeSettings, Notifications};
use apolline_theme::{metrics, neutral};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, image, space};
use iced::{ContentFit, Element, Fill, Padding, Task, window};

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
use crate::popup;
use crate::power::{self, Battery};
use crate::profile;
use crate::status::Status;
use crate::switcher;
use crate::{style, wallpaper};

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
    Forget(u32),
    ClearAll,
    Idle(Stage, bool),
    KeepAwake(bool),
    Osd(Option<Osd>),
    OsdDone(u64),
    Clip(Clip),
    FocusWindow(u64),
    CloseWindow(u64),
    ClipQuery(String),
    ClipCopy(Option<usize>),
    ClipPin(usize),
    ClipRemove(usize),
    KeysQuery(String),
    Overlay(Option<Overlay>),
    Opened,
    Thumb(String, Option<image::Handle>),
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
    apps: Vec<App>,
    overlay: Option<Overlay>,
    launcher: Launcher,
    controls: Controls,
    touched: Option<Instant>,
    power_menu: bool,
    armed: Option<Power>,
    thumbs: HashMap<String, image::Handle>,
    workspace: usize,
    wallpaper: Option<(String, image::Handle)>,
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
                Vec::new()
            },
            picker: Picker::default(),
            keymap: Vec::new(),
            keys_query: String::new(),
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
            config,
            wallpaper: None,
            ticks: 0,
            published: None,
            now: Local::now(),
        };
        shell.report(&diagnostics);
        fragment::sync(&shell.look());
        shell.launcher.set_modes(shell.mode_names());
        let load = Task::batch([
            shell.load_wallpaper(),
            shell.load_thumbs(),
            shell.read_controls(),
            Task::perform(apps::scan(), Message::Apps),
        ]);
        (shell, load)
    }

    fn reload_config(&mut self) -> Task<Message> {
        let (config, diagnostics) = Config::load(&self.config_path);
        let key = self.current().map(ModeSettings::key);
        let before = self.config.wallpaper(self.mode).to_owned();
        self.config = config;
        self.mode = key.and_then(|key| self.config.find(&key));
        self.launcher.set_modes(self.mode_names());
        self.report(&diagnostics);
        fragment::sync(&self.look());
        self.release_held();
        let wallpaper = if self.config.wallpaper(self.mode) != before {
            self.load_wallpaper()
        } else {
            Task::none()
        };
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
        self.wallpaper
            .as_ref()
            .is_some_and(|(name, _)| name == self.config.wallpaper(self.mode))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.now = Local::now();
                self.ticks += 1;
                let reload = match stamp(&self.config_path) {
                    stamp if stamp != self.config_stamp => {
                        self.config_stamp = stamp;
                        self.reload_config()
                    }
                    _ => Task::none(),
                };
                let every = if self.overlay == Some(Overlay::Panel) {
                    1
                } else {
                    5
                };
                let controls = if self.ticks.is_multiple_of(every) {
                    self.read_controls()
                } else {
                    Task::none()
                };
                self.publish();
                return Task::batch([reload, self.expire(), controls]);
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
                }
            }
            Message::ClipRemove(index) => {
                if index < self.clips.len() {
                    self.clips.remove(index);
                    let count = self.picker.visible(&self.clips).len();
                    self.picker.selected = self.picker.selected.min(count.saturating_sub(1));
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
            Message::Niri(event) => {
                let switched = matches!(event, apolline_niri::Event::KeyboardLayoutSwitched { .. });
                let state = self.niri.get_or_insert_default();
                state.apply(event);
                if switched && let Some(name) = state.layout().map(str::to_owned) {
                    return self.show(Osd::Layout(name));
                }
                let state = self.niri.get_or_insert_default();
                let outputs = state.outputs();
                if outputs != self.outputs {
                    self.outputs = outputs;
                    fragment::sync(&self.look());
                }
            }
            Message::NiriReply(Err(err)) => eprintln!("apolline: niri: {err}"),
            Message::Wallpaper(name, Ok(handle)) if name == self.config.wallpaper(self.mode) => {
                self.wallpaper = Some((name, handle));
                fragment::sync(&self.look());
            }
            Message::Wallpaper(_, Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => self.system.get_or_insert_default().battery = battery,
            Message::Media(media) => self.system.get_or_insert_default().media = media,
            Message::Boosted(previous) if self.current().is_some_and(|m| m.performance) => {
                self.boosted = previous
            }
            Message::Boosted(Some(previous)) => {
                return Task::future(profile::restore(previous)).discard();
            }
            Message::Notify(mut notification) => {
                self.forget(notification.id);
                if notification.timeout == Some(notify::SHOWN_FOR) {
                    let seconds = u64::from(self.config.alerts.seconds);
                    notification.timeout = Some(Duration::from_secs(seconds));
                }
                self.history.retain(|n| n.id != notification.id);
                if notification.id != notify::CONFIG_CARD {
                    self.history.insert(0, notification.clone());
                    self.history.truncate(50);
                }
                if notify::lets_through(self.rule(), notification.urgency) {
                    self.popups.push((notification, Instant::now()));
                } else {
                    self.held.push(notification);
                }
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
                return notify::closed(id, Closed::Dismissed);
            }
            Message::Apps(apps) => {
                let apps = if apps.is_empty() && self.demo {
                    apps::sample()
                } else {
                    apps
                };
                self.apps = apps.clone();
                self.launcher.set_apps(apps);
            }
            Message::Query(query) => return self.launcher.search(query),
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
                    self.controls = controls;
                }
            }
            Message::Change(change) => {
                self.controls.apply(&change);
                self.touched = Some(Instant::now());
                if !self.sample() {
                    return Task::future(controls::change(change)).discard();
                }
            }
            Message::Player(action) => return self.player(action),
            Message::DoNotDisturb(on) => {
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
        let launcher = self.overlay == Some(Overlay::Launcher);
        if self.overlay == Some(Overlay::Clipboard) {
            return self.clipboard_key(key, text, captured);
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
            Key::Named(Named::ArrowDown) if launcher => return self.launcher.step(1),
            Key::Named(Named::ArrowUp) if launcher => return self.launcher.step(-1),
            _ if captured => {}
            Key::Named(Named::Backspace) if launcher => {
                let mut query = self.launcher.query.clone();
                query.pop();
                return self.launcher.search(query);
            }
            Key::Named(Named::Enter) if launcher => return self.update(Message::Submit),
            _ if launcher => {
                if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                    let query = format!("{}{typed}", self.launcher.query);
                    return self.launcher.search(query);
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
        self.overlay = overlay;
        self.power_menu = false;
        self.armed = None;
        match overlay {
            Some(Overlay::Launcher) => Task::batch([
                {
                    self.launcher.set_windows(self.open_windows());
                    self.launcher.open()
                },
                Task::perform(apps::scan(), Message::Apps),
            ]),
            Some(Overlay::Panel) => {
                self.held.clear();
                self.read_controls()
            }
            Some(Overlay::Clipboard) => self.picker.open(),
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
            Target::App(command) => self.spawn(&command),
            Target::Window(id) => {
                return Task::batch([close, self.update(Message::FocusWindow(id))]);
            }
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
            .map(|(osd, _)| osd::view(osd, self.config.palette(self.mode)))
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
        let shared = Shared {
            mode: self.current().map(ModeSettings::key),
            waiting: self.held.len() + self.popups.len(),
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

    fn switch(&mut self, mode: Option<usize>) -> Task<Message> {
        let close = self.set_overlay(None);
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
        fragment::sync(&self.look());
        self.release_held();
        Task::batch([close, self.load_wallpaper(), self.open_apps(), profile])
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

    fn forget(&mut self, id: u32) {
        self.popups.retain(|(n, _)| n.id != id);
        self.held.retain(|n| n.id != id);
    }

    fn expire(&mut self) -> Task<Message> {
        let now = Instant::now();
        let (gone, live): (Vec<_>, Vec<_>) = std::mem::take(&mut self.popups)
            .into_iter()
            .partition(|(n, shown)| n.timeout.is_some_and(|t| now.duration_since(*shown) >= t));
        self.popups = live;
        Task::batch(
            gone.into_iter()
                .map(|(n, _)| notify::closed(n.id, Closed::Expired)),
        )
    }

    #[cfg(target_os = "linux")]
    pub fn popup_count(&self) -> usize {
        self.popups.len().min(popup::MAX)
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
            Some(Overlay::Panel) => panel::view(Panel {
                at_bottom: self.config.bar.position == Edge::Bottom,
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
                history: &self.history,
            }),
            None => space().into(),
        }
    }

    pub fn popups(&self) -> Element<'_, Message> {
        popup::stack(
            self.popups.iter().rev().map(|(n, _)| n),
            self.config.palette(self.mode),
        )
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

    fn load_wallpaper(&self) -> Task<Message> {
        let name = self.config.wallpaper(self.mode).to_owned();
        Task::perform(wallpaper::load(name.clone()), move |result| {
            Message::Wallpaper(name.clone(), result)
        })
    }

    pub fn wallpaper(&self) -> Element<'_, Message> {
        match &self.wallpaper {
            Some((_, handle)) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            None => container(space())
                .width(Fill)
                .height(Fill)
                .style(|_| container::background(style::color(neutral::INK)))
                .into(),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn bar_look(&self) -> (Edge, bool) {
        (self.config.bar.position, self.config.bar.floating)
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
            look: self.config.bar,
            panel_open: self.overlay == Some(Overlay::Panel),
            apps: &self.apps,
        })
    }

    pub fn bar_margin(&self) -> Padding {
        let look = self.config.bar;
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
        iced::Subscription::batch([
            subscription(),
            clipboard,
            idle::subscription(stages).map(|(stage, idle)| Message::Idle(stage, idle)),
        ])
    }
}

fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
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
    ])
}

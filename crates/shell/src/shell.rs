use apolline_config::{Config, Editor, Notifications};
use apolline_theme::{Mode, metrics, neutral};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, image, space};
use iced::{ContentFit, Element, Fill, Padding, Task, window};

use crate::apps::{self, App};
use crate::bar::{self, Bar};
use crate::control::{self, Command, Shared, me};
use crate::controls::{self, Change, Controls};
use crate::fragment::{self, Fragment};
use crate::idle::{self, Stage};
use crate::launcher::{Launcher, Target};
use crate::logind;
use crate::media::{self, Action, Track};
use crate::niri;
use crate::notify::{self, Closed, Notification};
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
    Mode(Mode),
    Workspace(usize, Option<u64>),
    Captured(window::Screenshot),
    Niri(apolline_niri::Event),
    NiriReply(Result<(), String>),
    Control(Command),
    Wallpaper(Mode, Result<image::Handle, String>),
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
    Overlay(Option<Overlay>),
    Opened,
    Thumb(Mode, Option<image::Handle>),
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
    mode: Mode,
    status: Status,
    niri: Option<apolline_niri::State>,
    outputs: Vec<String>,
    system: Option<System>,
    boosted: Option<String>,
    popups: Vec<(Notification, Instant)>,
    held: Vec<Notification>,
    history: Vec<Notification>,
    awake: bool,
    overlay: Option<Overlay>,
    launcher: Launcher,
    controls: Controls,
    touched: Option<Instant>,
    power_menu: bool,
    armed: Option<Power>,
    thumbs: [Option<image::Handle>; 3],
    workspace: usize,
    wallpaper: Option<(Mode, image::Handle)>,
    ticks: u64,
    published: Option<Shared>,
    now: DateTime<Local>,
}

impl Shell {
    pub fn new(mode: Option<Mode>, demo: bool) -> (Self, Task<Message>) {
        let config_path = apolline_config::default_path();
        let (config, diagnostics) = Config::load(&config_path);

        let mode = mode.unwrap_or(config.mode);
        let niri = niri::connected().then(apolline_niri::State::default);
        let sample = demo && niri.is_none();
        let status = if sample {
            Status::sample(mode)
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
            thumbs: [None, None, None],
            config,
            wallpaper: None,
            ticks: 0,
            published: None,
            now: Local::now(),
        };
        shell.report(&diagnostics);
        fragment::sync(&shell.look());
        let load = Task::batch([
            shell.load_wallpaper(),
            shell.load_thumbs(&Mode::ALL),
            shell.read_controls(),
        ]);
        (shell, load)
    }

    fn reload_config(&mut self) -> Task<Message> {
        let (config, diagnostics) = Config::load(&self.config_path);
        let moved: Vec<Mode> = Mode::ALL
            .into_iter()
            .filter(|&m| config.mode(m).wallpaper != self.config.mode(m).wallpaper)
            .collect();
        self.config = config;
        self.report(&diagnostics);
        fragment::sync(&self.look());
        self.release_held();
        let wallpaper = if moved.contains(&self.mode) {
            self.load_wallpaper()
        } else {
            Task::none()
        };
        Task::batch([wallpaper, self.load_thumbs(&moved)])
    }

    fn load_thumbs(&self, modes: &[Mode]) -> Task<Message> {
        Task::batch(modes.iter().map(|&mode| {
            let source = wallpaper::resolve(&self.config.mode(mode).wallpaper);
            Task::perform(wallpaper::thumbnail(source), move |thumb| {
                Message::Thumb(mode, thumb)
            })
        }))
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
            .is_some_and(|(mode, _)| *mode == self.mode)
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
            Message::Overlay(overlay) => return self.set_overlay(overlay),
            Message::Opened if self.overlay == Some(Overlay::Launcher) => {
                return self.launcher.focus();
            }
            Message::Thumb(mode, thumb) => self.thumbs[mode as usize] = thumb,
            Message::Mode(mode) | Message::Control(Command::Mode(mode)) => {
                return self.switch(mode);
            }
            Message::Control(Command::NextMode) => return self.switch(self.mode.next()),
            Message::Workspace(_, Some(id)) => {
                return niri::send(apolline_niri::Request::FocusWorkspace(id));
            }
            Message::Workspace(index, None) => self.workspace = index,
            Message::Niri(event) => {
                let state = self.niri.get_or_insert_default();
                state.apply(event);
                let outputs = state.outputs();
                if outputs != self.outputs {
                    self.outputs = outputs;
                    fragment::sync(&self.look());
                }
            }
            Message::NiriReply(Err(err)) => eprintln!("apolline: niri: {err}"),
            Message::Wallpaper(mode, Ok(handle)) if mode == self.mode => {
                self.wallpaper = Some((mode, handle));
            }
            Message::Wallpaper(_, Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => self.system.get_or_insert_default().battery = battery,
            Message::Media(media) => self.system.get_or_insert_default().media = media,
            Message::Boosted(previous) if self.mode == Mode::Play => self.boosted = previous,
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
            Key::Character("1") => return self.switch(Mode::Work),
            Key::Character("2") => return self.switch(Mode::Play),
            Key::Character("3") => return self.switch(Mode::Evening),
            Key::Character("m") if self.demo => return self.toggle(Overlay::Switcher),
            Key::Character("q") if self.demo => return self.toggle(Overlay::Panel),
            Key::Named(Named::Space) if self.demo => return self.toggle(Overlay::Launcher),
            Key::Named(Named::ArrowLeft) => self.workspace = self.workspace.saturating_sub(1),
            Key::Named(Named::ArrowRight) => {
                self.workspace = (self.workspace + 1).min(self.status.workspaces.saturating_sub(1));
            }
            _ => {}
        }
        Task::none()
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
                self.launcher.open(),
                Task::perform(apps::scan(), Message::Apps),
            ]),
            Some(Overlay::Panel) => {
                self.held.clear();
                self.read_controls()
            }
            _ => Task::none(),
        }
    }

    fn run(&mut self, target: Target) -> Task<Message> {
        let close = self.set_overlay(None);
        let task = match target {
            Target::App(command) => self.spawn(&command),
            Target::Mode(mode) => self.switch(mode),
            Target::Settings => self.spawn(&format!("{} settings", me())),
            Target::Power(power) => self.spawn(&power.command()),
        };
        Task::batch([close, task])
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
            mode: Some(self.mode),
            waiting: self.held.len() + self.popups.len(),
        };
        if self.published != Some(shared) {
            control::publish(shared);
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
            self.config.mode(self.mode).notifications
        }
    }

    fn switch(&mut self, mode: Mode) -> Task<Message> {
        let close = self.set_overlay(None);
        let profile = match (self.mode == Mode::Play, mode == Mode::Play) {
            (false, true) => Task::perform(profile::boost(), Message::Boosted),
            (true, false) => match self.boosted.take() {
                Some(previous) => Task::future(profile::restore(previous)).discard(),
                None => Task::none(),
            },
            _ => Task::none(),
        };
        self.mode = mode;
        if self.sample() {
            self.status = Status::sample(mode);
            self.workspace = self.status.active;
        }
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
            Some(Overlay::Launcher) => self.launcher.view(palette, self.mode),
            Some(Overlay::Panel) => panel::view(Panel {
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
        let Some(state) = &self.niri else {
            return Task::none();
        };
        Task::batch(
            self.config
                .mode(self.mode)
                .open
                .iter()
                .filter(|command| !state.is_running(command))
                .map(|command| niri::send(apolline_niri::Request::Spawn(command.clone()))),
        )
    }

    fn load_wallpaper(&self) -> Task<Message> {
        let mode = self.mode;
        let path = wallpaper::resolve(&self.config.mode(mode).wallpaper);
        Task::perform(wallpaper::load(path), move |result| {
            Message::Wallpaper(mode, result)
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
        let bar = bar::view(Bar {
            mode: self.mode,
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
        });
        container(bar)
            .padding(Padding::new(metrics::BAR_INSET).bottom(0))
            .into()
    }
}

fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Shell {
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let resting = self.awake
            || self.sample()
            || (self.mode == Mode::Play && self.config.idle.awake_in_play);
        let stages = if resting {
            Vec::new()
        } else {
            idle::stages(&self.config.idle)
        };
        iced::Subscription::batch([
            subscription(),
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

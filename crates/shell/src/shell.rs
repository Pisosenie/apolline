use apolline_config::Config;
use apolline_theme::{Mode, metrics, neutral};
use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, image, space};
use iced::{ContentFit, Element, Fill, Padding, Task, window};

use crate::bar::{self, Bar};
use crate::control::{self, Command};
use crate::fragment;
use crate::niri;
use crate::power::{self, Battery};
use crate::status::Status;
use crate::{media, style, wallpaper};

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message(multi))]
#[derive(Clone, Debug)]
pub enum Message {
    Tick,
    Key(keyboard::Event),
    Mode(Mode),
    Workspace(usize),
    Captured(window::Screenshot),
    Niri(apolline_niri::Event),
    NiriReply(Result<(), String>),
    Control(Command),
    Wallpaper(Mode, Result<image::Handle, String>),
    Battery(Option<Battery>),
    Media(Option<String>),
}

#[derive(Default)]
struct System {
    battery: Option<Battery>,
    media: Option<String>,
}

pub struct Shell {
    config: Config,
    mode: Mode,
    status: Status,
    niri: Option<apolline_niri::State>,
    system: Option<System>,
    workspace: usize,
    wallpaper: Option<(Mode, image::Handle)>,
    now: DateTime<Local>,
}

impl Shell {
    pub fn new(mode: Option<Mode>) -> (Self, Task<Message>) {
        let path = apolline_config::default_path();
        let (config, diagnostics) = Config::load(&path);
        for diagnostic in &diagnostics {
            eprintln!("{}:{}: {diagnostic}", path.display(), diagnostic.line);
        }

        let mode = mode.unwrap_or(config.mode);
        let niri = niri::connected().then(apolline_niri::State::default);
        let status = if niri.is_some() {
            Status::default()
        } else {
            Status::sample(mode)
        };
        let shell = Self {
            mode,
            workspace: status.active,
            status,
            niri,
            system: None,
            config,
            wallpaper: None,
            now: Local::now(),
        };
        fragment::sync(shell.config.palette(mode).accent);
        let load = shell.load_wallpaper();
        (shell, load)
    }

    pub fn is_ready(&self) -> bool {
        self.wallpaper
            .as_ref()
            .is_some_and(|(mode, _)| *mode == self.mode)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.now = Local::now(),
            Message::Key(keyboard::Event::KeyPressed { key, .. }) => match key.as_ref() {
                Key::Character("1") => return self.switch(Mode::Work),
                Key::Character("2") => return self.switch(Mode::Play),
                Key::Character("3") => return self.switch(Mode::Evening),
                Key::Named(Named::ArrowLeft) => self.workspace = self.workspace.saturating_sub(1),
                Key::Named(Named::ArrowRight) => {
                    self.workspace =
                        (self.workspace + 1).min(self.status.workspaces.saturating_sub(1));
                }
                Key::Named(Named::Escape) => return iced::exit(),
                _ => {}
            },
            Message::Mode(mode) | Message::Control(Command::Mode(mode)) => {
                return self.switch(mode);
            }
            Message::Control(Command::NextMode) => return self.switch(self.mode.next()),
            Message::Workspace(index) => match self.status.ids.get(index) {
                Some(&id) => return niri::send(apolline_niri::Request::FocusWorkspace(id)),
                None => self.workspace = index,
            },
            Message::Niri(event) => {
                let state = self.niri.get_or_insert_default();
                state.apply(event);
                self.status = Status::from_niri(state);
                self.workspace = self.status.active;
            }
            Message::NiriReply(Err(err)) => eprintln!("apolline: niri: {err}"),
            Message::Wallpaper(mode, Ok(handle)) if mode == self.mode => {
                self.wallpaper = Some((mode, handle));
            }
            Message::Wallpaper(_, Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => self.system.get_or_insert_default().battery = battery,
            Message::Media(media) => self.system.get_or_insert_default().media = media,
            _ => {}
        }
        Task::none()
    }

    fn switch(&mut self, mode: Mode) -> Task<Message> {
        self.mode = mode;
        if self.niri.is_none() {
            self.status = Status::sample(mode);
            self.workspace = self.status.active;
        }
        fragment::sync(self.config.palette(mode).accent);
        Task::batch([self.load_wallpaper(), self.open_apps()])
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

    pub fn bar(&self) -> Element<'_, Message> {
        let (battery, media) = match &self.system {
            Some(system) => (system.battery, system.media.as_deref()),
            None => (self.status.battery, self.status.media.as_deref()),
        };
        let bar = bar::view(Bar {
            mode: self.mode,
            palette: self.config.palette(self.mode),
            active: self.workspace,
            status: &self.status,
            battery,
            media,
            now: self.now,
        });
        container(bar)
            .padding(Padding::new(metrics::BAR_INSET).bottom(0))
            .into()
    }
}

pub fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick),
        keyboard::listen().map(Message::Key),
        niri::events(),
        control::listen(),
        power::subscription(),
        media::subscription(),
    ])
}

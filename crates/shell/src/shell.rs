use apolline_config::Config;
use apolline_theme::{Mode, metrics};
use chrono::{DateTime, Local};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, image};
use iced::{ContentFit, Element, Fill, Padding, Task, window};

use crate::bar::{self, Bar};
use crate::niri;
use crate::status::Status;
use crate::wallpaper;

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
}

pub struct Shell {
    config: Config,
    mode: Mode,
    status: Status,
    niri: Option<apolline_niri::State>,
    workspace: usize,
    wallpapers: [image::Handle; 3],
    now: DateTime<Local>,
}

impl Shell {
    pub fn new(mode: Option<Mode>) -> Self {
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
        let wallpapers = Mode::ALL
            .map(|m| image::Handle::from_path(wallpaper::resolve(&config.mode(m).wallpaper)));
        Self {
            mode,
            workspace: status.active,
            status,
            niri,
            config,
            wallpapers,
            now: Local::now(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.now = Local::now(),
            Message::Key(keyboard::Event::KeyPressed { key, .. }) => match key.as_ref() {
                Key::Character("1") => self.switch(Mode::Work),
                Key::Character("2") => self.switch(Mode::Play),
                Key::Character("3") => self.switch(Mode::Evening),
                Key::Named(Named::ArrowLeft) => self.workspace = self.workspace.saturating_sub(1),
                Key::Named(Named::ArrowRight) => {
                    self.workspace =
                        (self.workspace + 1).min(self.status.workspaces.saturating_sub(1));
                }
                Key::Named(Named::Escape) => return iced::exit(),
                _ => {}
            },
            Message::Mode(mode) => self.switch(mode),
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
            _ => {}
        }
        Task::none()
    }

    fn switch(&mut self, mode: Mode) {
        self.mode = mode;
        if self.niri.is_none() {
            self.status = Status::sample(mode);
            self.workspace = self.status.active;
        }
    }

    pub fn wallpaper(&self) -> Element<'_, Message> {
        image(self.wallpapers[self.mode as usize].clone())
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .into()
    }

    pub fn bar(&self) -> Element<'_, Message> {
        let bar = bar::view(Bar {
            mode: self.mode,
            palette: self.config.palette(self.mode),
            active: self.workspace,
            status: &self.status,
            now: self.now,
        });
        container(bar)
            .padding(Padding::new(metrics::BAR_INSET).bottom(0))
            .into()
    }
}

pub fn ticks() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick),
        keyboard::listen().map(Message::Key),
        niri::events(),
    ])
}

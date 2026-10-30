use apolline_theme::{Mode, font, metrics};
use iced::widget::space;
use iced::{Color, Element, Font, Subscription, Task, Theme, window};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

use crate::shell::{self, Message, Shell};

const BAR_SURFACE: u32 = (metrics::BAR_INSET + metrics::BAR_HEIGHT) as u32;

pub fn run(mode: Option<Mode>) -> Result<(), String> {
    daemon(
        move || Layers::new(mode),
        "apolline",
        Layers::update,
        Layers::view,
    )
    .subscription(Layers::subscription)
    .theme(Theme::Dark)
    .style(|_, theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    })
    .default_font(Font::with_name(font::UI))
    .settings(Settings {
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
    .map_err(|err| err.to_string())
}

struct Layers {
    shell: Shell,
    wallpaper: window::Id,
    bar: window::Id,
}

impl Layers {
    fn new(mode: Option<Mode>) -> (Self, Task<Message>) {
        let (wallpaper, open_wallpaper) = open(NewLayerShellSettings {
            layer: Layer::Background,
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            exclusive_zone: Some(-1),
            keyboard_interactivity: KeyboardInteractivity::None,
            namespace: Some("apolline-wallpaper".into()),
            ..Default::default()
        });
        let (bar, open_bar) = open(NewLayerShellSettings {
            layer: Layer::Top,
            anchor: Anchor::Top | Anchor::Left | Anchor::Right,
            size: Some((0, BAR_SURFACE)),
            exclusive_zone: Some(BAR_SURFACE as i32),
            keyboard_interactivity: KeyboardInteractivity::None,
            namespace: Some("apolline-bar".into()),
            ..Default::default()
        });

        let (shell, load) = Shell::new(mode);
        let layers = Self {
            shell,
            wallpaper,
            bar,
        };
        (layers, Task::batch([open_wallpaper, open_bar, load]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        self.shell.update(message)
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        if id == self.wallpaper {
            self.shell.wallpaper()
        } else if id == self.bar {
            self.shell.bar()
        } else {
            space().into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        shell::subscription()
    }
}

fn open(settings: NewLayerShellSettings) -> (window::Id, Task<Message>) {
    let id = window::Id::unique();
    (id, Task::done(Message::NewLayerShell { settings, id }))
}

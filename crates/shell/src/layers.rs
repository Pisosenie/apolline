use std::collections::BTreeMap;

use apolline_theme::{Mode, font, metrics};
use iced::widget::space;
use iced::{Color, Element, Font, Subscription, Task, Theme, window};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

use crate::fonts;
use crate::popup;
use crate::shell::{self, Message, Overlay, Shell};

const BAR_SURFACE: u32 = (metrics::BAR_INSET + metrics::BAR_HEIGHT) as u32;
const POPUP_MARGIN: i32 = 8;

pub fn run(mode: Option<Mode>) -> Result<(), String> {
    let app = daemon(
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
    });
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
        .map_err(|err| err.to_string())
}

type Output = Option<String>;

struct Screen {
    wallpaper: window::Id,
    bar: window::Id,
}

struct Layers {
    shell: Shell,
    screens: BTreeMap<Output, Screen>,
    popups: Option<(window::Id, usize)>,
    overlay: Option<(window::Id, Overlay)>,
}

impl Layers {
    fn new(mode: Option<Mode>) -> (Self, Task<Message>) {
        let (shell, load) = Shell::new(mode, false);
        let mut layers = Self {
            shell,
            screens: BTreeMap::new(),
            popups: None,
            overlay: None,
        };
        let screens = layers.fit_screens();
        (layers, Task::batch([load, screens]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.shell.update(message);
        Task::batch([
            task,
            self.fit_screens(),
            self.fit_popups(),
            self.fit_overlay(),
        ])
    }

    fn fit_overlay(&mut self) -> Task<Message> {
        let wanted = self.shell.overlay();
        if self.overlay.map(|(_, kind)| kind) == wanted {
            return Task::none();
        }
        let close = match self.overlay.take() {
            Some((id, _)) => window::close(id),
            None => Task::none(),
        };
        let Some(kind) = wanted else {
            return close;
        };
        let namespace = match kind {
            Overlay::Switcher => "apolline-switcher",
            Overlay::Launcher => "apolline-launcher",
            Overlay::Panel => "apolline-panel",
        };
        let (id, open) = open(NewLayerShellSettings {
            layer: Layer::Overlay,
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            exclusive_zone: Some(-1),
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            namespace: Some(namespace.into()),
            ..Default::default()
        });
        self.overlay = Some((id, kind));
        Task::batch([close, open])
    }

    fn fit_screens(&mut self) -> Task<Message> {
        let wanted: Vec<Output> = match self.shell.outputs() {
            None => vec![None],
            Some(names) => names.iter().cloned().map(Some).collect(),
        };
        let have: Vec<Output> = self.screens.keys().cloned().collect();
        let (added, removed) = diff(&have, &wanted);

        let mut tasks = Vec::new();
        for output in removed {
            if let Some(screen) = self.screens.remove(&output) {
                tasks.push(window::close(screen.wallpaper));
                tasks.push(window::close(screen.bar));
            }
        }
        for output in added {
            let (screen, open_screen) = open_screen(&output);
            self.screens.insert(output, screen);
            tasks.push(open_screen);
        }
        Task::batch(tasks)
    }

    fn fit_popups(&mut self) -> Task<Message> {
        let count = self.shell.popup_count();
        match self.popups {
            None if count > 0 => {
                let (id, open) = open(NewLayerShellSettings {
                    layer: Layer::Overlay,
                    anchor: Anchor::Top | Anchor::Right,
                    size: Some((popup::WIDTH, popup::height(count))),
                    margin: Some((POPUP_MARGIN, POPUP_MARGIN, 0, 0)),
                    keyboard_interactivity: KeyboardInteractivity::None,
                    namespace: Some("apolline-notifications".into()),
                    ..Default::default()
                });
                self.popups = Some((id, count));
                open
            }
            Some((id, _)) if count == 0 => {
                self.popups = None;
                window::close(id)
            }
            Some((id, shown)) if shown != count => {
                self.popups = Some((id, count));
                Task::done(Message::SizeChange {
                    id,
                    size: (popup::WIDTH, popup::height(count)),
                })
            }
            _ => Task::none(),
        }
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        if self.popups.is_some_and(|(popups, _)| popups == id) {
            return self.shell.popups();
        }
        if self.overlay.is_some_and(|(overlay, _)| overlay == id) {
            return self.shell.overlay_view();
        }
        for (output, screen) in &self.screens {
            if id == screen.wallpaper {
                return self.shell.wallpaper();
            }
            if id == screen.bar {
                return self.shell.bar(output.as_deref());
            }
        }
        space().into()
    }

    fn subscription(&self) -> Subscription<Message> {
        shell::subscription()
    }
}

fn open_screen(output: &Output) -> (Screen, Task<Message>) {
    let output_option = match output {
        Some(name) => OutputOption::OutputName(name.clone()),
        None => OutputOption::default(),
    };
    let (wallpaper, open_wallpaper) = open(NewLayerShellSettings {
        layer: Layer::Background,
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        exclusive_zone: Some(-1),
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output_option.clone(),
        namespace: Some("apolline-wallpaper".into()),
        ..Default::default()
    });
    let (bar, open_bar) = open(NewLayerShellSettings {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: Some((0, BAR_SURFACE)),
        exclusive_zone: Some(BAR_SURFACE as i32),
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option,
        namespace: Some("apolline-bar".into()),
        ..Default::default()
    });
    (
        Screen { wallpaper, bar },
        Task::batch([open_wallpaper, open_bar]),
    )
}

fn diff<T: Clone + PartialEq>(have: &[T], wanted: &[T]) -> (Vec<T>, Vec<T>) {
    let added = wanted
        .iter()
        .filter(|w| !have.contains(w))
        .cloned()
        .collect();
    let removed = have
        .iter()
        .filter(|h| !wanted.contains(h))
        .cloned()
        .collect();
    (added, removed)
}

fn open(settings: NewLayerShellSettings) -> (window::Id, Task<Message>) {
    let id = window::Id::unique();
    (id, Task::done(Message::NewLayerShell { settings, id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_follow_monitors_coming_and_going() {
        let have = ["eDP-1", "DP-1"].map(|s| Some(s.to_owned()));
        let wanted = ["eDP-1", "HDMI-A-1"].map(|s| Some(s.to_owned()));
        let (added, removed) = diff(&have, &wanted);
        assert_eq!(added, [Some("HDMI-A-1".to_owned())]);
        assert_eq!(removed, [Some("DP-1".to_owned())]);
        assert_eq!(diff::<Output>(&[None], &[None]), (vec![], vec![]));
    }
}

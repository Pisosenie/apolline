use std::collections::BTreeMap;

use apolline_theme::metrics;
use iced::widget::space;
use iced::{Color, Element, Font, Subscription, Task, Theme, mouse, window};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

use crate::fonts;
use crate::shell::{Message, Overlay, Shell};
use apolline_config::Edge;

use crate::popup::{Felt, Hold, Nudge};
use crate::{osd, popup};

const POPUP_MARGIN: i32 = 8;

pub fn run(mode: Option<String>) -> Result<(), String> {
    let app = daemon(
        move || Layers::new(mode.clone()),
        "apolline",
        Layers::update,
        Layers::view,
    )
    .subscription(Layers::subscription)
    .scale_factor(Layers::scale)
    .theme(Theme::Dark)
    .style(|_, theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    })
    .default_font(Font::with_name(crate::style::family()))
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
    look: (Edge, bool),
    popups: Option<(window::Id, u32)>,
    popups_hold: Hold,
    overlay: Option<(window::Id, Overlay)>,
    osd: Option<window::Id>,
}

impl Layers {
    fn new(mode: Option<String>) -> (Self, Task<Message>) {
        let (shell, load) = Shell::new(mode, false);
        let mut layers = Self {
            shell,
            screens: BTreeMap::new(),
            look: (Edge::Top, true),
            popups: None,
            popups_hold: Hold::default(),
            overlay: None,
            osd: None,
        };
        let screens = layers.fit_screens();
        (layers, Task::batch([load, screens]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let message = match message {
            Message::LayerEvent(id, event) if self.popups.is_some_and(|(p, _)| p == id) => {
                let (held, nudge) = self.popups_felt(id, &event);
                let next = match nudge {
                    Some(Nudge::Unfocused) => self.update(Message::ReplyUnfocused),
                    Some(Nudge::Refocus) => self.update(Message::ReplyRefocus),
                    None => self.fit_popups(),
                };
                return Task::batch([held, next]);
            }
            Message::LayerEvent(..) => return Task::none(),
            other => other,
        };
        let task = self.shell.update(message);
        Task::batch([
            task,
            self.fit_screens(),
            self.fit_popups(),
            self.fit_overlay(),
            self.fit_osd(),
        ])
    }

    fn fit_osd(&mut self) -> Task<Message> {
        match (self.osd, self.shell.osd_view().is_some()) {
            (None, true) => {
                let (id, open) = open(NewLayerShellSettings {
                    layer: Layer::Overlay,
                    anchor: Anchor::Bottom,
                    size: Some(osd::SURFACE),
                    margin: Some((0, 0, 56, 0)),
                    keyboard_interactivity: KeyboardInteractivity::None,
                    namespace: Some("apolline-osd".into()),
                    ..Default::default()
                });
                self.osd = Some(id);
                open
            }
            (Some(id), false) => {
                self.osd = None;
                window::close(id)
            }
            _ => Task::none(),
        }
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
            Overlay::Clipboard => "apolline-clipboard",
            Overlay::Keys => "apolline-keys",
            Overlay::Polkit => "apolline-polkit",
            Overlay::Calendar => "apolline-calendar",
            Overlay::Marks => "apolline-marks",
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
        let mut have: Vec<Output> = self.screens.keys().cloned().collect();
        let look = self.shell.bar_look();
        let mut tasks = Vec::new();
        if look != self.look {
            self.look = look;
            for (_, screen) in std::mem::take(&mut self.screens) {
                tasks.push(window::close(screen.wallpaper));
                tasks.push(window::close(screen.bar));
            }
            have.clear();
        }
        let (added, removed) = diff(&have, &wanted);

        for output in removed {
            if let Some(screen) = self.screens.remove(&output) {
                tasks.push(window::close(screen.wallpaper));
                tasks.push(window::close(screen.bar));
            }
        }
        for output in added {
            let (screen, open_screen) = open_screen(&output, look);
            self.screens.insert(output, screen);
            tasks.push(open_screen);
        }
        Task::batch(tasks)
    }

    fn fit_popups(&mut self) -> Task<Message> {
        let size = self.size_popups();
        let Some((id, _)) = self.popups else {
            return size;
        };
        let next = self
            .popups_hold
            .settle(self.shell.replying(), self.shell.answerable());
        Task::batch([size, self.hold_popups(id, next)])
    }

    fn popups_felt(
        &mut self,
        id: window::Id,
        event: &iced::Event,
    ) -> (Task<Message>, Option<Nudge>) {
        use iced::Event::{Mouse, Window};
        let felt = match event {
            Mouse(mouse::Event::CursorEntered) => Felt::PointerIn,
            Mouse(mouse::Event::CursorLeft) => Felt::PointerOut,
            Mouse(mouse::Event::ButtonReleased(_)) => Felt::Click,
            Window(window::Event::Unfocused) => Felt::KeysLost,
            _ => return (Task::none(), None),
        };
        let (next, nudge) = self.popups_hold.felt(felt);
        (self.hold_popups(id, next), nudge)
    }

    fn hold_popups(&mut self, id: window::Id, next: Hold) -> Task<Message> {
        let changed = next.on_demand() != self.popups_hold.on_demand();
        self.popups_hold = next;
        if !changed {
            return Task::none();
        }
        Task::done(Message::KeyboardInteractivityChange {
            id,
            keyboard_interactivity: if next.on_demand() {
                KeyboardInteractivity::OnDemand
            } else {
                KeyboardInteractivity::None
            },
        })
    }

    fn size_popups(&mut self) -> Task<Message> {
        let height = self.shell.popup_height();
        match self.popups {
            None if height > 0 => {
                let (id, open) = open(NewLayerShellSettings {
                    layer: Layer::Overlay,
                    anchor: Anchor::Top | Anchor::Right,
                    size: Some((popup::WIDTH, height)),
                    margin: Some((POPUP_MARGIN, POPUP_MARGIN, 0, 0)),
                    keyboard_interactivity: KeyboardInteractivity::None,
                    namespace: Some("apolline-notifications".into()),
                    ..Default::default()
                });
                self.popups = Some((id, height));
                self.popups_hold = Hold::default();
                open
            }
            Some((id, _)) if height == 0 => {
                self.popups = None;
                window::close(id)
            }
            Some((id, shown)) if shown != height => {
                self.popups = Some((id, height));
                Task::done(Message::SizeChange {
                    id,
                    size: (popup::WIDTH, height),
                })
            }
            _ => Task::none(),
        }
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        if self.popups.is_some_and(|(popups, _)| popups == id) {
            return self.shell.popups();
        }
        if self.osd == Some(id) {
            return self.shell.osd_view().unwrap_or_else(|| space().into());
        }
        if self.overlay.is_some_and(|(overlay, _)| overlay == id) {
            return self.shell.overlay_view();
        }
        for (output, screen) in &self.screens {
            if id == screen.wallpaper {
                return self.shell.wallpaper(output.as_deref());
            }
            if id == screen.bar {
                return self.shell.bar(output.as_deref());
            }
        }
        space().into()
    }

    fn scale(&self, id: window::Id) -> f32 {
        match self.overlay {
            Some((overlay, kind)) if overlay == id => self.shell.size_of(kind),
            _ => 1.0,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.shell.subscription(),
            iced::event::listen_with(|event, _, id| match event {
                iced::Event::Window(window::Event::Unfocused)
                | iced::Event::Mouse(
                    mouse::Event::CursorEntered
                    | mouse::Event::CursorLeft
                    | mouse::Event::ButtonReleased(_),
                ) => Some(Message::LayerEvent(id, event)),
                _ => None,
            }),
        ])
    }
}

fn open_screen(output: &Output, (edge, floating): (Edge, bool)) -> (Screen, Task<Message>) {
    let height = metrics::BAR_HEIGHT as u32;
    let inset = if floating {
        metrics::BAR_INSET as i32
    } else {
        0
    };
    let margin = match edge {
        Edge::Top => (inset, inset, 0, inset),
        Edge::Bottom => (0, inset, inset, inset),
    };
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
        anchor: match edge {
            Edge::Top => Anchor::Top,
            Edge::Bottom => Anchor::Bottom,
        } | Anchor::Left
            | Anchor::Right,
        size: Some((0, height)),
        exclusive_zone: Some(height as i32),
        margin: Some(margin),
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

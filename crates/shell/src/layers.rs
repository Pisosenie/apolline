use std::collections::BTreeMap;

use apolline_theme::metrics;
use iced::widget::space;
use iced::{Color, Element, Font, Subscription, Task, mouse, window};
use iced_layershell::actions::ActionCallback;
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption, WlRegion,
};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

use crate::fonts;
use crate::shell::{Message, Overlay, Shell};
use apolline_config::{Corner, Edge};

use crate::osd;
use crate::popup::{Felt, Hold, Nudge};

const POPUP_MARGIN: i32 = 8;
// What a hidden bar feels the pointer through: a strip at the screen edge.
const STRIP: i32 = 2;
// A leaving that only asks for the strip to be set again, once a new bar's surface is up.
const SETTLED: u64 = u64::MAX;
const HIDES_AFTER: std::time::Duration = std::time::Duration::from_millis(700);

pub fn run(mode: Option<String>) -> Result<(), String> {
    let app = daemon(
        move || Layers::new(mode.clone()),
        "apolline",
        Layers::update,
        Layers::view,
    )
    .subscription(Layers::subscription)
    .scale_factor(Layers::scale)
    .theme(|_: &Layers, _| crate::style::theme())
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
    // None on a screen [bar] screens leaves out.
    bar: Option<window::Id>,
}

struct Layers {
    shell: Shell,
    screens: BTreeMap<Output, Screen>,
    look: (Edge, bool, u16, bool),
    // Whether another program draws the wallpaper, which puts the shell's desktop a layer up.
    elsewhere: bool,
    // Bars out on their screens, and the ones the pointer is over with when it last left.
    out: BTreeMap<window::Id, bool>,
    over: BTreeMap<window::Id, Option<u64>>,
    leavings: u64,
    popups: Option<(window::Id, u32)>,
    // Where the cards' window was opened, and at what size; either changing opens it again.
    popups_place: (Corner, f32, u16),
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
            look: (Edge::Top, true, 100, false),
            elsewhere: false,
            out: BTreeMap::new(),
            over: BTreeMap::new(),
            leavings: 0,
            popups: None,
            popups_place: (Corner::TopRight, 1.0, 360),
            popups_hold: Hold::default(),
            overlay: None,
            osd: None,
        };
        let screens = layers.fit_screens();
        (layers, Task::batch([load, screens]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let message = match message {
            // The keyboard leaving is told to whichever surface was last clicked or opened, so
            // while an answer holds it any window's word counts.
            Message::LayerEvent(_, iced::Event::Window(window::Event::Unfocused))
                if self.popups_hold.holds() =>
            {
                let Some((id, _)) = self.popups else {
                    return Task::none();
                };
                let (held, nudge) =
                    self.popups_felt(id, &iced::Event::Window(window::Event::Unfocused));
                let next = match nudge {
                    Some(Nudge::Unfocused) => self.update(Message::ReplyUnfocused),
                    _ => self.fit_popups(),
                };
                return Task::batch([held, next]);
            }
            Message::LayerEvent(id, event) if self.popups.is_some_and(|(p, _)| p == id) => {
                let (held, nudge) = self.popups_felt(id, &event);
                let next = match nudge {
                    Some(Nudge::Unfocused) => self.update(Message::ReplyUnfocused),
                    Some(Nudge::Refocus) => self.update(Message::ReplyRefocus),
                    None => self.fit_popups(),
                };
                return Task::batch([held, next]);
            }
            Message::LayerEvent(id, iced::Event::Mouse(mouse::Event::CursorEntered))
                if self.is_bar(id) =>
            {
                self.over.insert(id, None);
                return self.fit_bars();
            }
            Message::LayerEvent(id, iced::Event::Mouse(mouse::Event::CursorLeft))
                if self.is_bar(id) =>
            {
                self.leavings += 1;
                let leaving = self.leavings;
                self.over.insert(id, Some(leaving));
                return Task::perform(tokio::time::sleep(HIDES_AFTER), move |()| {
                    Message::HideBar(id, leaving)
                });
            }
            Message::HideBar(id, SETTLED) => {
                self.out.remove(&id);
                return self.fit_bars();
            }
            Message::HideBar(id, leaving) => {
                if self.over.get(&id) == Some(&Some(leaving)) {
                    self.over.remove(&id);
                }
                return self.fit_bars();
            }
            Message::LayerEvent(..) => return Task::none(),
            other => other,
        };
        let task = self.shell.update(message);
        Task::batch([
            task,
            self.fit_screens(),
            self.fit_bars(),
            self.fit_popups(),
            self.fit_overlay(),
            self.fit_osd(),
        ])
    }

    fn is_bar(&self, id: window::Id) -> bool {
        self.screens.values().any(|screen| screen.bar == Some(id))
    }

    // A hidden bar is a clear surface that feels the pointer only along the screen edge. It
    // comes out while the pointer is on it or something opened from it is open, over the
    // windows, and the whole of it takes the pointer then.
    fn fit_bars(&mut self) -> Task<Message> {
        let (edge, floating, size, autohide) = self.look;
        let in_use = self.shell.bar_in_use();
        let bars: Vec<window::Id> = self.screens.values().filter_map(|s| s.bar).collect();
        self.over.retain(|id, _| bars.contains(id));
        self.out.retain(|id, _| bars.contains(id));
        let thick = (thickness(edge, size) + gap(floating)) as i32;
        let mut tasks = Vec::new();
        for id in bars {
            let wanted = !autohide || in_use || self.over.contains_key(&id);
            if self.out.insert(id, wanted) == Some(wanted) || !autohide {
                continue;
            }
            let callback = move |region: &WlRegion| {
                let far = i32::MAX / 4;
                match (wanted, edge) {
                    (true, _) => region.add(0, 0, far, far),
                    (false, Edge::Top) => region.add(0, 0, far, STRIP),
                    (false, Edge::Bottom) => region.add(0, thick - STRIP, far, STRIP),
                    (false, Edge::Left) => region.add(0, 0, STRIP, far),
                    (false, Edge::Right) => region.add(thick - STRIP, 0, STRIP, far),
                }
            };
            tasks.push(Task::done(Message::SetInputRegion {
                id,
                callback: ActionCallback(std::sync::Arc::new(callback)),
            }));
        }
        Task::batch(tasks)
    }

    fn fit_osd(&mut self) -> Task<Message> {
        match (self.osd, self.shell.osd_view().is_some()) {
            (None, true) => {
                let grow = |n: u32| (n as f32 * self.shell.osd_size()).round() as u32;
                let (id, open) = open(NewLayerShellSettings {
                    layer: Layer::Overlay,
                    anchor: Anchor::Bottom,
                    size: Some((grow(osd::SURFACE.0), grow(osd::SURFACE.1))),
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
            Overlay::Machine => "apolline-machine",
            Overlay::TrayMenu => "apolline-tray-menu",
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

    // Each screen with its wallpaper, and its bar where it has one; a screen whose bar comes or
    // goes is opened again.
    fn fit_screens(&mut self) -> Task<Message> {
        let names: Vec<Output> = match self.shell.outputs() {
            None => vec![None],
            Some(names) => names.iter().cloned().map(Some).collect(),
        };
        let mut wanted: Vec<(Output, bool)> = names
            .into_iter()
            .map(|output| {
                let bar = self.shell.bar_on(output.as_deref());
                (output, bar)
            })
            .collect();
        // Never no bar at all, say with the lid shut on the one screen that had it.
        if wanted.iter().all(|(_, bar)| !bar) {
            for (_, bar) in &mut wanted {
                *bar = true;
            }
        }
        let mut have: Vec<(Output, bool)> = self
            .screens
            .iter()
            .map(|(output, screen)| (output.clone(), screen.bar.is_some()))
            .collect();
        let look = self.shell.bar_look();
        let elsewhere = self.shell.wallpaper_elsewhere();
        let mut tasks = Vec::new();
        if look != self.look || elsewhere != self.elsewhere {
            self.look = look;
            self.elsewhere = elsewhere;
            for (_, screen) in std::mem::take(&mut self.screens) {
                tasks.extend(close_screen(screen));
            }
            have.clear();
        }
        let (added, removed) = diff(&have, &wanted);

        for (output, _) in removed {
            if let Some(screen) = self.screens.remove(&output) {
                tasks.extend(close_screen(screen));
            }
        }
        for (output, with_bar) in added {
            let (screen, open_screen) = open_screen(&output, look, with_bar, elsewhere);
            if elsewhere {
                tasks.extend(pass_through(screen.wallpaper));
            }
            if let Some(bar) = screen.bar.filter(|_| look.3) {
                tasks.push(Task::perform(
                    tokio::time::sleep(std::time::Duration::from_millis(300)),
                    move |()| Message::HideBar(bar, SETTLED),
                ));
            }
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
        let place = self.shell.cards_place();
        let scale = place.1;
        let size = |height: u32| {
            let grow = |n: u32| (n as f32 * scale).round() as u32;
            (grow(u32::from(place.2)), grow(height))
        };
        if let Some((id, _)) = self.popups.filter(|_| place != self.popups_place) {
            self.popups = None;
            return Task::batch([window::close(id), self.size_popups_at(place, height, size)]);
        }
        match self.popups {
            None if height > 0 => self.size_popups_at(place, height, size),
            Some((id, _)) if height == 0 => {
                self.popups = None;
                window::close(id)
            }
            Some((id, shown)) if shown != height => {
                self.popups = Some((id, height));
                Task::done(Message::SizeChange {
                    id,
                    size: size(height),
                })
            }
            _ => Task::none(),
        }
    }

    fn size_popups_at(
        &mut self,
        place: (Corner, f32, u16),
        height: u32,
        size: impl Fn(u32) -> (u32, u32),
    ) -> Task<Message> {
        if height == 0 {
            return Task::none();
        }
        let m = POPUP_MARGIN;
        let (anchor, margin) = match place.0 {
            Corner::TopRight => (Anchor::Top | Anchor::Right, (m, m, 0, 0)),
            Corner::Top => (Anchor::Top, (m, 0, 0, 0)),
            Corner::TopLeft => (Anchor::Top | Anchor::Left, (m, 0, 0, m)),
            Corner::BottomRight => (Anchor::Bottom | Anchor::Right, (0, m, m, 0)),
            Corner::Bottom => (Anchor::Bottom, (0, 0, m, 0)),
            Corner::BottomLeft => (Anchor::Bottom | Anchor::Left, (0, 0, m, m)),
        };
        let (id, open) = open(NewLayerShellSettings {
            layer: Layer::Overlay,
            anchor,
            size: Some(size(height)),
            margin: Some(margin),
            keyboard_interactivity: KeyboardInteractivity::None,
            namespace: Some("apolline-notifications".into()),
            ..Default::default()
        });
        self.popups = Some((id, height));
        self.popups_place = place;
        self.popups_hold = Hold::default();
        open
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
            if Some(id) == screen.bar {
                if self.out.get(&id) == Some(&false) {
                    return space().into();
                }
                let (edge, floating, size, autohide) = self.look;
                if !autohide {
                    return self.shell.bar(output.as_deref());
                }
                // The bar draws at its own size, the gap is in the screen's points.
                let inset = gap(floating) as f32 * 100.0 / f32::from(size);
                let padding = match edge {
                    Edge::Top => iced::Padding::ZERO.top(inset),
                    Edge::Bottom => iced::Padding::ZERO.bottom(inset),
                    Edge::Left => iced::Padding::ZERO.left(inset),
                    Edge::Right => iced::Padding::ZERO.right(inset),
                };
                return iced::widget::container(self.shell.bar(output.as_deref()))
                    .padding(padding)
                    .into();
            }
        }
        space().into()
    }

    fn scale(&self, id: window::Id) -> f32 {
        match self.overlay {
            Some((overlay, kind)) if overlay == id => self.shell.size_of(kind),
            _ if self.is_bar(id) => f32::from(self.look.2) / 100.0,
            _ if self.popups.is_some_and(|(popups, _)| popups == id) => self.popups_place.1,
            _ if self.osd == Some(id) => self.shell.osd_size(),
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

// How thick the bar is across its edge: tall along the top, wide down a side.
fn thickness(edge: Edge, size: u16) -> u32 {
    let scale = f32::from(size) / 100.0;
    if edge.vertical() {
        (crate::style::RAIL * scale).round() as u32
    } else {
        (metrics::BAR_HEIGHT * scale).round() as u32
    }
}

fn bar_size(edge: Edge, size: u16) -> (u32, u32) {
    across(edge, thickness(edge, size))
}

// A thickness across the edge, the length left to the anchors.
fn across(edge: Edge, thick: u32) -> (u32, u32) {
    if edge.vertical() {
        (thick, 0)
    } else {
        (0, thick)
    }
}

fn gap(floating: bool) -> u32 {
    if floating {
        metrics::BAR_INSET as u32
    } else {
        0
    }
}

// The margin without the one along the edge the bar sits against.
fn edge_free((top, right, bottom, left): (i32, i32, i32, i32), edge: Edge) -> (i32, i32, i32, i32) {
    match edge {
        Edge::Top => (0, right, bottom, left),
        Edge::Bottom => (top, right, 0, left),
        Edge::Left => (top, right, bottom, 0),
        Edge::Right => (top, 0, bottom, left),
    }
}

// Top, right, bottom, left; nothing on the screen edge it sits against.
fn bar_margin(edge: Edge, floating: bool) -> (i32, i32, i32, i32) {
    let inset = gap(floating) as i32;
    match edge {
        Edge::Top => (inset, inset, 0, inset),
        Edge::Bottom => (0, inset, inset, inset),
        Edge::Left => (inset, 0, inset, inset),
        Edge::Right => (inset, inset, inset, 0),
    }
}

// A hidden bar opens as its strip and takes no room from the windows.
fn close_screen(screen: Screen) -> impl Iterator<Item = Task<Message>> {
    std::iter::once(window::close(screen.wallpaper)).chain(screen.bar.map(window::close))
}

// Clicks on the desktop go to the program drawing the wallpaper below; once now, and again once
// the surface surely exists.
fn pass_through(id: window::Id) -> [Task<Message>; 2] {
    let none = move || Message::SetInputRegion {
        id,
        callback: ActionCallback(std::sync::Arc::new(|_: &WlRegion| {})),
    };
    [
        Task::done(none()),
        Task::perform(
            tokio::time::sleep(std::time::Duration::from_millis(300)),
            move |()| none(),
        ),
    ]
}

fn open_screen(
    output: &Output,
    (edge, floating, size, autohide): (Edge, bool, u16, bool),
    with_bar: bool,
    elsewhere: bool,
) -> (Screen, Task<Message>) {
    let (extent, margin, zone) = if autohide {
        (
            across(edge, thickness(edge, size) + gap(floating)),
            edge_free(bar_margin(edge, floating), edge),
            0,
        )
    } else {
        (
            bar_size(edge, size),
            bar_margin(edge, floating),
            thickness(edge, size) as i32,
        )
    };
    let output_option = match output {
        Some(name) => OutputOption::OutputName(name.clone()),
        None => OutputOption::default(),
    };
    let (wallpaper, open_wallpaper) = open(NewLayerShellSettings {
        layer: if elsewhere {
            Layer::Bottom
        } else {
            Layer::Background
        },
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        exclusive_zone: Some(-1),
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output_option.clone(),
        namespace: Some("apolline-wallpaper".into()),
        ..Default::default()
    });
    if !with_bar {
        let screen = Screen {
            wallpaper,
            bar: None,
        };
        return (screen, open_wallpaper);
    }
    let (bar, open_bar) = open(NewLayerShellSettings {
        layer: Layer::Top,
        anchor: match edge {
            Edge::Top => Anchor::Top | Anchor::Left | Anchor::Right,
            Edge::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
            Edge::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
            Edge::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
        },
        size: Some(extent),
        exclusive_zone: Some(zone),
        margin: Some(margin),
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option,
        namespace: Some("apolline-bar".into()),
        ..Default::default()
    });
    (
        Screen {
            wallpaper,
            bar: Some(bar),
        },
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

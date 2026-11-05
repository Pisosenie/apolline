use std::collections::BTreeMap;

use apolline_config::Edge;
use iced::{Task, mouse, window};
use iced_layershell::actions::ActionCallback;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption, WlRegion,
};

use super::{HIDES_AFTER, Layers, Output, SETTLED, open};
use crate::dock::Event;
use crate::dock_shape::{self, Shape};
use crate::shell::Message;

// The docks out on their screens, and what's known of the pointer on each.
#[derive(Default)]
pub(super) struct Docks {
    on: BTreeMap<Output, window::Id>,
    // Where they sit, whether they hide, and their icons' size; any change opens them again.
    look: Option<(Edge, bool, u16)>,
    surface: (u32, u32),
    // The ones the pointer is on, or left with this leaving, and what of each takes the pointer.
    over: BTreeMap<window::Id, Option<u64>>,
    leavings: u64,
    shown: BTreeMap<window::Id, bool>,
    regions: BTreeMap<window::Id, (i32, i32, i32, i32)>,
}

impl Docks {
    pub(super) fn has(&self, id: window::Id) -> bool {
        self.on.values().any(|dock| *dock == id)
    }

    pub(super) fn output(&self, id: window::Id) -> Option<&Output> {
        self.on
            .iter()
            .find(|(_, dock)| **dock == id)
            .map(|(output, _)| output)
    }

    // Whether a hiding dock is out; it draws nothing while it waits at the edge.
    pub(super) fn is_out(&self, id: window::Id) -> bool {
        self.shown.get(&id) != Some(&false)
    }
}

impl Layers {
    // What a dock hears of the pointer coming and going, or None when it isn't the dock's.
    pub(super) fn dock_felt(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::LayerEvent(id, iced::Event::Mouse(mouse::Event::CursorEntered))
                if self.docks.has(*id) =>
            {
                self.docks.over.insert(*id, None);
                Some(self.fit_docks())
            }
            Message::LayerEvent(id, iced::Event::Mouse(mouse::Event::CursorLeft))
                if self.docks.has(*id) =>
            {
                let id = *id;
                if !self.docks.look.is_some_and(|(_, autohide, _)| autohide) {
                    self.docks.over.remove(&id);
                    return Some(self.fit_docks());
                }
                self.docks.leavings += 1;
                let leaving = self.docks.leavings;
                self.docks.over.insert(id, Some(leaving));
                Some(Task::perform(tokio::time::sleep(HIDES_AFTER), move |()| {
                    Message::Dock(Event::Hide(id, leaving))
                }))
            }
            Message::Dock(Event::Hide(id, SETTLED)) => {
                self.docks.regions.remove(id);
                Some(self.fit_docks())
            }
            Message::Dock(Event::Hide(id, leaving)) => {
                if self.docks.over.get(id) == Some(&Some(*leaving)) {
                    self.docks.over.remove(id);
                }
                Some(self.fit_docks())
            }
            _ => None,
        }
    }

    // A dock on each screen that wants one, sized to its icons, feeling the pointer only where
    // it is or, hidden, along the edge.
    pub(super) fn fit_docks(&mut self) -> Task<Message> {
        let (edge, autohide, shape) = self.shell.dock_look();
        let wanted: Vec<Output> = self
            .outputs()
            .into_iter()
            .filter(|output| shape.count > 0 && self.shell.dock_on(output.as_deref()))
            .collect();
        let look = (edge, autohide, shape.size as u16);
        let surface = shape.surface();
        let size = (surface.0 as u32, surface.1 as u32);
        let mut tasks = Vec::new();
        if self.docks.look != Some(look) {
            self.docks.look = Some(look);
            tasks.extend(self.close_docks(|_| true));
        } else if self.docks.surface != size {
            tasks.extend(self.docks.on.values().map(|id| {
                Task::done(Message::SizeChange {
                    id: *id,
                    size: across_edge(edge, size),
                })
            }));
        }
        self.docks.surface = size;
        tasks.extend(self.close_docks(|output| !wanted.contains(output)));
        for output in wanted {
            if !self.docks.on.contains_key(&output) {
                tasks.push(self.open_dock(output, edge, autohide, shape));
            }
        }
        tasks.push(self.fit_dock_regions(edge, autohide, shape));
        Task::batch(tasks)
    }

    fn close_docks(&mut self, gone: impl Fn(&Output) -> bool) -> Vec<Task<Message>> {
        let closing: Vec<Output> = self.docks.on.keys().filter(|o| gone(o)).cloned().collect();
        closing
            .into_iter()
            .filter_map(|output| self.docks.on.remove(&output))
            .map(|id| {
                self.docks.over.remove(&id);
                self.docks.shown.remove(&id);
                self.docks.regions.remove(&id);
                window::close(id)
            })
            .collect()
    }

    fn open_dock(
        &mut self,
        output: Output,
        edge: Edge,
        autohide: bool,
        shape: Shape,
    ) -> Task<Message> {
        let (along, across) = shape.surface();
        let (id, opened) = open(NewLayerShellSettings {
            layer: Layer::Top,
            anchor: match edge {
                Edge::Top => Anchor::Top,
                Edge::Bottom => Anchor::Bottom,
                Edge::Left => Anchor::Left,
                Edge::Right => Anchor::Right,
            },
            size: Some(across_edge(edge, (along as u32, across as u32))),
            exclusive_zone: Some(if autohide {
                0
            } else {
                shape.zone().ceil() as i32
            }),
            keyboard_interactivity: KeyboardInteractivity::None,
            output_option: match &output {
                Some(name) => OutputOption::OutputName(name.clone()),
                None => OutputOption::default(),
            },
            namespace: Some("apolline-dock".into()),
            ..Default::default()
        });
        self.docks.on.insert(output, id);
        // The region is set again once the surface is surely up.
        let settled = Task::perform(
            tokio::time::sleep(std::time::Duration::from_millis(300)),
            move |()| Message::Dock(Event::Hide(id, SETTLED)),
        );
        Task::batch([opened, settled])
    }

    fn fit_dock_regions(&mut self, edge: Edge, autohide: bool, shape: Shape) -> Task<Message> {
        let in_use = self.shell.dock_in_use();
        let surface = shape.surface();
        let docks: Vec<window::Id> = self.docks.on.values().copied().collect();
        let mut tasks = Vec::new();
        for id in docks {
            let hovered = self.docks.over.get(&id) == Some(&None);
            let shown = !autohide || in_use || self.docks.over.contains_key(&id);
            self.docks.shown.insert(id, shown);
            let reach = dock_shape::input(shape, surface, shown, hovered || (shown && in_use));
            let rect = dock_shape::rect(edge, surface, reach);
            if self.docks.regions.insert(id, rect) == Some(rect) {
                continue;
            }
            let callback = move |region: &WlRegion| {
                let (x, y, width, height) = rect;
                region.add(x, y, width, height);
            };
            tasks.push(Task::done(Message::SetInputRegion {
                id,
                callback: ActionCallback(std::sync::Arc::new(callback)),
            }));
        }
        Task::batch(tasks)
    }
}

// A surface's width and height from its length along the edge and its depth away from it.
fn across_edge(edge: Edge, (along, across): (u32, u32)) -> (u32, u32) {
    if edge.vertical() {
        (across, along)
    } else {
        (along, across)
    }
}

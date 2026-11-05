use super::*;
use crate::dock::{self, Event, Item};
use crate::dock_shape::{self, Shape};

// Turns of a touchpad come many to a gesture; one window per this long.
const SCROLL_EVERY: Duration = Duration::from_millis(220);

impl Shell {
    // Every window as the dock sees it, by the app it belongs to.
    fn dock_windows(&self) -> Vec<(String, dock::Window)> {
        let key = |app: &str| bar::app_key(&self.apps, app).to_owned();
        let windows: Vec<(String, dock::Window)> = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .map(|w| {
                    let window = dock::Window {
                        id: w.id,
                        title: w.title.clone().unwrap_or_default(),
                        focused: w.is_focused,
                        urgent: w.is_urgent,
                    };
                    (key(w.app_id.as_deref().unwrap_or_default()), window)
                })
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .map(|(id, app, focused)| {
                    let window = dock::Window {
                        id: *id,
                        title: app.clone(),
                        focused: *focused,
                        urgent: false,
                    };
                    (key(app), window)
                })
                .collect(),
        };
        windows
            .into_iter()
            .filter(|(key, _)| !key.is_empty())
            .collect()
    }

    pub(super) fn dock_items(&self, pinned: &[String]) -> Vec<Item> {
        dock::items(pinned, self.dock_windows())
            .into_iter()
            .map(|item| Item {
                unread: self.unread(&item.key),
                ..item
            })
            .collect()
    }

    // What the app sent that's still in the history and came after it was last in front.
    fn unread(&self, key: &str) -> usize {
        if self.dock.is_front(key) {
            return 0;
        }
        let mut names = vec![key];
        if let Some(app) = apps::find(&self.apps, key) {
            names.extend([app.id.as_str(), app.name.as_str()]);
            names.extend(app.class.as_deref());
        }
        let sent = self
            .history
            .iter()
            .filter(|n| dock::sent_by(&names, n.desktop.as_deref(), &n.app))
            .map(|n| n.received);
        dock::unread(sent, self.dock.seen(key))
    }

    // The pinned apps as shown: as a drag would leave them while one is under way.
    fn dock_pinned(&self, shape: Shape) -> Vec<String> {
        let Some(drag) = &self.dock.dragging else {
            return self.config.pinned.clone();
        };
        let (along, away) = self.dock_point(shape, drag.at);
        dock::landing(
            &self.config.pinned,
            &drag.key,
            drag.was_pinned,
            shape,
            along,
            away,
        )
    }

    // A point on the dock's surface as how far along the resting row it is, and how far past
    // the glass away from the screen edge.
    fn dock_point(&self, shape: Shape, at: iced::Point) -> (f32, f32) {
        let surface = shape.surface();
        let (along, from_edge) = dock_shape::local(self.config.dock.position, surface.1, at);
        (along - shape.row_start(surface.0), from_edge - shape.zone())
    }

    pub(super) fn dock_shape(&self, items: &[Item]) -> Shape {
        let pinned = items.iter().filter(|item| item.pinned).count();
        Shape {
            size: f32::from(self.config.dock.size),
            count: items.len(),
            split: (pinned > 0 && pinned < items.len()).then_some(pinned),
            sideways: self.config.dock.position.vertical(),
        }
    }

    // The shape of the dock as it rests, before any drag moves things.
    fn resting_shape(&self) -> Shape {
        self.dock_shape(&dock::items(&self.config.pinned, self.dock_windows()))
    }

    pub fn dock_edge(&self) -> Edge {
        self.config.dock.position
    }

    pub fn dock_on(&self, output: Option<&str>) -> bool {
        self.config.dock.shown_on(output)
    }

    // A hidden dock stays out while its menu is open or something is being dragged on it.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn dock_in_use(&self) -> bool {
        self.overlay == Some(Overlay::DockMenu) || self.dock.dragging.is_some()
    }

    #[cfg(target_os = "linux")]
    // Where the dock sits, whether it hides, and its shape now.
    pub fn dock_look(&self) -> (Edge, bool, Shape) {
        let dock = &self.config.dock;
        (dock.position, dock.autohide, self.resting_shape())
    }

    pub fn dock(&self, output: Option<&str>) -> Element<'_, Message> {
        style::within(self.config.style.dock.font, || self.built_dock(output))
    }

    fn built_dock(&self, output: Option<&str>) -> Element<'_, Message> {
        let rest = self.resting_shape();
        let items = self.dock_items(&self.dock_pinned(rest));
        let shape = self.dock_shape(&items);
        let pointer = self
            .dock
            .pointer
            .as_ref()
            .filter(|(over, _)| over.as_deref() == output)
            .map(|(_, along)| *along);
        let look = &self.config.style.dock;
        crate::dock_view::view(crate::dock_view::Look {
            items,
            apps: &self.apps,
            palette: self.config.palette(self.mode),
            edge: self.config.dock.position,
            shape,
            surface: rest.surface(),
            pointer,
            strength: if self.config.dock.magnify {
                self.dock.strength()
            } else {
                0.0
            },
            opacity: look.opacity.unwrap_or(self.config.bar.opacity),
            radius: look.radius.map_or(crate::dock_view::RADIUS, f32::from),
            output: output.map(str::to_owned),
            held: self.dock.dragging.as_ref().map(|d| d.key.clone()),
        })
    }

    pub(super) fn dock_menu(&self) -> Element<'_, Message> {
        let Some((key, offset)) = &self.dock.menu else {
            return space().into();
        };
        let items = self.dock_items(&self.config.pinned);
        let Some(item) = items.into_iter().find(|item| item.key == *key) else {
            return space().into();
        };
        let shape = self.resting_shape();
        let name = apps::find(&self.apps, key).map_or(key.clone(), |app| app.name.clone());
        crate::dock_menu::view(crate::dock_menu::Menu {
            item,
            name,
            palette: self.config.palette(self.mode),
            edge: self.config.dock.position,
            offset: *offset,
            gap: shape.zone() + 6.0,
        })
    }

    // The app that has the keyboard: what it sent so far is read.
    pub(super) fn watch_front(&mut self) {
        let front = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .find(|w| w.is_focused)
                .map(|w| (w.app_id.as_deref().unwrap_or_default(), w.id)),
            None => self
                .status
                .windows
                .iter()
                .find(|(_, _, focused)| *focused)
                .map(|(id, app, _)| (app.as_str(), *id)),
        }
        .map(|(app, id)| (bar::app_key(&self.apps, app).to_owned(), id));
        let (key, window) = front.unzip();
        self.dock.front(key, window, SystemTime::now());
    }

    pub(super) fn dock_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Pointer(output, along) => self.dock.point(output, along, Instant::now()),
            Event::Frame(now) => self.dock.now = now,
            Event::Open(key) => return self.dock_cycle(&key, true, true),
            Event::Scroll(key, forward) => {
                if self
                    .dock
                    .scrolled
                    .is_some_and(|at| at.elapsed() < SCROLL_EVERY)
                {
                    return Task::none();
                }
                self.dock.scrolled = Some(Instant::now());
                return self.dock_cycle(&key, forward, false);
            }
            Event::Launch(key) => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.launch(&key, true)]);
            }
            Event::Menu(key, offset) => {
                self.dock.menu = Some((key, offset));
                return self.set_overlay(Some(Overlay::DockMenu));
            }
            Event::Pick(window) => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.update(Message::FocusWindow(window))]);
            }
            Event::Pin(key) => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.update(Message::Pin(key))]);
            }
            Event::CloseAll(key) => {
                let close = self.set_overlay(None);
                let windows: Vec<u64> = self
                    .dock_windows()
                    .into_iter()
                    .filter(|(app, _)| *app == key)
                    .map(|(_, window)| window.id)
                    .collect();
                let closing = windows
                    .into_iter()
                    .map(|id| self.update(Message::CloseWindow(id)));
                return Task::batch(std::iter::once(close).chain(closing.collect::<Vec<_>>()));
            }
            Event::DragStart(at) => self.dock_lift(at),
            Event::DragMove(at) => {
                if let Some(drag) = &mut self.dock.dragging {
                    drag.at = at;
                }
            }
            Event::DragDrop(at) => return self.dock_drop(at),
            Event::Hide(..) => {}
        }
        Task::none()
    }

    // The next of an app's windows; with none open, a click starts it.
    fn dock_cycle(&mut self, key: &str, forward: bool, start: bool) -> Task<Message> {
        let windows = self
            .dock_items(&self.config.pinned)
            .into_iter()
            .find(|item| item.key == key)
            .map(|item| item.windows)
            .unwrap_or_default();
        match dock::cycle(&windows, self.dock.last(key), forward) {
            Some(window) => self.update(Message::FocusWindow(window)),
            None if start => self.launch(key, false),
            None => Task::none(),
        }
    }

    fn dock_lift(&mut self, at: iced::Point) {
        let items = self.dock_items(&self.config.pinned);
        let shape = self.dock_shape(&items);
        let (along, _) = self.dock_point(shape, at);
        self.dock.dragging = shape
            .at(along)
            .and_then(|index| items.get(index))
            .map(|item| dock::Drag {
                key: item.key.clone(),
                was_pinned: item.pinned,
                at,
            });
        self.dock.settle(Instant::now());
    }

    fn dock_drop(&mut self, at: iced::Point) -> Task<Message> {
        let rest = self.resting_shape();
        let Some(drag) = self.dock.dragging.as_mut() else {
            return Task::none();
        };
        drag.at = at;
        let pinned = self.dock_pinned(rest);
        self.dock.dragging = None;
        self.dock.settle(Instant::now());
        if pinned == self.config.pinned {
            return Task::none();
        }
        self.save(|e| e.set_list("bar", "pinned", &pinned));
        self.reload_config()
    }
}

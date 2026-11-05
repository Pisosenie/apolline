// The dock's apps: the pinned ones, then those running that aren't, each with its windows and the
// notifications it sent that haven't been looked at.
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use iced::time::Instant;
use iced::{Animation, Point};

use crate::dock_shape::Shape;
use crate::motion::Speed;

const GROWS_IN: Duration = Duration::from_millis(120);
// A pinned app let go this far past the glass, away from the edge, is unpinned.
const OFF: f32 = 48.0;
// A running app joins the pinned ones only when dropped this close past the last of them.
const PIN_BEFORE: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub id: u64,
    pub title: String,
    pub focused: bool,
    pub urgent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    // The desktop entry's id, or the window's app id when there is no entry.
    pub key: String,
    pub pinned: bool,
    // Oldest first, so cycling through them keeps one order.
    pub windows: Vec<Window>,
    pub unread: usize,
}

impl Item {
    pub fn focused(&self) -> bool {
        self.windows.iter().any(|w| w.focused)
    }

    pub fn urgent(&self) -> bool {
        self.windows.iter().any(|w| w.urgent)
    }
}

// Pinned apps in their order, then the others by when their first window opened.
pub fn items(pinned: &[String], windows: Vec<(String, Window)>) -> Vec<Item> {
    let mut items: Vec<Item> = pinned
        .iter()
        .map(|key| Item {
            key: key.clone(),
            pinned: true,
            windows: Vec::new(),
            unread: 0,
        })
        .collect();
    let mut windows = windows;
    windows.sort_by_key(|(_, window)| window.id);
    for (key, window) in windows {
        match items.iter_mut().find(|item| item.key == key) {
            Some(item) => item.windows.push(window),
            None => items.push(Item {
                key,
                pinned: false,
                windows: vec![window],
                unread: 0,
            }),
        }
    }
    items
}

// The window a click or a wheel turn brings forward: the next after the focused one, either way,
// or the one used last when none of the app's is focused.
pub fn cycle(windows: &[Window], last: Option<u64>, forward: bool) -> Option<u64> {
    let Some(at) = windows.iter().position(|w| w.focused) else {
        return last
            .filter(|id| windows.iter().any(|w| w.id == *id))
            .or(windows.first().map(|w| w.id));
    };
    let count = windows.len();
    let next = if forward {
        (at + 1) % count
    } else {
        (at + count - 1) % count
    };
    Some(windows[next].id)
}

// Whether a notification from `desktop` (its desktop entry) and `app` (the name it gave) came
// from the app known by `names`: its key, its entry's name and class, its windows' app ids. As
// the cards do it, a reverse-DNS id matches by its last part too: "org.gnome.Nautilus" is
// "nautilus".
pub fn sent_by(names: &[&str], desktop: Option<&str>, app: &str) -> bool {
    let desktop = desktop.map(|d| d.strip_suffix(".desktop").unwrap_or(d));
    [desktop, Some(app)]
        .into_iter()
        .flatten()
        .filter(|sender| !sender.trim().is_empty())
        .any(|sender| {
            names.iter().filter(|name| !name.is_empty()).any(|name| {
                name.eq_ignore_ascii_case(sender)
                    || last_part(name).eq_ignore_ascii_case(sender)
                    || last_part(sender).eq_ignore_ascii_case(name)
            })
        })
}

fn last_part(id: &str) -> &str {
    match id.rsplit_once('.') {
        Some((_, last)) if !last.is_empty() => last,
        _ => id,
    }
}

// The pinned list as it would be if `dragged` were let go `along` the resting row (its pinned
// part laid out by `shape`) and `away` past the glass from the screen edge.
pub fn landing(
    pinned: &[String],
    dragged: &str,
    was_pinned: bool,
    shape: Shape,
    along: f32,
    away: f32,
) -> Vec<String> {
    let mut rest: Vec<String> = pinned.iter().filter(|p| *p != dragged).cloned().collect();
    if away > OFF {
        return rest;
    }
    let starts = shape.starts();
    let end = rest
        .len()
        .checked_sub(1)
        .and_then(|last| starts.get(last))
        .map_or(0.0, |start| start + shape.size);
    if !was_pinned && along > end + PIN_BEFORE {
        return rest;
    }
    let index = starts
        .iter()
        .take(rest.len())
        .filter(|start| *start + shape.size / 2.0 < along)
        .count();
    rest.insert(index, dragged.to_owned());
    rest
}

// What can happen on the dock.
#[derive(Clone, Debug)]
pub enum Event {
    // The pointer over a screen's dock, as far along its surface as this; none when it left.
    Pointer(Option<String>, Option<f32>),
    Frame(Instant),
    // A click: the app's next window, or the app started.
    Open(String),
    // Another of the app's windows, from a middle click or the menu.
    Launch(String),
    Scroll(String, bool),
    // The app's menu, by its icon this far from the dock's middle, and what's picked in it.
    Menu(String, f32),
    Pick(u64),
    Pin(String),
    CloseAll(String),
    // A drag on a dock, in its surface's points.
    DragStart(Point),
    DragMove(Point),
    DragDrop(Point),
    // A hidden dock's pointer left a while ago; the number tells a later leaving from this one.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Hide(iced::window::Id, u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub key: String,
    pub was_pinned: bool,
    pub at: Point,
}

pub struct Dock {
    // When each app was last in front, so only what came after is unread.
    seen: HashMap<String, SystemTime>,
    front: Option<String>,
    // The window each app had in front last, to go back to.
    last: HashMap<String, u64>,
    pub pointer: Option<(Option<String>, f32)>,
    grow: Animation<bool>,
    // How long growing takes at the shell's motion speed; zero is at once.
    grows_in: Duration,
    pub now: Instant,
    pub dragging: Option<Drag>,
    // Whose menu is open, and how far from the dock's middle its icon is.
    pub menu: Option<(String, f32)>,
    pub scrolled: Option<Instant>,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            seen: HashMap::new(),
            front: None,
            last: HashMap::new(),
            pointer: None,
            grow: growing(false, GROWS_IN),
            grows_in: GROWS_IN,
            now: Instant::now(),
            dragging: None,
            menu: None,
            scrolled: None,
        }
    }
}

impl Dock {
    // The app in front now; the one before is marked seen as it goes, this one as it comes.
    pub fn front(&mut self, key: Option<String>, window: Option<u64>, now: SystemTime) {
        if let (Some(key), Some(window)) = (&key, window) {
            self.last.insert(key.clone(), window);
        }
        if self.front == key {
            return;
        }
        for app in [self.front.take(), key.clone()].into_iter().flatten() {
            self.seen.insert(app, now);
        }
        self.front = key;
    }

    pub fn seen(&self, key: &str) -> Option<SystemTime> {
        self.seen.get(key).copied()
    }

    pub fn is_front(&self, key: &str) -> bool {
        self.front.as_deref() == Some(key)
    }

    pub fn last(&self, key: &str) -> Option<u64> {
        self.last.get(key).copied()
    }

    pub fn point(&mut self, output: Option<String>, along: Option<f32>, now: Instant) {
        self.pointer = along.map(|along| (output, along));
        self.settle(now);
    }

    // Icons grow under a pointer, and not while one of them is being dragged.
    pub fn settle(&mut self, now: Instant) {
        self.now = now;
        let wanted = self.pointer.is_some() && self.dragging.is_none();
        if self.grow.value() != wanted {
            self.grow.go_mut(wanted, now);
        }
    }

    // How far grown the icons are, 0 to 1.
    pub fn strength(&self) -> f32 {
        if self.grows_in.is_zero() {
            return if self.grow.value() { 1.0 } else { 0.0 };
        }
        self.grow.interpolate(0.0, 1.0, self.now)
    }

    pub fn animating(&self) -> bool {
        !self.grows_in.is_zero() && self.grow.is_animating(self.now)
    }

    // Growing follows the shell's motion speed; taken up from where the icons are.
    pub fn pace(&mut self, speed: Speed) {
        let grows_in = speed.time(GROWS_IN);
        if grows_in == self.grows_in {
            return;
        }
        self.grows_in = grows_in;
        self.grow = growing(self.grow.value(), grows_in);
    }
}

// lilt divides by the length, so a zero one is never handed to it.
fn growing(grown: bool, length: Duration) -> Animation<bool> {
    Animation::new(grown)
        .duration(length.max(Duration::from_millis(1)))
        .easing(iced::animation::Easing::EaseOutQuad)
}

// How many of `sent` came after `seen`; the app in front has none waiting.
pub fn unread(sent: impl Iterator<Item = SystemTime>, seen: Option<SystemTime>) -> usize {
    sent.filter(|at| seen.is_none_or(|seen| *at > seen)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, focused: bool) -> Window {
        Window {
            id,
            title: format!("window {id}"),
            focused,
            urgent: false,
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn pinned_apps_come_first_then_the_running_ones() {
        let windows = vec![
            ("foot".to_owned(), window(9, false)),
            ("code".to_owned(), window(3, true)),
            ("firefox".to_owned(), window(5, false)),
            ("code".to_owned(), window(1, false)),
        ];
        let items = items(&names(&["firefox", "files"]), windows);
        let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["firefox", "files", "code", "foot"]);
        assert!(items[0].pinned && !items[2].pinned);
        assert!(items[1].windows.is_empty(), "pinned, not running");
        let ids: Vec<u64> = items[2].windows.iter().map(|w| w.id).collect();
        assert_eq!(ids, [1, 3], "oldest first");
        assert!(items[2].focused() && !items[0].focused());
    }

    #[test]
    fn clicks_and_the_wheel_go_round_the_windows() {
        let windows = [window(1, false), window(2, true), window(3, false)];
        assert_eq!(cycle(&windows, None, true), Some(3));
        assert_eq!(cycle(&windows, None, false), Some(1));
        let last = [window(1, false), window(2, false), window(3, true)];
        assert_eq!(cycle(&last, None, true), Some(1), "round to the first");
        let away = [window(1, false), window(2, false)];
        assert_eq!(cycle(&away, Some(2), true), Some(2), "back to the one used");
        assert_eq!(cycle(&away, Some(7), true), Some(1), "that one is gone");
        assert_eq!(cycle(&[], None, true), None);
    }

    #[test]
    fn notifications_find_their_app() {
        let telegram = [
            "org.telegram.desktop",
            "Telegram Desktop",
            "TelegramDesktop",
        ];
        assert!(sent_by(
            &telegram,
            Some("org.telegram.desktop"),
            "Telegram Desktop"
        ));
        assert!(sent_by(&telegram, None, "Telegram Desktop"), "by its name");
        assert!(sent_by(
            &["firefox", "Firefox"],
            Some("firefox.desktop"),
            "Firefox"
        ));
        assert!(sent_by(&["org.gnome.Nautilus"], None, "nautilus"));
        assert!(sent_by(&["discord"], Some("com.discordapp.Discord"), ""));
        assert!(!sent_by(&["firefox"], Some("thunderbird"), "Thunderbird"));
        assert!(!sent_by(&["code", ""], None, ""), "nothing matches nothing");
    }

    #[test]
    fn only_what_came_after_a_look_is_unread() {
        let at = |s| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
        let sent = [at(10), at(20), at(30)];
        assert_eq!(unread(sent.into_iter(), None), 3, "never looked");
        assert_eq!(unread(sent.into_iter(), Some(at(20))), 1);
        assert_eq!(unread(sent.into_iter(), Some(at(40))), 0);
    }

    #[test]
    fn looking_at_an_app_reads_it() {
        let mut dock = Dock::default();
        let at = |s| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
        dock.front(Some("chat".into()), Some(4), at(10));
        assert_eq!(dock.seen("chat"), Some(at(10)));
        dock.front(Some("chat".into()), Some(5), at(20));
        assert_eq!(dock.seen("chat"), Some(at(10)), "still in front");
        assert_eq!(dock.last("chat"), Some(5));
        dock.front(Some("code".into()), Some(1), at(30));
        assert_eq!(dock.seen("chat"), Some(at(30)), "seen until it went");
        assert!(dock.is_front("code") && !dock.is_front("chat"));
    }

    const ROW: Shape = Shape {
        size: 48.0,
        count: 4,
        split: Some(3),
        sideways: false,
    };

    #[test]
    fn a_pinned_app_moves_between_the_others() {
        let pinned = names(&["a", "b", "c"]);
        assert_eq!(
            landing(&pinned, "a", true, ROW, 140.0, 0.0),
            names(&["b", "c", "a"])
        );
        assert_eq!(
            landing(&pinned, "c", true, ROW, 0.0, 0.0),
            names(&["c", "a", "b"])
        );
        assert_eq!(
            landing(&pinned, "b", true, ROW, 60.0, 0.0),
            pinned,
            "barely moved"
        );
        assert_eq!(
            landing(&pinned, "c", true, ROW, 400.0, 10.0),
            pinned,
            "past the end it stays last"
        );
    }

    #[test]
    fn dragged_off_the_dock_it_is_unpinned() {
        let pinned = names(&["a", "b", "c"]);
        assert_eq!(
            landing(&pinned, "b", true, ROW, 60.0, OFF + 1.0),
            names(&["a", "c"])
        );
        assert_eq!(
            landing(&pinned, "w", false, ROW, 60.0, OFF + 1.0),
            pinned,
            "a running app dragged off stays as it was"
        );
    }

    #[test]
    fn a_running_app_dropped_among_the_pinned_is_pinned_there() {
        let pinned = names(&["a", "b"]);
        assert_eq!(
            landing(&pinned, "w", false, ROW, 30.0, 0.0),
            names(&["a", "w", "b"])
        );
        assert_eq!(
            landing(&pinned, "w", false, ROW, 112.0, 0.0),
            names(&["a", "b", "w"]),
            "just past the last"
        );
        assert_eq!(
            landing(&pinned, "w", false, ROW, 200.0, 0.0),
            pinned,
            "still among the running"
        );
    }

    #[test]
    fn without_animations_the_icons_grow_at_once() {
        let mut dock = Dock::default();
        dock.pace(Speed::new(Some(0.0), false));
        let now = Instant::now();
        dock.point(None, Some(10.0), now);
        assert!(!dock.animating());
        assert_eq!(dock.strength(), 1.0);
        dock.pace(Speed::default());
        dock.point(None, None, now);
        assert!(dock.animating(), "back to growing in time");
    }
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::json;

pub fn socket() -> Option<PathBuf> {
    std::env::var_os("NIRI_SOCKET").map(PathBuf::from)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    EventStream,
    FocusWorkspace(u64),
    FocusWindow(u64),
    CloseWindow(u64),
    Spawn(String),
}

impl Request {
    pub fn line(self) -> String {
        let value = match self {
            Request::EventStream => json!("EventStream"),
            Request::FocusWorkspace(id) => {
                json!({ "Action": { "FocusWorkspace": { "reference": { "Id": id } } } })
            }
            Request::FocusWindow(id) => json!({ "Action": { "FocusWindow": { "id": id } } }),
            Request::CloseWindow(id) => json!({ "Action": { "CloseWindow": { "id": id } } }),
            Request::Spawn(command) => json!({ "Action": { "SpawnSh": { "command": command } } }),
        };
        format!("{value}\n")
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Output {
    pub name: String,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    pub modes: Vec<OutputMode>,
    pub current_mode: Option<usize>,
    #[serde(default)]
    pub vrr_supported: bool,
    #[serde(default)]
    pub vrr_enabled: bool,
    pub logical: Option<Logical>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct OutputMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    #[serde(default)]
    pub is_preferred: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Logical {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub transform: String,
}

pub fn outputs(json: &str) -> Vec<Output> {
    serde_json::from_str::<BTreeMap<String, Output>>(json)
        .map(|outputs| outputs.into_values().collect())
        .unwrap_or_default()
}

pub fn output_names(outputs: &str) -> Vec<String> {
    serde_json::from_str::<BTreeMap<String, serde_json::Value>>(outputs)
        .map(|outputs| outputs.into_keys().collect())
        .unwrap_or_default()
}

pub fn largest_mode(outputs: &str) -> Option<(u32, u32)> {
    let outputs: BTreeMap<String, serde_json::Value> = serde_json::from_str(outputs).ok()?;
    outputs
        .values()
        .filter_map(|output| {
            let current = output.get("current_mode")?.as_u64()?;
            let mode = output.get("modes")?.get(current as usize)?;
            Some((
                u32::try_from(mode.get("width")?.as_u64()?).ok()?,
                u32::try_from(mode.get("height")?.as_u64()?).ok()?,
            ))
        })
        .max_by_key(|(w, h)| u64::from(*w) * u64::from(*h))
}

pub fn is_ok(reply: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(reply).is_ok_and(|v| v.get("Ok").is_some())
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Workspace {
    pub id: u64,
    pub idx: u8,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_active: bool,
    pub is_focused: bool,
    #[serde(default)]
    pub is_urgent: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    #[serde(default)]
    pub is_floating: bool,
    // Asking to be looked at; niri before 25.05 doesn't say.
    #[serde(default)]
    pub is_urgent: bool,
    #[serde(default)]
    pub layout: Option<WindowLayout>,
    #[serde(default)]
    pub focus_timestamp: Option<Timestamp>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WindowLayout {
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct Timestamp {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct KeyboardLayouts {
    pub names: Vec<String>,
    pub current_idx: u8,
}

// A screencast: a portal sharing the screen, a recorder, a VNC server.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Cast {
    pub stream_id: u64,
    // False while it's paused, as OBS does when switching away from the scene.
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Event {
    WorkspacesChanged {
        workspaces: Vec<Workspace>,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WindowsChanged {
        windows: Vec<Window>,
    },
    WindowOpenedOrChanged {
        window: Window,
    },
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowFocusTimestampChanged {
        id: u64,
        focus_timestamp: Option<Timestamp>,
    },
    WindowUrgencyChanged {
        id: u64,
        urgent: bool,
    },
    KeyboardLayoutsChanged {
        keyboard_layouts: KeyboardLayouts,
    },
    KeyboardLayoutSwitched {
        idx: u8,
    },
    ScreenshotCaptured {
        path: Option<String>,
    },
    CastsChanged {
        casts: Vec<Cast>,
    },
    CastStartedOrChanged {
        cast: Cast,
    },
    CastStopped {
        stream_id: u64,
    },
}

impl Event {
    pub fn parse(line: &str) -> Option<Self> {
        serde_json::from_str(line).ok()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    workspaces: Vec<Workspace>,
    windows: BTreeMap<u64, Window>,
    last: Option<u64>,
    layouts: Option<KeyboardLayouts>,
    casts: BTreeMap<u64, Cast>,
}

impl State {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::WorkspacesChanged { workspaces } => self.workspaces = workspaces,
            Event::WorkspaceActivated { id, focused } => {
                let output = self
                    .workspaces
                    .iter()
                    .find(|w| w.id == id)
                    .map(|w| w.output.clone());
                for workspace in &mut self.workspaces {
                    if output.as_ref() == Some(&workspace.output) {
                        workspace.is_active = workspace.id == id;
                    }
                    if focused {
                        workspace.is_focused = workspace.id == id;
                    }
                }
            }
            Event::WindowsChanged { windows } => {
                self.last = windows
                    .iter()
                    .find(|w| w.is_focused)
                    .map(|w| w.id)
                    .or(self.last);
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
            }
            Event::WindowOpenedOrChanged { window } => {
                if window.is_focused {
                    self.windows.values_mut().for_each(|w| w.is_focused = false);
                    self.last = Some(window.id);
                }
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
                if self.last == Some(id) {
                    self.last = None;
                }
            }
            Event::WindowFocusChanged { id } => {
                self.windows
                    .values_mut()
                    .for_each(|w| w.is_focused = Some(w.id) == id);
                if id.is_some() {
                    self.last = id;
                }
            }
            Event::WindowFocusTimestampChanged {
                id,
                focus_timestamp,
            } => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.focus_timestamp = focus_timestamp;
                }
            }
            Event::WindowUrgencyChanged { id, urgent } => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.is_urgent = urgent;
                }
            }
            Event::KeyboardLayoutsChanged { keyboard_layouts } => {
                self.layouts = Some(keyboard_layouts)
            }
            Event::KeyboardLayoutSwitched { idx } => {
                if let Some(layouts) = &mut self.layouts {
                    layouts.current_idx = idx;
                }
            }
            Event::ScreenshotCaptured { .. } => {}
            Event::CastsChanged { casts } => {
                self.casts = casts.into_iter().map(|c| (c.stream_id, c)).collect();
            }
            Event::CastStartedOrChanged { cast } => {
                self.casts.insert(cast.stream_id, cast);
            }
            Event::CastStopped { stream_id } => {
                self.casts.remove(&stream_id);
            }
        }
    }

    // Whether anything is taking the screen right now.
    pub fn casting(&self) -> bool {
        self.casts.values().any(|c| c.is_active)
    }

    pub fn workspaces(&self) -> Vec<&Workspace> {
        let focused = self
            .workspaces
            .iter()
            .find(|w| w.is_focused)
            .and_then(|w| w.output.as_deref());
        self.workspaces_on(focused)
    }

    pub fn workspaces_on(&self, output: Option<&str>) -> Vec<&Workspace> {
        let mut list: Vec<_> = self
            .workspaces
            .iter()
            .filter(|w| output.is_none() || w.output.as_deref() == output)
            .collect();
        list.sort_by_key(|w| w.idx);
        list
    }

    pub fn outputs(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .workspaces
            .iter()
            .filter_map(|w| w.output.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn is_busy(&self, workspace: u64) -> bool {
        self.windows
            .values()
            .any(|w| w.workspace_id == Some(workspace))
    }

    pub fn is_running(&self, command: &str) -> bool {
        let Some(program) = command.split_whitespace().next() else {
            return false;
        };
        let name = program.rsplit('/').next().unwrap_or(program);
        self.windows
            .values()
            .filter_map(|w| w.app_id.as_deref())
            .any(|app| app.eq_ignore_ascii_case(name))
    }

    pub fn recent(&self) -> Vec<&Window> {
        let mut windows: Vec<&Window> = self.windows.values().collect();
        windows.sort_by_key(|w| {
            (
                std::cmp::Reverse(Some(w.id) == self.last),
                std::cmp::Reverse(w.focus_timestamp),
            )
        });
        windows
    }

    pub fn windows_on(&self, workspace: u64) -> Vec<&Window> {
        let mut windows: Vec<&Window> = self
            .windows
            .values()
            .filter(|w| w.workspace_id == Some(workspace))
            .collect();
        windows.sort_by_key(|w| {
            let place = w.layout.as_ref().and_then(|l| l.pos_in_scrolling_layout);
            (
                w.is_floating,
                place.unwrap_or((usize::MAX, usize::MAX)),
                w.id,
            )
        });
        windows
    }

    pub fn focused_output(&self) -> Option<&str> {
        self.workspaces
            .iter()
            .find(|w| w.is_focused)
            .and_then(|w| w.output.as_deref())
    }

    #[cfg(test)]
    fn focused_window(&self) -> Option<&Window> {
        self.windows.values().find(|w| w.is_focused)
    }

    pub fn layout(&self) -> Option<&str> {
        let layouts = self.layouts.as_ref()?;
        layouts
            .names
            .get(layouts.current_idx as usize)
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_read_modes_and_where_they_sit() {
        let reply = r#"{"winit":{"name":"winit","make":"Smithay","model":"Winit","serial":null,"physical_size":null,"modes":[{"width":1440,"height":900,"refresh_rate":60000,"is_preferred":true}],"current_mode":0,"is_custom_mode":true,"vrr_supported":false,"vrr_enabled":false,"logical":{"x":0,"y":0,"width":1440,"height":900,"scale":1.0,"transform":"Flipped180"}}}"#;
        let found = outputs(reply);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].modes[0].refresh_rate, 60000);
        assert_eq!(found[0].logical.as_ref().unwrap().transform, "Flipped180");
    }

    #[test]
    fn the_largest_screen_wins() {
        let reply = r#"{"eDP-1":{"modes":[{"width":2880,"height":1800,"refresh_rate":120000,"is_preferred":true}],"current_mode":0},"HDMI-A-1":{"modes":[{"width":1920,"height":1080,"refresh_rate":60000},{"width":3840,"height":2160,"refresh_rate":60000}],"current_mode":1},"DP-2":{"modes":[],"current_mode":null}}"#;
        assert_eq!(largest_mode(reply), Some((3840, 2160)));
        assert_eq!(largest_mode("{}"), None);
    }

    #[test]
    fn recent_windows_follow_focus_and_bar_order_follows_columns() {
        let mut state = State::default();
        let window = |id: u64, column: usize, secs: u64, focused: bool| {
            format!(
                r#"{{"id":{id},"title":"w{id}","app_id":"foot","workspace_id":1,"is_focused":{focused},"layout":{{"pos_in_scrolling_layout":[{column},1]}},"focus_timestamp":{{"secs":{secs},"nanos":0}}}}"#
            )
        };
        let all = format!(
            r#"{{"WindowsChanged":{{"windows":[{},{},{}]}}}}"#,
            window(1, 3, 10, false),
            window(2, 1, 30, true),
            window(3, 2, 20, false)
        );
        state.apply(Event::parse(&all).unwrap());
        let ids = |list: Vec<&Window>| list.iter().map(|w| w.id).collect::<Vec<_>>();
        assert_eq!(ids(state.recent()), [2, 3, 1]);
        assert_eq!(ids(state.windows_on(1)), [2, 3, 1]);
        state.apply(Event::parse(r#"{"WindowFocusChanged":{"id":1}}"#).unwrap());
        assert_eq!(
            ids(state.recent()),
            [1, 2, 3],
            "focused first, then niri's own order"
        );
        state.apply(
            Event::parse(r#"{"WindowFocusTimestampChanged":{"id":1,"focus_timestamp":{"secs":40,"nanos":0}}}"#)
                .unwrap(),
        );
        state.apply(Event::parse(r#"{"WindowFocusChanged":{"id":3}}"#).unwrap());
        assert_eq!(ids(state.recent()), [3, 1, 2]);
        state.apply(Event::parse(r#"{"WindowFocusChanged":{"id":null}}"#).unwrap());
        assert_eq!(
            ids(state.recent()),
            [3, 1, 2],
            "an overlay taking the keyboard changes nothing"
        );
        state.apply(Event::parse(r#"{"WindowClosed":{"id":2}}"#).unwrap());
        assert_eq!(ids(state.recent()), [3, 1]);
        assert_eq!(
            Request::FocusWindow(3).line(),
            "{\"Action\":{\"FocusWindow\":{\"id\":3}}}\n"
        );
    }

    fn replay(lines: &str) -> State {
        let mut state = State::default();
        lines
            .lines()
            .filter_map(Event::parse)
            .for_each(|e| state.apply(e));
        state
    }

    #[test]
    fn requests_are_single_json_lines() {
        assert_eq!(Request::EventStream.line(), "\"EventStream\"\n");
        assert_eq!(
            Request::FocusWorkspace(2).line(),
            "{\"Action\":{\"FocusWorkspace\":{\"reference\":{\"Id\":2}}}}\n"
        );
        assert_eq!(
            Request::Spawn("firefox --new-window".into()).line(),
            "{\"Action\":{\"SpawnSh\":{\"command\":\"firefox --new-window\"}}}\n"
        );
    }

    #[test]
    fn a_screencast_shows_while_it_streams() {
        let mut state = State::default();
        let cast = |id, active| {
            format!(
                r#"{{"CastStartedOrChanged":{{"cast":{{"stream_id":{id},"session_id":1,"kind":"PipeWire","target":{{"Output":{{"name":"eDP-1"}}}},"is_dynamic_target":false,"is_active":{active},"pid":null,"pw_node_id":40}}}}}}"#
            )
        };
        let apply = |state: &mut State, line: &str| state.apply(Event::parse(line).unwrap());
        apply(&mut state, &cast(5, false));
        assert!(!state.casting(), "paused");
        apply(&mut state, &cast(5, true));
        assert!(state.casting());
        apply(&mut state, r#"{"CastStopped":{"stream_id":5}}"#);
        assert!(!state.casting());
    }

    #[test]
    fn a_window_asks_for_attention_and_is_answered() {
        let mut state = State::default();
        let apply = |state: &mut State, line: &str| state.apply(Event::parse(line).unwrap());
        apply(
            &mut state,
            r#"{"WindowOpenedOrChanged":{"window":{"id":4,"title":"chat","app_id":"telegram","workspace_id":1,"is_focused":false,"is_urgent":false}}}"#,
        );
        assert!(!state.recent()[0].is_urgent);
        apply(
            &mut state,
            r#"{"WindowUrgencyChanged":{"id":4,"urgent":true}}"#,
        );
        assert!(state.recent()[0].is_urgent);
        apply(
            &mut state,
            r#"{"WindowUrgencyChanged":{"id":4,"urgent":false}}"#,
        );
        assert!(!state.recent()[0].is_urgent);
    }

    #[test]
    fn replies_are_ok_or_err() {
        assert!(is_ok(r#"{"Ok":"Handled"}"#));
        assert!(!is_ok(r#"{"Err":"error parsing request"}"#));
        assert!(!is_ok("garbage"));
    }

    #[test]
    fn unknown_events_are_skipped() {
        assert_eq!(Event::parse(r#"{"SomethingNew":{"x":[]}}"#), None);
        assert_eq!(
            Event::parse(r#"{"WindowClosed":{"id":7}}"#),
            Some(Event::WindowClosed { id: 7 })
        );
    }

    #[test]
    fn recorded_session_replays_to_the_final_state() {
        let state = replay(include_str!("../tests/events.jsonl"));

        let workspaces = state.workspaces();
        let ids: Vec<_> = workspaces.iter().map(|w| (w.idx, w.is_active)).collect();
        assert_eq!(ids, [(1, true), (2, false), (3, false)]);
        assert!(state.is_busy(workspaces[0].id));
        assert!(state.is_busy(workspaces[1].id));
        assert!(!state.is_busy(workspaces[2].id));
        assert_eq!(state.focused_window().map(|w| w.id), Some(2));
        assert_eq!(state.layout(), Some("English (US)"));
        assert_eq!(state.outputs(), ["winit"]);
        assert!(state.is_running("foot"));
        assert!(state.is_running("/usr/bin/foot --server"));
        assert!(!state.is_running("steam"));
        assert!(!state.is_running("  "));
    }

    // Every kind of event Apolline knows must still read, so a change in niri's JSON
    // shows up in the tests instead of as a quietly stale bar. Returns the state at the
    // end, and whether the focused window read as floating when the screenshot was taken.
    fn replay_session(session: &str) -> (State, bool) {
        const KNOWN: [&str; 10] = [
            "WorkspacesChanged",
            "WorkspaceActivated",
            "WindowsChanged",
            "WindowOpenedOrChanged",
            "WindowClosed",
            "WindowFocusChanged",
            "WindowFocusTimestampChanged",
            "KeyboardLayoutsChanged",
            "KeyboardLayoutSwitched",
            "ScreenshotCaptured",
        ];
        let mut state = State::default();
        let mut floated = false;
        for line in session.lines() {
            let kind = line
                .trim_start_matches("{\"")
                .split('"')
                .next()
                .unwrap_or_default();
            let event = Event::parse(line);
            if KNOWN.contains(&kind) {
                assert!(event.is_some(), "{kind} no longer reads: {line}");
            }
            if let Some(Event::ScreenshotCaptured { path }) = &event {
                assert!(path.as_deref().is_some_and(|p| p.ends_with(".png")));
                floated = state.focused_window().is_some_and(|w| w.is_floating);
            }
            if let Some(event) = event {
                state.apply(event);
            }
        }
        (state, floated)
    }

    #[test]
    fn a_niri_26_04_session_reads_in_full() {
        let (state, floated) = replay_session(include_str!("../tests/sessions/niri-26.04.jsonl"));
        assert!(floated, "the window made floating reads as floating");
        assert_eq!(state.focused_window().map(|w| w.id), Some(6));
        assert_eq!(state.focused_output(), Some("winit"));
        assert!(!state.is_running("rec-one") && !state.is_running("rec-two"));
        assert_eq!(state.workspaces().len(), 2);
    }

    // CI records a fresh session from the newest niri with tools/niri-session.sh.
    #[test]
    fn a_live_session_reads_in_full() {
        let Some(file) = std::env::var_os("APOLLINE_NIRI_SESSION") else {
            return;
        };
        let session = std::fs::read_to_string(file).expect("the recorded session");
        assert!(session.lines().count() > 10, "too little was recorded");
        let (state, floated) = replay_session(&session);
        assert!(floated, "the window made floating reads as floating");
        assert!(!state.is_running("session-one") && !state.is_running("session-two"));
        assert!(state.focused_output().is_some());
    }

    #[test]
    fn each_output_has_its_own_workspaces() {
        let state = replay(concat!(
            r#"{"WorkspacesChanged":{"workspaces":["#,
            r#"{"id":1,"idx":1,"name":null,"output":"DP-1","is_active":true,"is_focused":false},"#,
            r#"{"id":2,"idx":2,"name":null,"output":"DP-1","is_active":false,"is_focused":false},"#,
            r#"{"id":3,"idx":1,"name":null,"output":"eDP-1","is_active":true,"is_focused":true}]}}"#,
        ));
        let ids = |list: Vec<&Workspace>| list.iter().map(|w| w.id).collect::<Vec<_>>();
        assert_eq!(ids(state.workspaces_on(Some("DP-1"))), [1, 2]);
        assert_eq!(ids(state.workspaces_on(Some("eDP-1"))), [3]);
        assert_eq!(ids(state.workspaces()), [3]);
        assert_eq!(state.outputs(), ["DP-1", "eDP-1"]);
    }

    #[test]
    fn layout_switch_and_window_close() {
        let state = replay(concat!(
            r#"{"KeyboardLayoutsChanged":{"keyboard_layouts":{"names":["English (US)","Russian"],"current_idx":0}}}"#,
            "\n",
            r#"{"KeyboardLayoutSwitched":{"idx":1}}"#,
            "\n",
            r#"{"WindowsChanged":{"windows":[{"id":4,"title":null,"app_id":"foot","workspace_id":1,"is_focused":true}]}}"#,
            "\n",
            r#"{"WindowClosed":{"id":4}}"#,
        ));
        assert_eq!(state.layout(), Some("Russian"));
        assert!(!state.is_busy(1));
        assert_eq!(state.focused_window(), None);
    }
}

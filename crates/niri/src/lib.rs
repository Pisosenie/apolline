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
    Spawn(String),
}

impl Request {
    pub fn line(self) -> String {
        let value = match self {
            Request::EventStream => json!("EventStream"),
            Request::FocusWorkspace(id) => {
                json!({ "Action": { "FocusWorkspace": { "reference": { "Id": id } } } })
            }
            Request::Spawn(command) => json!({ "Action": { "SpawnSh": { "command": command } } }),
        };
        format!("{value}\n")
    }
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
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct KeyboardLayouts {
    pub names: Vec<String>,
    pub current_idx: u8,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Event {
    WorkspacesChanged { workspaces: Vec<Workspace> },
    WorkspaceActivated { id: u64, focused: bool },
    WindowsChanged { windows: Vec<Window> },
    WindowOpenedOrChanged { window: Window },
    WindowClosed { id: u64 },
    WindowFocusChanged { id: Option<u64> },
    KeyboardLayoutsChanged { keyboard_layouts: KeyboardLayouts },
    KeyboardLayoutSwitched { idx: u8 },
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
    layouts: Option<KeyboardLayouts>,
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
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
            }
            Event::WindowOpenedOrChanged { window } => {
                if window.is_focused {
                    self.windows.values_mut().for_each(|w| w.is_focused = false);
                }
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
            }
            Event::WindowFocusChanged { id } => {
                self.windows
                    .values_mut()
                    .for_each(|w| w.is_focused = Some(w.id) == id);
            }
            Event::KeyboardLayoutsChanged { keyboard_layouts } => {
                self.layouts = Some(keyboard_layouts)
            }
            Event::KeyboardLayoutSwitched { idx } => {
                if let Some(layouts) = &mut self.layouts {
                    layouts.current_idx = idx;
                }
            }
        }
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

    pub fn focused_window(&self) -> Option<&Window> {
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
    fn replies_are_ok_or_err() {
        assert!(is_ok(r#"{"Ok":"Handled"}"#));
        assert!(!is_ok(r#"{"Err":"error parsing request"}"#));
        assert!(!is_ok("garbage"));
    }

    #[test]
    fn unknown_events_are_skipped() {
        assert_eq!(Event::parse(r#"{"CastsChanged":{"casts":[]}}"#), None);
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

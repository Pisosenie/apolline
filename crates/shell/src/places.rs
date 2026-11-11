// A mode's windows: remembered as they stand, and put back in their places when the mode comes
// on. Windows already open move; the rest start and are placed as niri opens them.

use std::time::{Duration, Instant};

use apolline_config::Placed;
use apolline_niri::{Output, Request, Spot, State, Window, Workspace};

// Apps that bring back their own windows take a while; after this the mode stops waiting.
const WAIT: Duration = Duration::from_secs(60);

pub struct Arranging {
    wanted: Vec<Placed>,
    waiting: Vec<usize>,
    claimed: Vec<u64>,
    // The columns placed so far, by workspace id and the column asked for.
    columns: Vec<(u64, u16)>,
    until: Instant,
}

impl Arranging {
    // The windows that are open go now; what comes back is what to ask niri, in order, and the
    // commands to start for those that aren't.
    pub fn start(wanted: Vec<Placed>, state: &State) -> (Self, Vec<Request>, Vec<String>) {
        let mut arranging = Self {
            waiting: Vec::new(),
            claimed: Vec::new(),
            columns: Vec::new(),
            until: Instant::now() + WAIT,
            wanted,
        };
        let mut asked = Vec::new();
        let mut run = Vec::new();
        for index in 0..arranging.wanted.len() {
            let found = state
                .all_windows()
                .into_iter()
                .filter(|w| !arranging.claimed.contains(&w.id))
                .find(|w| fits(&arranging.wanted[index], w))
                .map(|w| w.id);
            match found {
                Some(id) => asked.extend(arranging.put(index, id, state)),
                None => {
                    arranging.waiting.push(index);
                    run.extend(arranging.wanted[index].run.clone());
                }
            }
        }
        run.dedup();
        (arranging, asked, run)
    }

    pub fn done(&self) -> bool {
        self.waiting.is_empty() || Instant::now() >= self.until
    }

    // A window niri opened or changed: placed if one of the mode's is waiting for it. A title
    // often comes a moment after the window, so a change is looked at as well.
    pub fn opened(&mut self, window: &Window, state: &State) -> Vec<Request> {
        if self.done() || self.claimed.contains(&window.id) {
            return Vec::new();
        }
        let Some(at) = self
            .waiting
            .iter()
            .position(|&index| fits(&self.wanted[index], window))
        else {
            return Vec::new();
        };
        let index = self.waiting.remove(at);
        self.put(index, window.id, state)
    }

    fn put(&mut self, index: usize, window: u64, state: &State) -> Vec<Request> {
        self.claimed.push(window);
        let wanted = &self.wanted[index];
        let screen = wanted
            .screen
            .clone()
            .filter(|name| state.outputs().contains(name))
            .or_else(|| state.focused_output().map(str::to_owned));
        let workspaces = state.workspaces_on(screen.as_deref());
        let Some(workspace) = workspaces
            .iter()
            .find(|w| w.idx == wanted.workspace)
            .or(workspaces.last())
            .map(|w| w.id)
        else {
            return Vec::new();
        };
        let below = self.columns.contains(&(workspace, wanted.column));
        // Columns of the mode keep their order among themselves; anything else stays right.
        let before = self
            .columns
            .iter()
            .filter(|(w, c)| *w == workspace && *c < wanted.column)
            .count();
        if !below {
            self.columns.push((workspace, wanted.column));
        }
        let spot = Spot {
            workspace,
            column: before + 1,
            width: wanted.width,
            below,
        };
        let focused = state
            .recent()
            .first()
            .filter(|w| w.is_focused)
            .map(|w| w.id);
        apolline_niri::place(window, &spot, focused)
    }
}

fn fits(wanted: &Placed, window: &Window) -> bool {
    !window.is_floating
        && window
            .app_id
            .as_deref()
            .is_some_and(|app| app.eq_ignore_ascii_case(&wanted.app))
        && wanted.title.as_deref().is_none_or(|part| {
            window
                .title
                .as_deref()
                .is_some_and(|title| title.to_lowercase().contains(&part.to_lowercase()))
        })
}

// The windows as they stand, each with its screen, workspace, column and width; `run` is how
// to start each one again. Floating windows are left out.
pub fn remember(
    windows: &[Window],
    workspaces: &[Workspace],
    outputs: &[Output],
    gaps: u16,
    run: impl Fn(&Window) -> Option<String>,
) -> Vec<Placed> {
    let mut placed: Vec<(u8, (usize, usize), Placed)> = windows
        .iter()
        .filter(|w| !w.is_floating)
        .filter_map(|window| {
            let app = window.app_id.clone().filter(|a| !a.is_empty())?;
            let workspace = workspaces
                .iter()
                .find(|w| Some(w.id) == window.workspace_id)?;
            let layout = window.layout.as_ref()?;
            let (column, tile) = layout.pos_in_scrolling_layout?;
            let screen = workspace.output.clone();
            let width = layout.tile_size.and_then(|(tile, _)| {
                let output = outputs.iter().find(|o| Some(&o.name) == screen.as_ref())?;
                percent(
                    tile,
                    f64::from(output.logical.as_ref()?.width),
                    f64::from(gaps),
                )
            });
            let order = (workspace.idx, (column, tile));
            Some((
                order.0,
                order.1,
                Placed {
                    run: run(window),
                    app,
                    title: None,
                    screen,
                    workspace: workspace.idx,
                    column: u16::try_from(column).ok()?,
                    width,
                },
            ))
        })
        .collect();
    placed.sort_by(|a, b| (&a.2.screen, a.0, a.1).cmp(&(&b.2.screen, b.0, b.1)));
    placed.into_iter().map(|(_, _, p)| p).collect()
}

// niri makes a column (screen - gaps) * p - gaps wide; this is p back, in whole percents.
fn percent(tile: f64, screen: f64, gaps: f64) -> Option<u8> {
    let room = screen - gaps;
    if room <= 0.0 {
        return None;
    }
    let p = ((tile + gaps) / room * 100.0).round();
    let range = Placed::WIDTHS;
    (f64::from(*range.start())..=f64::from(*range.end()))
        .contains(&p)
        .then_some(p as u8)
}

// How to start a window again: its app's own launcher entry, else the command it runs as.
pub fn run_of(window: &Window, apps: &[crate::apps::App]) -> Option<String> {
    let app = window.app_id.as_deref()?;
    if let Some(found) = crate::apps::find(apps, app) {
        return Some(found.exec.clone());
    }
    let line = std::fs::read(format!("/proc/{}/cmdline", window.pid?)).ok()?;
    let words: Vec<String> = line
        .split(|&b| b == 0)
        .filter(|w| !w.is_empty())
        .map(|w| quote(&String::from_utf8_lossy(w)))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn quote(word: &str) -> String {
    if word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolline_niri::Event;

    fn placed(app: &str, workspace: u8, column: u16) -> Placed {
        Placed {
            app: app.into(),
            title: None,
            run: Some(app.into()),
            screen: Some("DP-1".into()),
            workspace,
            column,
            width: Some(50),
        }
    }

    fn state(lines: &[&str]) -> State {
        let mut state = State::default();
        for line in lines {
            state.apply(Event::parse(line).unwrap());
        }
        state
    }

    const WORKSPACES: &str = r#"{"WorkspacesChanged":{"workspaces":[{"id":10,"idx":1,"name":null,"output":"DP-1","is_active":true,"is_focused":true},{"id":11,"idx":2,"name":null,"output":"DP-1","is_active":false,"is_focused":false},{"id":12,"idx":3,"name":null,"output":"DP-1","is_active":false,"is_focused":false}]}}"#;

    fn window(id: u64, app: &str, title: &str) -> String {
        format!(
            r#"{{"WindowOpenedOrChanged":{{"window":{{"id":{id},"title":"{title}","app_id":"{app}","workspace_id":10,"is_focused":false}}}}}}"#
        )
    }

    #[test]
    fn open_windows_move_and_the_rest_start_and_are_placed_in_column_order() {
        let now = state(&[WORKSPACES, &window(1, "firefox", "News")]);
        let wanted = vec![
            placed("kitty", 1, 1),
            placed("firefox", 1, 2),
            Placed {
                column: 1,
                ..placed("foot", 1, 1)
            },
            placed("code", 2, 1),
        ];
        let (mut arranging, asked, run) = Arranging::start(wanted, &now);
        assert_eq!(run, ["kitty", "foot", "code"]);
        assert_eq!(
            asked[..3],
            [
                Request::MoveWindowToWorkspace {
                    window: 1,
                    workspace: 10
                },
                Request::FocusWindow(1),
                Request::MoveColumnToIndex(1),
            ]
        );

        // kitty opens after: column 1 goes left of firefox's column 2.
        let now = state(&[
            WORKSPACES,
            &window(1, "firefox", "News"),
            &window(2, "kitty", "~"),
        ]);
        let kitty = now.window(2).unwrap().clone();
        assert!(
            arranging
                .opened(&kitty, &now)
                .contains(&Request::MoveColumnToIndex(1))
        );
        assert!(arranging.opened(&kitty, &now).is_empty(), "placed once");
        // foot shares kitty's column: in at 2, then into the column on its left.
        let foot = Window {
            id: 3,
            app_id: Some("foot".into()),
            ..kitty.clone()
        };
        let asked = arranging.opened(&foot, &now);
        assert!(asked.contains(&Request::MoveColumnToIndex(2)));
        assert!(asked.contains(&Request::ConsumeOrExpelWindowLeft(3)));
        let code = Window {
            id: 4,
            app_id: Some("Code".into()),
            ..kitty
        };
        assert_eq!(
            arranging.opened(&code, &now)[0],
            Request::MoveWindowToWorkspace {
                window: 4,
                workspace: 11
            }
        );
        assert!(arranging.done());
    }

    #[test]
    fn a_title_picks_which_window_and_a_missing_workspace_is_the_last() {
        let now = state(&[
            WORKSPACES,
            &window(1, "firefox", "News"),
            &window(2, "firefox", "Inbox — Mail"),
        ]);
        let wanted = vec![Placed {
            title: Some("mail".into()),
            workspace: 9,
            ..placed("firefox", 9, 1)
        }];
        let (_, asked, run) = Arranging::start(wanted, &now);
        assert!(run.is_empty());
        assert_eq!(
            asked[0],
            Request::MoveWindowToWorkspace {
                window: 2,
                workspace: 12
            }
        );
    }

    #[test]
    fn remembering_reads_columns_widths_and_screens() {
        let windows = apolline_niri::windows(
            r#"[{"id":1,"title":"a","app_id":"kitty","pid":10,"workspace_id":10,"is_focused":true,"is_floating":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[945.0,1000.0]}},
                {"id":2,"title":"b","app_id":"foot","pid":11,"workspace_id":10,"is_focused":false,"is_floating":false,"layout":{"pos_in_scrolling_layout":[1,2],"tile_size":[945.0,500.0]}},
                {"id":3,"title":"c","app_id":"firefox","pid":12,"workspace_id":11,"is_focused":false,"is_floating":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[1270.0,1000.0]}},
                {"id":4,"title":"d","app_id":"pavucontrol","workspace_id":10,"is_focused":false,"is_floating":true,"layout":{"pos_in_scrolling_layout":null,"tile_size":[400.0,300.0]}}]"#,
        );
        let workspaces = apolline_niri::workspaces(
            r#"[{"id":10,"idx":1,"name":null,"output":"DP-1","is_active":true,"is_focused":true},{"id":11,"idx":2,"name":null,"output":"DP-1","is_active":false,"is_focused":false}]"#,
        );
        let outputs = apolline_niri::outputs(
            r#"{"DP-1":{"name":"DP-1","modes":[],"current_mode":null,"logical":{"x":0,"y":0,"width":1920,"height":1080,"scale":1.0,"transform":"Normal"}}}"#,
        );
        let remembered = remember(&windows, &workspaces, &outputs, 10, |w| w.app_id.clone());
        let short: Vec<(&str, u8, u16, Option<u8>)> = remembered
            .iter()
            .map(|p| (p.app.as_str(), p.workspace, p.column, p.width))
            .collect();
        assert_eq!(
            short,
            [
                ("kitty", 1, 1, Some(50)),
                ("foot", 1, 1, Some(50)),
                ("firefox", 2, 1, Some(67))
            ]
        );
        assert_eq!(quote("--title=a b"), "'--title=a b'");
        assert_eq!(quote("--class=x"), "--class=x");
    }
}

// Run by tools/niri-place.sh against a real niri with three foot windows open.
#[cfg(test)]
mod live {
    use super::*;
    use apolline_niri::Event;
    use std::io::{BufRead, BufReader, Write};

    fn niri(what: &str) -> String {
        let out = std::process::Command::new("niri")
            .args(["msg", "-j", what])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    fn ask(request: Request) {
        let path = apolline_niri::socket().unwrap();
        let mut stream = std::os::unix::net::UnixStream::connect(path).unwrap();
        stream.write_all(request.clone().line().as_bytes()).unwrap();
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply).unwrap();
        assert!(apolline_niri::is_ok(&reply), "{request:?}: {reply}");
    }

    #[test]
    fn a_real_niri_puts_them_in_their_places() {
        if std::env::var_os("APOLLINE_NIRI_PLACE").is_none() {
            return;
        }
        let mut state = State::default();
        for line in [
            format!(
                r#"{{"WorkspacesChanged":{{"workspaces":{}}}}}"#,
                niri("workspaces")
            ),
            format!(r#"{{"WindowsChanged":{{"windows":{}}}}}"#, niri("windows")),
        ] {
            state.apply(Event::parse(&line).unwrap());
        }
        let screen = state.focused_output().map(str::to_owned);
        let wanted = |app: &str, workspace, column, width| Placed {
            app: app.into(),
            title: None,
            run: None,
            screen: screen.clone(),
            workspace,
            column,
            width,
        };
        let (_, asked, run) = Arranging::start(
            vec![
                wanted("place-c", 1, 1, Some(40)),
                wanted("place-b", 1, 1, None),
                wanted("place-a", 2, 1, Some(70)),
            ],
            &state,
        );
        assert!(run.is_empty());
        for request in asked {
            ask(request);
        }
        std::thread::sleep(Duration::from_millis(700));

        let windows = apolline_niri::windows(&niri("windows"));
        let workspaces = apolline_niri::workspaces(&niri("workspaces"));
        let outputs = apolline_niri::outputs(&niri("outputs"));
        let now = remember(&windows, &workspaces, &outputs, 10, |_| None);
        let short: Vec<(&str, u8, u16)> = now
            .iter()
            .map(|p| (p.app.as_str(), p.workspace, p.column))
            .collect();
        assert_eq!(
            short,
            [("place-c", 1, 1), ("place-b", 1, 1), ("place-a", 2, 1)],
            "{now:#?}"
        );
        let width = |app: &str| now.iter().find(|p| p.app == app).and_then(|p| p.width);
        assert_eq!(width("place-c"), Some(40), "{now:#?}");
        assert_eq!(width("place-a"), Some(70), "{now:#?}");
    }
}

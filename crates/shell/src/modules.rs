use std::process::Stdio;
use std::time::Duration;

use apolline_config::Module;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::plugins::{Loaded, Runs};
use crate::shell::Message;

const LONGEST: usize = 48;
// A tooltip stays inside the bar's own surface, so it's one line.
const LONGEST_TIP: usize = 120;
// A touchpad sends a stream of small scrolls; one step per this long.
pub const SCROLL_GAP: Duration = Duration::from_millis(150);
// Long enough for a quick command like pactl to be done before the module looks again.
pub const RERUN_AFTER: Duration = Duration::from_millis(250);

// What a module's last line says: plain text, or Waybar's JSON with a tooltip and a class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Said {
    pub text: String,
    pub tooltip: Option<String>,
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Plain,
    Good,
    Warn,
    Urgent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Click,
    Right,
    Middle,
    Up,
    Down,
}

impl Act {
    pub fn command(self, module: &Module) -> Option<&str> {
        match self {
            Act::Click => module.click.as_deref(),
            Act::Right => module.right_click.as_deref(),
            Act::Middle => module.middle_click.as_deref(),
            Act::Up => module.scroll_up.as_deref(),
            Act::Down => module.scroll_down.as_deref(),
        }
    }
}

type Run = (usize, Module, u32, Option<Runs>);

// Each module runs under a count of its own: bumping it starts the command again. A plugin's
// pieces run as that plugin's.
pub fn subscription(modules: &[Module], runs: &[u32], plugins: &[Loaded]) -> Subscription<Message> {
    Subscription::batch(modules.iter().cloned().enumerate().map(|(index, module)| {
        let run = runs.get(index).copied().unwrap_or_default();
        let plugin = crate::plugins::runs(plugins, module.plugin.as_deref()).cloned();
        Subscription::run_with((index, module, run, plugin), run_one)
    }))
}

fn run_one((index, module, _, plugin): &Run) -> impl Stream<Item = Message> + use<> {
    let (index, module, plugin) = (*index, module.clone(), plugin.clone());
    iced::stream::channel(4, async move |mut output| {
        loop {
            let mut command = match &plugin {
                Some(plugin) => plugin.command(&module.command).await,
                None => in_group(&module.command),
            };
            command.stdout(Stdio::piped());
            match command.spawn() {
                Ok(mut child) => {
                    let mut group = crate::child::Group::new(child.id());
                    if let Some(stdout) = child.stdout.take() {
                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            if output
                                .send(Message::Module(index, said(&line)))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    let _ = child.wait().await;
                    group.let_go();
                }
                Err(err) => {
                    eprintln!("apolline: module {}: {err}", module.command);
                }
            }
            let wait = module.every.map_or(30, u64::from);
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
    })
}

// sh running `line` in a process group of its own, so the whole of it can be ended: on its way
// out the shell signals sh, and sh passes it on to the group, pipelines and loops included.
pub fn in_group(line: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("sh");
    command
        .args(["-c", &grouped(line)])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    crate::child::tie(command.as_std_mut(), libc::SIGTERM);
    command
}

// sh passes its own end on to the whole group, since the death signal from the shell reaches only
// sh itself. It ends with the command's own status.
fn grouped(command: &str) -> String {
    format!("trap 'trap - TERM; kill 0' TERM\n{{ {command}\n}} &\nwait $!")
}

pub fn said(line: &str) -> Said {
    let line = line.trim();
    let json = line
        .starts_with('{')
        .then(|| serde_json::from_str::<serde_json::Value>(line).ok())
        .flatten()
        .filter(|value| value.get("text").is_some());
    let Some(json) = json else {
        return Said {
            text: cut(&plain(line), LONGEST),
            ..Said::default()
        };
    };
    let words = |key: &str| match json.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let classes: Vec<String> = match json.get("class") {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(list)) => list
            .iter()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    let tooltip = plain(&words("tooltip"));
    let tooltip: Vec<&str> = tooltip
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let tooltip = cut(&tooltip.join(" · "), LONGEST_TIP);
    Said {
        text: cut(plain(&words("text")).trim(), LONGEST),
        tooltip: (!tooltip.is_empty()).then_some(tooltip),
        tone: tone(&classes),
    }
}

fn tone(classes: &[String]) -> Tone {
    let has = |names: &[&str]| classes.iter().any(|c| names.contains(&c.as_str()));
    if has(&["critical", "urgent", "error", "danger", "bad"]) {
        Tone::Urgent
    } else if has(&["warning", "warn", "high"]) {
        Tone::Warn
    } else if has(&["good", "ok", "success", "active", "on"]) {
        Tone::Good
    } else {
        Tone::Plain
    }
}

fn cut(line: &str, longest: usize) -> String {
    match line.char_indices().nth(longest) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_owned(),
    }
}

// Pango markup, which Waybar scripts often print, as the words alone.
pub fn plain(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag = &rest[open + 1..];
        let is_tag = tag
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/');
        match tag.find('>').filter(|_| is_tag) {
            Some(close) => rest = &tag[close + 1..],
            None => {
                out.push('<');
                rest = tag;
            }
        }
    }
    out.push_str(rest);
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        assert_eq!(said("  12°C  ").text, "12°C");
        let long = said(&"ж".repeat(60)).text;
        assert_eq!(long.chars().count(), LONGEST + 1);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn waybar_json_brings_a_tooltip_and_a_tone() {
        let line = r#"{"text": "<b>42%</b>", "tooltip": "Battery\n2 h left", "class": ["low", "warning"], "percentage": 42}"#;
        assert_eq!(
            said(line),
            Said {
                text: "42%".into(),
                tooltip: Some("Battery · 2 h left".into()),
                tone: Tone::Warn,
            }
        );
        assert_eq!(
            said(r#"{"text": "x", "class": "critical"}"#).tone,
            Tone::Urgent
        );
        assert_eq!(said(r#"{"oops": 1}"#).text, r#"{"oops": 1}"#);
        assert_eq!(said("a < b &amp; <i>c</i>").text, "a < b & c");
    }
}

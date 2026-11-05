use std::process::Stdio;
use std::time::Duration;

use apolline_config::Source;
use iced::futures::future::join_all;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{Loaded, Runs};

// Typing goes on for a moment before anything is asked, and a slow source is let go.
const PAUSE: Duration = Duration::from_millis(150);
const PATIENCE: Duration = Duration::from_secs(3);
const EACH: usize = 20;
const ALL: usize = 40;
const LONGEST: usize = 120;

// One result a plugin printed, as a JSON line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub plugin: String,
    pub title: String,
    pub about: String,
    pub icon: Option<String>,
    pub act: Act,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Run(String),
    Copy(String),
    Open(String),
    // A command run in your terminal.
    Terminal(String),
}

// {"title": "…", "about": "…", "icon": "…", and "run", "copy", "open" or "terminal"}.
pub fn found(plugin: &str, line: &str) -> Option<Found> {
    let json: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let raw = |key: &str| {
        json.get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|v| !v.trim().is_empty())
    };
    let text = |key: &str| raw(key).map(|v| crate::modules::plain(v).trim().to_owned());
    let act = if let Some(command) = raw("run") {
        Act::Run(command.to_owned())
    } else if let Some(words) = raw("copy") {
        Act::Copy(words.to_owned())
    } else if let Some(target) = raw("open") {
        Act::Open(target.trim().to_owned())
    } else {
        Act::Terminal(raw("terminal")?.to_owned())
    };
    Some(Found {
        plugin: plugin.to_owned(),
        title: cut(&text("title")?),
        about: cut(&text("about").unwrap_or_default()),
        icon: raw("icon").map(str::to_owned),
        act,
    })
}

fn cut(line: &str) -> String {
    line.chars().take(LONGEST).collect()
}

// The prefixes sources answer to, so the launcher shows only theirs after one.
pub fn prefixes(plugins: &[Loaded]) -> Vec<String> {
    plugins
        .iter()
        .flat_map(|l| &l.plugin.launchers)
        .filter_map(|s| s.prefix.clone())
        .collect()
}

// The sources a search goes to, each with what it's asked: the search without the prefix.
// One that starts with a prefix goes only to its sources; others to those without one.
pub fn asked(plugins: &[Loaded], query: &str) -> Vec<(Runs, Source, String)> {
    let query = query.trim_start();
    let prefixed = plugins
        .iter()
        .flat_map(|l| &l.plugin.launchers)
        .any(|s| s.prefix.as_deref().is_some_and(|p| query.starts_with(p)));
    plugins
        .iter()
        .flat_map(|l| l.plugin.launchers.iter().map(move |s| (&l.runs, s)))
        .filter_map(|(runs, source)| {
            let wanted = match &source.prefix {
                Some(prefix) => query.strip_prefix(prefix.as_str())?,
                None if prefixed => return None,
                None => query,
            };
            let wanted = wanted.trim();
            (!wanted.is_empty()).then(|| (runs.clone(), source.clone(), wanted.to_owned()))
        })
        .collect()
}

// Every source at once, after the pause; dropped, this ends them all.
pub async fn search(asked: Vec<(Runs, Source, String)>) -> Vec<Found> {
    tokio::time::sleep(PAUSE).await;
    let all = join_all(
        asked
            .iter()
            .map(|(runs, source, query)| ask(runs, source, query)),
    )
    .await;
    all.into_iter().flatten().take(ALL).collect()
}

async fn ask(runs: &Runs, source: &Source, query: &str) -> Vec<Found> {
    let mut command = tokio::process::Command::new("sh");
    // The search is the command's argument as well, never part of the line itself.
    command
        .args(["-c", &format!("{} \"$@\"", source.command), "sh", query])
        .env("APOLLINE_QUERY", query)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    runs.apply(&mut command).await;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("apolline: plugin {}: {err}", runs.plugin);
            return Vec::new();
        }
    };
    let Some(stdout) = child.stdout.take() else {
        return Vec::new();
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut results = Vec::new();
    let read = async {
        while results.len() < EACH
            && let Ok(Some(line)) = lines.next_line().await
        {
            results.extend(found(&runs.plugin, &line));
        }
    };
    let _ = tokio::time::timeout(PATIENCE, read).await;
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolline_config::Plugin;

    #[test]
    fn a_result_is_a_json_line_with_one_thing_to_do() {
        let line = r#"{"title": "printf(3)", "about": "formatted <b>output</b>", "icon": "book", "terminal": "man 3 printf"}"#;
        assert_eq!(
            found("man", line),
            Some(Found {
                plugin: "man".into(),
                title: "printf(3)".into(),
                about: "formatted output".into(),
                icon: Some("book".into()),
                act: Act::Terminal("man 3 printf".into()),
            })
        );
        let copy = found("x", r#"{"title": "Pi", "copy": "3.14159"}"#).unwrap();
        assert_eq!(
            (copy.about.as_str(), copy.act),
            ("", Act::Copy("3.14159".into()))
        );
        assert_eq!(
            found("x", r#"{"title": "Site", "open": "https://example.org"}"#).map(|f| f.act),
            Some(Act::Open("https://example.org".into()))
        );
        assert_eq!(found("x", r#"{"title": "Nothing to do"}"#), None);
        assert_eq!(found("x", r#"{"run": "true"}"#), None);
        assert_eq!(found("x", "not json"), None);
    }

    #[test]
    fn a_source_gets_the_search_as_an_argument_and_a_variable() {
        let runs = Runs {
            plugin: "echo".into(),
            dir: std::env::temp_dir(),
            env: vec![("APOLLINE_GREETING".into(), "hi".into())],
            secrets: Vec::new(),
        };
        let line = r#"printf '{"title": "%s %s", "copy": "%s"}\n' "$APOLLINE_GREETING" "$1" "$APOLLINE_QUERY""#;
        let source = Source {
            prefix: None,
            command: line.into(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let found = runtime.block_on(search(vec![(runs, source, "it's $HOME".into())]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "hi it's $HOME");
        assert_eq!(found[0].act, Act::Copy("it's $HOME".into()));
    }

    #[test]
    fn a_prefix_keeps_a_search_to_its_own_sources() {
        let (plugin, _) = Plugin::parse(
            "[plugin]\nname = \"p\"\n\n[[launcher]]\nprefix = \"man \"\ncommand = \"./man.sh\"\n\n[[launcher]]\ncommand = \"./all.sh\"\n",
        );
        let runs = Runs {
            plugin: "p".into(),
            dir: ".".into(),
            env: Vec::new(),
            secrets: Vec::new(),
        };
        let plugins = [Loaded { plugin, runs }];
        let asked = |query| -> Vec<(String, String)> {
            asked(&plugins, query)
                .into_iter()
                .map(|(_, source, query)| (source.command, query))
                .collect()
        };
        assert_eq!(asked("man printf"), [("./man.sh".into(), "printf".into())]);
        assert_eq!(asked("mango"), [("./all.sh".into(), "mango".into())]);
        assert_eq!(asked("man "), []);
        assert_eq!(asked(""), []);
        assert_eq!(prefixes(&plugins), ["man "]);
    }
}

use std::process::Stdio;
use std::time::Duration;

use apolline_config::Tile;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use super::{Loaded, Runs};
use crate::shell::Message;

// A state command that hangs is let go; the tile keeps what it last knew.
const STATE_PATIENCE: Duration = Duration::from_secs(5);
const SWITCH_PATIENCE: Duration = Duration::from_secs(15);
const EVERY: u32 = 10;

// What a tile's state command said: on or off, and a line under its label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub on: bool,
    pub detail: Option<String>,
}

// "on" or "off" with a detail on the next line, or {"on": true, "detail": "…"}.
pub fn state(output: &str) -> Option<State> {
    let output = output.trim();
    if output.starts_with('{') {
        let json: serde_json::Value = serde_json::from_str(output).ok()?;
        let detail = json.get("detail").and_then(|d| d.as_str());
        return Some(State {
            on: json.get("on")?.as_bool()?,
            detail: detail
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(Into::into),
        });
    }
    let mut lines = output.lines();
    let on = match lines.next()?.trim().to_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => true,
        "off" | "false" | "no" | "0" => false,
        _ => return None,
    };
    let detail = lines.next().map(str::trim).filter(|d| !d.is_empty());
    Some(State {
        on,
        detail: detail.map(Into::into),
    })
}

// A tile by "plugin.tile", as [panel] tiles places it.
pub fn find<'a>(plugins: &'a [Loaded], key: &str) -> Option<(&'a Runs, &'a Tile)> {
    let (plugin, name) = key.split_once('.')?;
    let loaded = plugins.iter().find(|l| l.plugin.name == plugin)?;
    let tile = loaded.plugin.tiles.iter().find(|t| t.name == name)?;
    Some((&loaded.runs, tile))
}

// The tiles' states, read while quick settings is open and every so often after.
pub fn subscription(plugins: &[Loaded], placed: &[String]) -> Subscription<Message> {
    Subscription::batch(
        placed
            .iter()
            .filter_map(|key| find(plugins, key).map(|(runs, tile)| (key, runs, tile)))
            .filter(|(_, _, tile)| tile.state.is_some())
            .map(|(key, runs, tile)| {
                Subscription::run_with((key.clone(), runs.clone(), tile.clone()), watch)
            }),
    )
}

fn watch((key, runs, tile): &(String, Runs, Tile)) -> impl Stream<Item = Message> + use<> {
    let (key, runs, tile) = (key.clone(), runs.clone(), tile.clone());
    iced::stream::channel(1, async move |mut output| {
        let Some(command) = tile.state.clone() else {
            return;
        };
        loop {
            if let Some(state) = read(&runs, &command).await
                && output
                    .send(Message::PluginTile(key.clone(), Some(state)))
                    .await
                    .is_err()
            {
                return;
            }
            let every = tile.every.unwrap_or(EVERY);
            tokio::time::sleep(Duration::from_secs(u64::from(every))).await;
        }
    })
}

// Runs the tile's own command to switch it, then reads where it ended up.
pub async fn switch(runs: Runs, tile: Tile, on: bool) -> Option<State> {
    let command = match on {
        true => tile.on.as_ref().or(tile.toggle.as_ref()),
        false => tile.off.as_ref().or(tile.toggle.as_ref()),
    }?;
    run(&runs, command, SWITCH_PATIENCE).await;
    match &tile.state {
        Some(state) => read(&runs, state).await,
        // With nothing to ask, the tile is what it was last switched to.
        None => Some(State { on, detail: None }),
    }
}

async fn read(runs: &Runs, command: &str) -> Option<State> {
    state(&run(runs, command, STATE_PATIENCE).await?)
}

async fn run(runs: &Runs, line: &str, patience: Duration) -> Option<String> {
    let mut command = tokio::process::Command::new("sh");
    command
        .args(["-c", line])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    runs.apply(&mut command).await;
    let out = tokio::time::timeout(patience, command.output()).await;
    match out {
        Ok(Ok(out)) => Some(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(Err(err)) => {
            eprintln!("apolline: plugin {}: {err}", runs.plugin);
            None
        }
        Err(_) => {
            eprintln!("apolline: plugin {}: “{line}” took too long", runs.plugin);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_reads_words_or_json() {
        assert_eq!(
            state("on\n"),
            Some(State {
                on: true,
                detail: None
            })
        );
        assert_eq!(
            state("Off\n  Since 12:00 \n"),
            Some(State {
                on: false,
                detail: Some("Since 12:00".into())
            })
        );
        assert_eq!(
            state(r#"{"on": true, "detail": "Home"}"#),
            Some(State {
                on: true,
                detail: Some("Home".into())
            })
        );
        assert_eq!(state(r#"{"on": "yes"}"#), None);
        assert_eq!(state("maybe"), None);
        assert_eq!(state(""), None);
    }
}

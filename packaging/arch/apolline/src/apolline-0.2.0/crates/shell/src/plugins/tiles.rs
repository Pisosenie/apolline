use std::process::Stdio;
use std::time::Duration;

use apolline_config::Tile;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use tokio::io::AsyncReadExt;

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

// Runs the tile's own command to switch it, then reads where it ended up; None when the
// command failed, and what it was switched to when it can't be read.
pub async fn switch(runs: Runs, tile: Tile, on: bool) -> Option<State> {
    let command = match on {
        true => tile.on.as_ref().or(tile.toggle.as_ref()),
        false => tile.off.as_ref().or(tile.toggle.as_ref()),
    }?;
    if !switched(&runs, command).await {
        return None;
    }
    let switched = State { on, detail: None };
    match &tile.state {
        Some(state) => Some(read(&runs, state).await.unwrap_or(switched)),
        // With nothing to ask, the tile is what it was last switched to.
        None => Some(switched),
    }
}

// Done when the command is: what it prints isn't waited for, so a daemon it starts with `&`
// doesn't hold the click up, and stays running.
async fn switched(runs: &Runs, line: &str) -> bool {
    let Ok(mut child) = runs.command(line).await.spawn() else {
        eprintln!("apolline: plugin {}: “{line}” didn't start", runs.plugin);
        return false;
    };
    let mut group = crate::child::Group::new(child.id());
    match tokio::time::timeout(SWITCH_PATIENCE, child.wait()).await {
        Ok(Ok(status)) => {
            group.let_go();
            status.success()
        }
        Ok(Err(err)) => {
            eprintln!("apolline: plugin {}: {err}", runs.plugin);
            false
        }
        Err(_) => {
            eprintln!("apolline: plugin {}: “{line}” took too long", runs.plugin);
            false
        }
    }
}

// What its state command says, heard out or given up on; then it ends whole, with whatever it
// started.
async fn read(runs: &Runs, line: &str) -> Option<State> {
    let mut command = runs.command(line).await;
    command.stdout(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| eprintln!("apolline: plugin {}: {err}", runs.plugin))
        .ok()?;
    let _group = crate::child::Group::new(child.id());
    let mut stdout = child.stdout.take()?.take(super::OUTPUT);
    let mut out = Vec::new();
    match tokio::time::timeout(STATE_PATIENCE, stdout.read_to_end(&mut out)).await {
        Ok(Ok(_)) => state(&String::from_utf8_lossy(&out)),
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
    fn a_switch_waits_for_its_command_not_for_what_it_left_running() {
        let runs = Runs {
            plugin: "t".into(),
            dir: std::env::temp_dir(),
            env: Vec::new(),
            secrets: Vec::new(),
        };
        let tile = |toggle: &str| Tile {
            name: "t".into(),
            label: "T".into(),
            icon: None,
            state: None,
            on: None,
            off: None,
            toggle: Some(toggle.into()),
            every: None,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let started = std::time::Instant::now();
        let daemon = runtime.block_on(switch(runs.clone(), tile("sleep 3 &"), true));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(
            daemon,
            Some(State {
                on: true,
                detail: None
            })
        );
        let failed = runtime.block_on(switch(runs, tile("exit 3"), true));
        assert_eq!(failed, None);
    }

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

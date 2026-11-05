use super::*;
use crate::plugins::search;
use crate::plugins::tiles::{self, State};

impl Shell {
    pub(super) fn tile_is(&mut self, key: String, state: State) {
        match self.tiles.iter_mut().find(|(k, _)| *k == key) {
            Some((_, was)) => *was = state,
            None => self.tiles.push((key, state)),
        }
    }

    // Shown switched at once; the plugin's state command has the last word. One switch at a time:
    // a second click while the first is under way would undo it.
    pub(super) fn switch_tile(&mut self, key: String) -> Task<Message> {
        if self.switching.contains(&key) {
            return Task::none();
        }
        let Some((runs, tile)) = tiles::find(&self.plugins, &key) else {
            return Task::none();
        };
        let (runs, tile) = (runs.clone(), tile.clone());
        let was = self
            .tiles
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, state)| state.clone())
            .unwrap_or_default();
        let on = !was.on;
        self.tile_is(key.clone(), State { on, ..was.clone() });
        if self.demo {
            return Task::none();
        }
        self.switching.push(key.clone());
        Task::perform(tiles::switch(runs, tile, on), move |now| {
            Message::PluginSwitched(key.clone(), was.clone(), now)
        })
    }

    // A switch that failed shows the tile as it was before.
    pub(super) fn tile_switched(&mut self, key: String, was: State, now: Option<State>) {
        self.switching.retain(|k| *k != key);
        self.tile_is(key, now.unwrap_or(was));
    }
}

impl Shell {
    // Plugins' launcher sources, asked once typing pauses; a newer search ends the one before.
    pub(super) fn ask_plugins(&mut self, query: &str) -> Task<Message> {
        self.searching = None;
        let asked = search::asked(&self.plugins, query);
        if asked.is_empty() {
            self.launcher.set_plugin_found(Vec::new());
            return Task::none();
        }
        let (task, handle) = Task::run(search::search(asked), Message::PluginFound).abortable();
        self.searching = Some(handle.abort_on_drop());
        task
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_waits_out_its_switch_and_goes_back_when_it_fails() {
        let (mut shell, _) = Shell::new(None, true);
        let (off, on) = (
            State::default(),
            State {
                on: true,
                detail: None,
            },
        );
        let key = "p.t".to_owned();
        shell.tile_is(key.clone(), on.clone());
        shell.switching.push(key.clone());
        let _ = shell.update(Message::PluginTile(key.clone(), Some(off.clone())));
        assert_eq!(
            shell.tiles,
            [(key.clone(), on)],
            "a read mid-switch is let go"
        );
        let _ = shell.update(Message::PluginSwitched(key.clone(), off.clone(), None));
        assert_eq!(shell.tiles, [(key.clone(), off)]);
        assert!(shell.switching.is_empty());
    }
}

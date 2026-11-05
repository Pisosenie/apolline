use super::*;
use crate::plugins::tiles::{self, State};

impl Shell {
    pub(super) fn tile_is(&mut self, key: String, state: State) {
        match self.tiles.iter_mut().find(|(k, _)| *k == key) {
            Some((_, was)) => *was = state,
            None => self.tiles.push((key, state)),
        }
    }

    // Shown switched at once; the plugin's state command has the last word.
    pub(super) fn switch_tile(&mut self, key: String) -> Task<Message> {
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
        self.tile_is(key.clone(), State { on, ..was });
        if self.demo {
            return Task::none();
        }
        Task::perform(tiles::switch(runs, tile, on), move |state| {
            Message::PluginTile(key.clone(), state)
        })
    }
}

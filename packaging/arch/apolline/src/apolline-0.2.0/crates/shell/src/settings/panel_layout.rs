use super::*;

use apolline_config::Panel;

fn tile_label(name: &str) -> String {
    match name {
        "wifi" => "Wi-Fi",
        "bluetooth" => "Bluetooth",
        "quiet" => "Do not disturb",
        "awake" => "Keep awake",
        "night" => "Night light",
        "power" => "Power",
        "vpn" => "VPN",
        name => return name.to_owned(),
    }
    .to_owned()
}

fn slider_label(name: &str) -> String {
    match name {
        "volume" => "Volume",
        "apps" => "Each app's volume",
        "mic" => "Microphone",
        "devices" => "Speakers and mics",
        "brightness" => "Brightness",
        name => return name.to_owned(),
    }
    .to_owned()
}

fn part_label(name: &str) -> String {
    match name {
        "tiles" => "Tiles",
        "sliders" => "Sliders",
        "modes" => "Modes",
        "player" => "What's playing",
        "notifications" => "Notifications",
        name => return name.to_owned(),
    }
    .to_owned()
}

fn board_of(shown: &[String], all: &[&str]) -> board::Board {
    board::Board {
        zones: vec![shown.to_vec()],
        spare: all
            .iter()
            .filter(|name| !shown.iter().any(|s| s == *name))
            .map(|name| (*name).to_owned())
            .collect(),
    }
}

impl Settings {
    pub(super) fn panel_boards(&self) -> [board::Board; 3] {
        let panel = &self.config.panel;
        // Enabled plugins' tiles wait beside the built-in ones.
        let plugin_tiles = self.plugin_parts(super::plugins::tiles);
        let tiles: Vec<&str> = Panel::TILES
            .into_iter()
            .chain(plugin_tiles.iter().map(String::as_str))
            .collect();
        [
            board_of(&panel.tiles, &tiles),
            board_of(&panel.sliders, &Panel::SLIDERS),
            board_of(&panel.parts, &Panel::PARTS),
        ]
    }

    pub(super) fn quick_settings(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let [tiles, sliders, parts] = self.panel_boards();
        group(
            "Quick settings",
            vec![
                labelled(
                    "From top to bottom",
                    board::view(
                        parts,
                        &["Shown"],
                        part_label,
                        palette,
                        self.parts_held.clone(),
                        Message::PartsHold,
                        Message::BoardMove,
                        Message::PartsDrop,
                    ),
                ),
                labelled(
                    "Tiles, two to a row",
                    board::view(
                        tiles,
                        &["Shown"],
                        tile_label,
                        palette,
                        self.tiles_held.clone(),
                        Message::TilesHold,
                        Message::BoardMove,
                        Message::TilesDrop,
                    ),
                ),
                labelled(
                    "Sliders",
                    board::view(
                        sliders,
                        &["Shown"],
                        slider_label,
                        palette,
                        self.sliders_held.clone(),
                        Message::SlidersHold,
                        Message::BoardMove,
                        Message::SlidersDrop,
                    ),
                ),
                note("Drag to reorder, and below a row to take a piece out of quick settings."),
                setting(
                    "Lyrics",
                    "The line being sung, under what's playing; asks lrclib.net by title and artist",
                    switch(self.config.panel.lyrics, Message::PanelLyrics, palette),
                ),
            ],
        )
    }

    // Where a dragged piece of quick settings lands, written to [panel].
    pub(super) fn panel_drop(&mut self, which: usize, at: iced::Point, width: f32) {
        let (held, key, label): (_, _, fn(&str) -> String) = match which {
            0 => (self.tiles_held.take(), "tiles", tile_label),
            1 => (self.sliders_held.take(), "sliders", slider_label),
            _ => (self.parts_held.take(), "parts", part_label),
        };
        let Some(held) = held else {
            return;
        };
        let board = self.panel_boards()[which.min(2)].clone();
        let shown = board::dropped(&board, &held.name, at, label, width)
            .zones
            .first()
            .cloned()
            .unwrap_or_default();
        let all: &[&str] = match which {
            0 => &Panel::TILES,
            1 => &Panel::SLIDERS,
            _ => &Panel::PARTS,
        };
        // As it comes, the file says nothing.
        if shown.iter().map(String::as_str).eq(all.iter().copied()) {
            self.edit(|e| e.unset("panel", key));
        } else {
            self.edit(|e| e.set_list("panel", key, &shown));
        }
    }
}

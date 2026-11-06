use super::*;

// A change to [dock], each option written only when it isn't what an empty config means.
#[derive(Clone, Copy, Debug)]
pub(super) enum DockSet {
    Show(bool),
    Edge(Edge),
    Autohide(bool),
    Size(u16),
    Magnify(bool),
}

impl Settings {
    pub(super) fn dock_rows(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let dock = &self.config.dock;
        let mut rows = vec![setting(
            "Show the dock",
            "Pinned and open apps along an edge, with a dot for each window and a count of unread notifications",
            switch(dock.show, |on| Message::Dock(DockSet::Show(on)), palette),
        )];
        if !dock.show {
            return rows;
        }
        rows.extend([
            setting(
                "Edge",
                "",
                segmented(
                    [
                        (Edge::Bottom, "Bottom", None),
                        (Edge::Left, "Left", None),
                        (Edge::Right, "Right", None),
                    ],
                    dock.position,
                    |edge| Message::Dock(DockSet::Edge(edge)),
                ),
            ),
            setting(
                "Hide it",
                "It comes out when the pointer reaches the screen edge",
                switch(dock.autohide, |on| Message::Dock(DockSet::Autohide(on)), palette),
            ),
            setting(
                "Icon size",
                "",
                value_slider(
                    apolline_config::Dock::SIZES,
                    dock.size,
                    |size| Message::Dock(DockSet::Size(size)),
                    "px",
                    palette,
                ),
            ),
            setting(
                "Magnify",
                "Icons near the pointer grow",
                switch(dock.magnify, |on| Message::Dock(DockSet::Magnify(on)), palette),
            ),
            note(
                "The dock shares the bar's pinned apps. Drag an icon to move it, off the dock to unpin it; right-click for its windows.",
            ),
        ]);
        rows
    }

    pub(super) fn set_dock(&mut self, change: DockSet) {
        let defaults = apolline_config::Dock::default();
        match change {
            DockSet::Show(on) => self.edit(|e| match on {
                true => e.set("dock", "show", true),
                false => e.unset("dock", "show"),
            }),
            DockSet::Edge(edge) if edge == defaults.position => {
                self.edit(|e| e.unset("dock", "position"))
            }
            DockSet::Edge(edge) => self.edit(|e| e.set("dock", "position", edge.key())),
            DockSet::Autohide(on) if on == defaults.autohide => {
                self.edit(|e| e.unset("dock", "autohide"))
            }
            DockSet::Autohide(on) => self.edit(|e| e.set("dock", "autohide", on)),
            DockSet::Size(size) if size == defaults.size => self.stage(|e| e.unset("dock", "size")),
            DockSet::Size(size) => self.stage(|e| e.set("dock", "size", i64::from(size))),
            DockSet::Magnify(on) if on == defaults.magnify => {
                self.edit(|e| e.unset("dock", "magnify"))
            }
            DockSet::Magnify(on) => self.edit(|e| e.set("dock", "magnify", on)),
        }
    }
}

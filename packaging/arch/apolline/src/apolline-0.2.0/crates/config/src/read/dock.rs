use toml::de::DeTable;

use super::Reader;
use crate::Dock;

impl Reader<'_> {
    pub(super) fn dock(&mut self, table: &DeTable, dock: &mut Dock) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "show" => {
                    if let Some(v) = self.boolean(key, value) {
                        dock.show = v
                    }
                }
                "position" => {
                    if let Some(v) = self.choice(key, value, &Dock::EDGES) {
                        dock.position = v
                    }
                }
                "autohide" => {
                    if let Some(v) = self.boolean(key, value) {
                        dock.autohide = v
                    }
                }
                "size" => {
                    if let Some(v) = self.number(key, value, Dock::SIZES) {
                        dock.size = v
                    }
                }
                "magnify" => {
                    if let Some(v) = self.boolean(key, value) {
                        dock.magnify = v
                    }
                }
                "skip" => {
                    if let Some(v) = self.strings(key, value) {
                        dock.skip = v
                    }
                }
                _ => self.unknown(key, "option", &Dock::OPTIONS),
            }
        }
    }
}

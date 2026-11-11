use toml::de::DeValue;

use super::{Key, Reader, Value};
use crate::{Diagnostic, Placed};

const OPTIONS: [&str; 7] = [
    "app",
    "title",
    "run",
    "screen",
    "workspace",
    "column",
    "width",
];
const SHAPE: &str = r#"a list like [{ app = "kitty", workspace = 1, column = 1, width = 50 }]"#;

impl Reader<'_> {
    // A mode's windows = [{ app = "kitty", run = "kitty", screen = "DP-1", workspace = 1, … }].
    pub(super) fn placed(&mut self, key: &Key, value: &Value) -> Option<Vec<Placed>> {
        let Some(items) = value.get_ref().as_array() else {
            return self.wrong(key, value, SHAPE);
        };
        let mut placed = Vec::new();
        for item in items {
            let DeValue::Table(table) = item.get_ref() else {
                self.wrong::<()>(key, item, SHAPE);
                continue;
            };
            let mut window = Placed {
                app: String::new(),
                title: None,
                run: None,
                screen: None,
                workspace: 1,
                column: 1,
                width: None,
            };
            let text = |reader: &mut Self, key: &Key, value: &Value| {
                reader
                    .string(key, value)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
            };
            for (name, value) in table.iter() {
                match name.get_ref().as_ref() {
                    "app" => window.app = text(self, name, value).unwrap_or_default(),
                    "title" => window.title = text(self, name, value),
                    "run" => window.run = text(self, name, value),
                    "screen" => window.screen = text(self, name, value),
                    "workspace" => {
                        if let Some(n) = self.number(name, value, 1..=99) {
                            window.workspace = n as u8;
                        }
                    }
                    "column" => {
                        if let Some(n) = self.number(name, value, 1..=99) {
                            window.column = n;
                        }
                    }
                    "width" => {
                        window.width = self.number(name, value, Placed::WIDTHS).map(|n| n as u8);
                    }
                    _ => self.unknown(name, "option", &OPTIONS),
                }
            }
            if window.app.is_empty() {
                self.diagnostics.push(
                    Diagnostic::new(self.line(item.span()), "a window needs its app")
                        .hint("the app id niri shows, like app = \"firefox\""),
                );
                continue;
            }
            placed.push(window);
        }
        Some(placed)
    }
}

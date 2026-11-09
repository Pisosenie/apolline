use toml::de::{DeTable, DeValue};

use super::{Key, Reader, Value, key_combo};
use crate::{Diagnostic, KeyAction};

const ACTION: &str = r#"a command in quotes, or { niri = "close-window" }"#;

impl Reader<'_> {
    // [keys]: "Mod+Return" = "kitty", "Mod+C" = { niri = "close-window" }.
    pub(super) fn global_keys(&mut self, table: &DeTable, keys: &mut Vec<(String, KeyAction)>) {
        for (combo, value) in table.iter() {
            let Some(written) = key_combo(combo.get_ref()) else {
                self.diagnostics.push(
                    Diagnostic::new(
                        self.line(combo.span()),
                        format!("“{}” is not a key combination", combo.get_ref()),
                    )
                    .hint("write it like Mod+T or Mod+Shift+Return"),
                );
                continue;
            };
            if let Some(action) = self.key_action(combo, value) {
                keys.retain(|(had, _)| *had != written);
                keys.push((written, action));
            }
        }
    }

    fn key_action(&mut self, key: &Key, value: &Value) -> Option<KeyAction> {
        match value.get_ref() {
            DeValue::String(command) if !command.trim().is_empty() => {
                Some(KeyAction::Run(command.trim().to_owned()))
            }
            DeValue::Table(table) => {
                let mut action = None;
                for (name, value) in table.iter() {
                    if name.get_ref() != "niri" {
                        self.unknown(name, "option", &["niri"]);
                        continue;
                    }
                    let text = self.string(name, value)?;
                    action = KeyAction::niri(text);
                    if action.is_none() {
                        self.diagnostics.push(
                            Diagnostic::new(
                                self.line(value.span()),
                                format!("“{text}” is not one of niri's actions"),
                            )
                            .hint("an action and its plain arguments, like focus-workspace 3"),
                        );
                    }
                }
                action
            }
            _ => self.wrong(key, value, ACTION),
        }
    }
}

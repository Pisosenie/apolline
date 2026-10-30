use std::path::Path;
use std::{fs, io};

use apolline_theme::Mode;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::{Accent, Notifications};

#[derive(Debug)]
pub struct Editor {
    doc: DocumentMut,
}

impl Editor {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err.to_string()),
        };
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        text.parse::<DocumentMut>()
            .map(|doc| Self { doc })
            .map_err(|err| format!("fix the syntax error first: {}", err.message()))
    }

    pub fn set_wallpaper(&mut self, mode: Mode, wallpaper: &str) {
        put(self.mode(mode), "wallpaper", wallpaper.into());
    }

    pub fn set_accent(&mut self, mode: Mode, accent: Accent) {
        let text = match accent {
            Accent::Auto => "auto".to_owned(),
            Accent::Fixed(color) => color.to_hex(),
        };
        put(self.mode(mode), "accent", text.into());
    }

    pub fn set_notifications(&mut self, mode: Mode, notifications: Notifications) {
        let text = match notifications {
            Notifications::All => "all",
            Notifications::Mentions => "mentions",
            Notifications::Hold => "hold",
        };
        put(self.mode(mode), "notifications", text.into());
    }

    pub fn set_open(&mut self, mode: Mode, open: &[String]) {
        let list: Array = open.iter().map(String::as_str).collect();
        put(self.mode(mode), "open", list.into());
    }

    pub fn set(&mut self, section: &str, key: &str, value: impl Into<Value>) {
        put(table(self.doc.as_table_mut(), section), key, value.into());
    }

    pub fn set_speed(&mut self, section: &str, speed: f32) {
        self.set(section, "speed", (f64::from(speed) * 100.0).round() / 100.0);
    }

    pub fn set_list(&mut self, section: &str, key: &str, items: &[String]) {
        let list: Array = items.iter().map(String::as_str).collect();
        self.set(section, key, list);
    }

    pub fn section(&self, mode: Mode) -> String {
        let mut doc = DocumentMut::new();
        if let Some(found) = self.doc.get("mode").and_then(|m| m.get(mode.key())) {
            table(doc.as_table_mut(), "mode").set_implicit(true);
            doc["mode"][mode.key()] = found.clone();
        }
        doc.to_string()
    }

    pub fn sections(&self, names: &[&str]) -> String {
        let mut doc = DocumentMut::new();
        for name in names {
            if let Some(found) = self.doc.get(name) {
                doc[*name] = found.clone();
            }
        }
        doc.to_string()
    }

    pub fn text(&self) -> String {
        self.doc.to_string()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let staged = path.with_extension("toml.new");
        fs::write(&staged, self.text())?;
        fs::rename(&staged, path)
    }

    fn mode(&mut self, mode: Mode) -> &mut Table {
        let modes = table(self.doc.as_table_mut(), "mode");
        modes.set_implicit(true);
        table(modes, mode.key())
    }
}

fn table<'a>(parent: &'a mut Table, key: &str) -> &'a mut Table {
    if !parent.get(key).is_some_and(Item::is_table) {
        parent.insert(key, toml_edit::table());
    }
    parent[key].as_table_mut().expect("just inserted a table")
}

fn put(table: &mut Table, key: &str, mut value: Value) {
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use apolline_theme::Rgba;

    const SHIPPED: &str = include_str!("../../../assets/config.toml");

    #[test]
    fn edits_keep_comments_and_order() {
        let mut editor = Editor::parse(SHIPPED).unwrap();
        editor.set_accent(Mode::Work, Accent::Fixed(Rgba::hex(0xc084fc)));
        let text = editor.text();

        assert!(
            text.contains(
                r##"accent = "#c084fc"            # "auto" takes it from the wallpaper"##
            )
        );
        assert!(text.starts_with("# Apolline reads this file when it starts."));
        assert_eq!(text.lines().count(), SHIPPED.lines().count());
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        assert_eq!(
            config.mode(Mode::Work).accent,
            Accent::Fixed(Rgba::hex(0xc084fc))
        );
    }

    #[test]
    fn missing_sections_are_created() {
        let mut editor = Editor::parse("[layout]\ngaps = 4\n").unwrap();
        editor.set_open(Mode::Play, &["steam".into(), "discord".into()]);
        editor.set_notifications(Mode::Play, Notifications::Hold);
        editor.set("layout", "borders", false);
        editor.set_speed("touchpad", 0.3);
        editor.set_list("keyboard", "layouts", &["us".into(), "ru".into()]);

        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(config.mode(Mode::Play).open, ["steam", "discord"]);
        assert_eq!(config.mode(Mode::Play).notifications, Notifications::Hold);
        assert!(!config.layout.borders);
        assert_eq!(config.layout.gaps, 4);
        assert_eq!(config.touchpad.map(|t| t.speed), Some(0.3));
        assert!(config.touchpad.is_some_and(|t| t.tap));
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert!(editor.text().contains("speed = 0.3\n"));
        assert!(
            editor
                .sections(&["touchpad", "mouse"])
                .starts_with("[touchpad]\n")
        );
        assert!(editor.section(Mode::Play).starts_with("[mode.play]\n"));
        assert_eq!(editor.section(Mode::Evening), "");
    }

    #[test]
    fn a_broken_file_is_never_rewritten() {
        assert!(
            Editor::parse("[layout\ngaps = 4\n")
                .unwrap_err()
                .starts_with("fix the syntax error first")
        );
    }
}

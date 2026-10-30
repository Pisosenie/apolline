use std::path::Path;
use std::{fs, io};

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use crate::{Accent, ModeSettings, Notifications, Rule};

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

    pub fn set(&mut self, section: &str, key: &str, value: impl Into<Value>) {
        put(table(self.doc.as_table_mut(), section), key, value.into());
    }

    pub fn set_speed(&mut self, section: &str, speed: f32) {
        self.set(section, "speed", (f64::from(speed) * 100.0).round() / 100.0);
    }

    pub fn set_pairs(&mut self, section: &str, key: &str, pairs: &[(String, String)]) {
        if pairs.is_empty() {
            self.unset(section, key);
        } else {
            self.set(section, key, keys(pairs));
        }
    }

    pub fn set_list(&mut self, section: &str, key: &str, items: &[String]) {
        let list: Array = items.iter().map(String::as_str).collect();
        self.set(section, key, list);
    }

    pub fn add_mode(&mut self, mode: &ModeSettings) -> usize {
        let mut table = Table::new();
        table.insert("name", mode.name.as_str().into());
        table.insert("icon", mode.icon.as_str().into());
        if let Some(wallpaper) = &mode.wallpaper {
            table.insert("wallpaper", wallpaper.as_str().into());
        }
        if let Some(accent) = mode.accent {
            table.insert("accent", accent_text(accent).into());
        }
        table.insert(
            "notifications",
            notifications_text(mode.notifications).into(),
        );
        let open: Array = mode.open.iter().map(String::as_str).collect();
        table.insert("open", Item::Value(open.into()));
        table.insert("performance", mode.performance.into());
        table.insert("keep_awake", mode.keep_awake.into());
        if !mode.keys.is_empty() {
            table.insert("keys", Item::Value(keys(&mode.keys).into()));
        }
        let modes = self.modes();
        modes.push(table);
        modes.len() - 1
    }

    pub fn remove_mode(&mut self, index: usize) {
        let modes = self.modes();
        if index < modes.len() {
            modes.remove(index);
        }
    }

    pub fn move_mode(&mut self, index: usize, to: usize) {
        let modes = self.modes();
        if index >= modes.len() || to >= modes.len() || index == to {
            return;
        }
        let tables: Vec<Table> = modes.iter().cloned().collect();
        let mut order: Vec<Table> = tables;
        let moved = order.remove(index);
        order.insert(to, moved);
        modes.clear();
        for table in order {
            modes.push(table);
        }
    }

    pub fn set_mode(&mut self, index: usize, key: &str, value: impl Into<Value>) {
        if let Some(table) = self.modes().get_mut(index) {
            put(table, key, value.into());
        }
    }

    pub fn unset_mode(&mut self, index: usize, key: &str) {
        if let Some(table) = self.modes().get_mut(index) {
            table.remove(key);
        }
    }

    pub fn set_mode_accent(&mut self, index: usize, accent: Option<Accent>) {
        match accent {
            Some(accent) => self.set_mode(index, "accent", accent_text(accent)),
            None => self.unset_mode(index, "accent"),
        }
    }

    pub fn set_mode_notifications(&mut self, index: usize, notifications: Notifications) {
        self.set_mode(index, "notifications", notifications_text(notifications));
    }

    pub fn set_mode_list(&mut self, index: usize, key: &str, items: &[String]) {
        let list: Array = items.iter().map(String::as_str).collect();
        self.set_mode(index, key, list);
    }

    pub fn set_mode_keys(&mut self, index: usize, list: &[(String, String)]) {
        if list.is_empty() {
            self.unset_mode(index, "keys");
        } else {
            self.set_mode(index, "keys", keys(list));
        }
    }

    pub fn add_rule(&mut self, rule: &Rule) {
        let mut table = Table::new();
        table.insert("app", rule.app.as_str().into());
        if let Some(floating) = rule.floating {
            table.insert("floating", floating.into());
        }
        if let Some(width) = rule.width {
            table.insert("width", width.key().into());
        }
        if let Some(opacity) = rule.opacity {
            table.insert(
                "opacity",
                ((f64::from(opacity) * 100.0).round() / 100.0).into(),
            );
        }
        list(self.doc.as_table_mut(), "rule").push(table);
    }

    pub fn remove_rule(&mut self, index: usize) {
        let rules = list(self.doc.as_table_mut(), "rule");
        if index < rules.len() {
            rules.remove(index);
        }
    }

    pub fn unset(&mut self, section: &str, key: &str) {
        if let Some(table) = self.doc.get_mut(section).and_then(Item::as_table_mut) {
            table.remove(key);
        }
    }

    pub fn lists(&self, name: &str) -> String {
        let mut doc = DocumentMut::new();
        if let Some(found) = self.doc.get(name).filter(|item| item.is_array_of_tables()) {
            doc.insert(name, found.clone());
        }
        doc.to_string()
    }

    pub fn set_accent(&mut self, accent: Accent) {
        self.set("desktop", "accent", accent_text(accent));
    }

    pub fn sections(&self, names: &[&str]) -> String {
        let mut doc = DocumentMut::new();
        for name in names {
            if let Some(found) = self.doc.get(name)
                && !found.is_array_of_tables()
            {
                doc[*name] = found.clone();
            }
        }
        doc.to_string()
    }

    pub fn mode_section(&self, index: usize) -> String {
        let mut doc = DocumentMut::new();
        if let Some(found) = self
            .doc
            .get("mode")
            .and_then(Item::as_array_of_tables)
            .and_then(|modes| modes.get(index))
        {
            let mut list = ArrayOfTables::new();
            list.push(found.clone());
            doc.insert("mode", Item::ArrayOfTables(list));
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

    fn modes(&mut self) -> &mut ArrayOfTables {
        let root = self.doc.as_table_mut();
        if !root.get("mode").is_some_and(Item::is_array_of_tables) {
            let mut list = ArrayOfTables::new();
            if let Some(legacy) = root.get("mode").and_then(Item::as_table) {
                for (key, item) in legacy.iter() {
                    let Some(old) = item.as_table() else {
                        continue;
                    };
                    let mut title = key.to_owned();
                    if let Some(first) = title.get_mut(..1) {
                        first.make_ascii_uppercase();
                    }
                    let template = ModeSettings::template(&title);
                    let mut table = Table::new();
                    table.insert("name", title.as_str().into());
                    table.insert("icon", template.icon.as_str().into());
                    for (key, value) in old.iter() {
                        table.insert(key, value.clone());
                    }
                    table.insert("performance", template.performance.into());
                    table.insert("keep_awake", template.keep_awake.into());
                    list.push(table);
                }
            }
            root.insert("mode", Item::ArrayOfTables(list));
        }
        root["mode"]
            .as_array_of_tables_mut()
            .expect("just made a list of modes")
    }
}

pub fn accent_text(accent: Accent) -> String {
    match accent {
        Accent::Auto => "auto".to_owned(),
        Accent::Fixed(color) => color.to_hex(),
    }
}

fn notifications_text(notifications: Notifications) -> &'static str {
    match notifications {
        Notifications::All => "all",
        Notifications::Mentions => "mentions",
        Notifications::Hold => "hold",
    }
}

fn keys(list: &[(String, String)]) -> InlineTable {
    let mut table = InlineTable::new();
    for (combo, command) in list {
        table.insert(combo, command.as_str().into());
    }
    table
}

fn list<'a>(parent: &'a mut Table, key: &str) -> &'a mut ArrayOfTables {
    if !parent.get(key).is_some_and(Item::is_array_of_tables) {
        parent.insert(key, Item::ArrayOfTables(ArrayOfTables::new()));
    }
    parent[key]
        .as_array_of_tables_mut()
        .expect("just inserted a list")
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
        editor.set_accent(Accent::Fixed(Rgba::hex(0xc084fc)));
        let text = editor.text();

        assert!(text.contains(r##"accent = "#c084fc"            # "auto" follows the wallpaper"##));
        assert!(text.starts_with("# Apolline reads this file when it starts."));
        assert_eq!(text.lines().count(), SHIPPED.lines().count());
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        assert_eq!(config.desktop.accent, Accent::Fixed(Rgba::hex(0xc084fc)));
        assert!(config.modes.is_empty());
    }

    #[test]
    fn modes_are_added_edited_moved_and_removed() {
        let mut editor = Editor::parse("[layout]\ngaps = 4\n").unwrap();
        let play = editor.add_mode(&ModeSettings::template("Play"));
        let work = editor.add_mode(&ModeSettings::template("Work"));
        editor.set_mode_list(play, "open", &["steam".into(), "discord".into()]);
        editor.set_mode_keys(play, &[("Mod+G".into(), "steam".into())]);
        editor.set_mode_accent(work, None);
        editor.set_mode_notifications(work, Notifications::Hold);
        editor.move_mode(work, 0);
        editor.set("general", "mode", "play");

        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        let names: Vec<&str> = config.modes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Work", "Play"]);
        assert_eq!(config.modes[1].open, ["steam", "discord"]);
        assert_eq!(
            config.modes[1].keys,
            [("Mod+G".to_owned(), "steam".to_owned())]
        );
        assert!(config.modes[1].performance);
        assert_eq!(config.modes[0].accent, None);
        assert_eq!(config.modes[0].notifications, Notifications::Hold);
        assert_eq!(config.start(), Some(1));
        assert_eq!(config.layout.gaps, 4);
        assert!(
            editor
                .mode_section(1)
                .starts_with("[[mode]]\nname = \"Play\"")
        );
        assert!(!editor.sections(&["layout", "mode"]).contains("[[mode]]"));

        editor.remove_mode(0);
        let (config, _) = Config::parse(&editor.text());
        assert_eq!(config.modes.len(), 1);
    }

    #[test]
    fn the_old_mode_tables_are_read_and_rewritten_as_a_list() {
        let old = "[mode.play]\nwallpaper = \"ember.jpg\" # mine\nopen = [\"steam\"]\n\n[mode.evening]\nnotifications = \"all\"\n";
        let (config, problems) = Config::parse(old);
        assert_eq!(problems, vec![]);
        assert_eq!(config.modes[0].name, "Play");
        assert!(config.modes[0].performance);
        assert_eq!(config.modes[1].icon, "moon");

        let mut editor = Editor::parse(old).unwrap();
        editor.set_mode(1, "keep_awake", true);
        let text = editor.text();
        assert!(text.contains("[[mode]]"), "{text}");
        assert!(text.contains("# mine"));
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        assert_eq!(config.modes.len(), 2);
        assert!(config.modes[1].keep_awake);
        assert_eq!(config.modes[0].open, ["steam"]);
    }

    #[test]
    fn layout_sections_are_created_when_missing() {
        let mut editor = Editor::parse("").unwrap();
        editor.set("layout", "borders", false);
        editor.set_speed("touchpad", 0.3);
        editor.set_list("keyboard", "layouts", &["us".into(), "ru".into()]);
        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert!(!config.layout.borders);
        assert_eq!(config.touchpad.map(|t| t.speed), Some(0.3));
        assert!(config.touchpad.is_some_and(|t| t.tap));
        assert_eq!(config.keyboard.layouts, ["us", "ru"]);
        assert!(editor.text().contains("speed = 0.3\n"));
        assert!(
            editor
                .sections(&["touchpad", "mouse"])
                .starts_with("[touchpad]\n")
        );
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

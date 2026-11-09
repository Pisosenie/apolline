use std::path::Path;
use std::{fs, io};

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use crate::{Accent, Calendar, KeyAction, ModeSettings, Notifications, Rule};

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

    // One calendar a line, since a CalDAV address alone fills most of one.
    pub fn set_calendars(&mut self, calendars: &[Calendar]) {
        if calendars.is_empty() {
            self.unset("bar", "calendars");
            return;
        }
        let mut list: Array = calendars
            .iter()
            .map(|calendar| match calendar {
                Calendar::Ics(link) => Value::from(link.as_str()),
                Calendar::CalDav { url, user } => {
                    let mut table = InlineTable::new();
                    table.insert("caldav", url.as_str().into());
                    table.insert("user", user.as_str().into());
                    Value::InlineTable(table)
                }
            })
            .collect();
        for value in list.iter_mut() {
            value.decor_mut().set_prefix("\n    ");
        }
        list.set_trailing("\n");
        list.set_trailing_comma(true);
        self.set("bar", "calendars", list);
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
        if mode.replay > 0 {
            table.insert("replay", i64::from(mode.replay).into());
        }
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

    // [keys]; none removes the section.
    pub fn set_keys(&mut self, keys: &[(String, KeyAction)]) {
        if keys.is_empty() {
            self.doc.remove("keys");
            return;
        }
        let section = table(self.doc.as_table_mut(), "keys");
        // Keys still wanted keep their place and comments; the rest go.
        let gone: Vec<String> = section
            .iter()
            .map(|(combo, _)| combo.to_owned())
            .filter(|combo| !keys.iter().any(|(wanted, _)| wanted == combo))
            .collect();
        for combo in gone {
            section.remove(&combo);
        }
        for (combo, action) in keys {
            let value = match action {
                KeyAction::Run(command) => Value::from(command.as_str()),
                KeyAction::Niri(niri) => {
                    let mut inline = InlineTable::new();
                    inline.insert("niri", niri.as_str().into());
                    Value::InlineTable(inline)
                }
            };
            put(section, combo, value);
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

    pub fn set_display(&mut self, name: &str, key: &str, value: impl Into<Value>) {
        let displays = list(self.doc.as_table_mut(), "display");
        let at = displays
            .iter()
            .position(|t| t.get("name").and_then(Item::as_str) == Some(name));
        let at = at.unwrap_or_else(|| {
            let mut table = Table::new();
            table.insert("name", name.into());
            displays.push(table);
            displays.len() - 1
        });
        if let Some(table) = displays.get_mut(at) {
            put(table, key, value.into());
        }
    }

    pub fn set_display_position(&mut self, name: &str, x: i64, y: i64) {
        let xy: Array = [x, y].into_iter().collect();
        self.set_display(name, "position", xy);
    }

    pub fn unset_display(&mut self, name: &str, key: &str) {
        let displays = list(self.doc.as_table_mut(), "display");
        let Some(at) = displays
            .iter()
            .position(|t| t.get("name").and_then(Item::as_str) == Some(name))
        else {
            return;
        };
        if let Some(table) = displays.get_mut(at) {
            table.remove(key);
            if table.len() <= 1 {
                displays.remove(at);
            }
        }
    }

    pub fn add_module(&mut self, command: &str) {
        let mut table = Table::new();
        table.insert("command", command.into());
        list(self.doc.as_table_mut(), "module").push(table);
    }

    pub fn remove_module(&mut self, index: usize) {
        remove_kept(list(self.doc.as_table_mut(), "module"), index, "command");
    }

    pub fn set_module(&mut self, index: usize, key: &str, value: impl Into<Value>) {
        let modules = list(self.doc.as_table_mut(), "module");
        if let Some(table) = kept(modules, index, "command").and_then(|at| modules.get_mut(at)) {
            put(table, key, value.into());
        }
    }

    pub fn unset_module(&mut self, index: usize, key: &str) {
        let modules = list(self.doc.as_table_mut(), "module");
        if let Some(table) = kept(modules, index, "command").and_then(|at| modules.get_mut(at)) {
            table.remove(key);
        }
    }

    pub fn remove_rule(&mut self, index: usize) {
        remove_kept(list(self.doc.as_table_mut(), "rule"), index, "app");
    }

    // A key in a table inside a table, such as `[style.launcher]`.
    pub fn set_in(&mut self, path: &[&str], key: &str, value: impl Into<Value>) {
        let mut at = self.doc.as_table_mut();
        for (depth, name) in path.iter().enumerate() {
            let item = at.entry(name).or_insert(Item::None);
            *item = match std::mem::take(item) {
                Item::Table(table) => Item::Table(table),
                Item::Value(Value::InlineTable(inline)) => Item::Table(inline.into_table()),
                _ => {
                    let mut table = Table::new();
                    table.set_implicit(depth + 1 < path.len());
                    Item::Table(table)
                }
            };
            at = item.as_table_mut().expect("just made a table");
        }
        put(at, key, value.into());
    }

    // Drops a key from a table inside a table; an emptied table goes with it.
    pub fn unset_in(&mut self, path: &[&str], key: &str) {
        let Some((last, parents)) = path.split_last() else {
            return;
        };
        let mut at = self.doc.as_table_mut();
        for name in parents {
            match at.get_mut(name).and_then(Item::as_table_mut) {
                Some(table) => at = table,
                None => return,
            }
        }
        if let Some(table) = at.get_mut(last).and_then(Item::as_table_mut) {
            table.remove(key);
            if table.is_empty() {
                at.remove(last);
            }
        }
    }

    pub fn unset(&mut self, section: &str, key: &str) {
        let Some(table) = self.doc.get_mut(section).and_then(Item::as_table_mut) else {
            return;
        };
        table.remove(key);
        // A section left with nothing in it goes too, unless you wrote something above it.
        let commented = table
            .decor()
            .prefix()
            .and_then(|prefix| prefix.as_str())
            .is_some_and(|prefix| prefix.contains('#'));
        if table.is_empty() && !commented {
            self.doc.remove(section);
        }
    }

    pub fn lists(&self, name: &str) -> String {
        let mut doc = DocumentMut::new();
        if let Some(found) = self.doc.get(name).filter(|item| item.is_array_of_tables()) {
            doc.insert(name, found.clone());
        }
        doc.to_string()
    }

    pub fn take_list(&mut self, name: &str, from: &Editor) {
        match from.doc.get(name).filter(|item| item.is_array_of_tables()) {
            Some(list) => {
                self.doc.insert(name, list.clone());
            }
            None => {
                self.doc.remove(name);
            }
        }
    }

    pub fn set_accent(&mut self, accent: Accent) {
        self.set("desktop", "accent", accent_text(accent));
    }

    // A dotted name picks one part of a table: "style.bar", or "bar.skip" for just that key.
    pub fn sections(&self, names: &[&str]) -> String {
        let mut doc = DocumentMut::new();
        'names: for name in names {
            let path: Vec<&str> = name.split('.').collect();
            let Some(found) = path
                .iter()
                .try_fold(self.doc.as_item(), |item, key| item.get(key))
                .filter(|found| !found.is_array_of_tables())
            else {
                continue;
            };
            let (last, parents) = path.split_last().expect("split gives at least one part");
            let mut table = doc.as_table_mut();
            for parent in parents {
                let item = table.entry(parent).or_insert_with(|| {
                    let mut implicit = Table::new();
                    implicit.set_implicit(true);
                    Item::Table(implicit)
                });
                let Some(inner) = item.as_table_mut() else {
                    continue 'names;
                };
                table = inner;
            }
            table.insert(last, found.clone());
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
        Accent::Palette => "palette".to_owned(),
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
    let item = parent
        .entry(key)
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    if !item.is_array_of_tables() {
        // An inline list keeps its tables; entries the reader skips anyway don't survive.
        let tables = match std::mem::take(item) {
            Item::Value(Value::Array(array)) => array
                .into_iter()
                .filter_map(|value| match value {
                    Value::InlineTable(table) => Some(table.into_table()),
                    _ => None,
                })
                .collect(),
            _ => ArrayOfTables::new(),
        };
        *item = Item::ArrayOfTables(tables);
    }
    item.as_array_of_tables_mut().expect("just made a list")
}

// The reader skips entries without their key, so its n-th entry is the n-th that has one.
fn remove_kept(tables: &mut ArrayOfTables, index: usize, key: &str) {
    if let Some(at) = kept(tables, index, key) {
        tables.remove(at);
    }
}

// Where the index-th entry the reader keeps (one with `key` written) sits among all of them.
fn kept(tables: &ArrayOfTables, index: usize, key: &str) -> Option<usize> {
    tables
        .iter()
        .enumerate()
        .filter(|(_, table)| {
            table
                .get(key)
                .and_then(Item::as_str)
                .is_some_and(|v| !v.trim().is_empty())
        })
        .nth(index)
        .map(|(at, _)| at)
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

    #[test]
    fn keys_of_your_own_are_written_and_read_back() {
        let mut editor =
            Editor::parse("[keys]\n# mine\n\"Mod+Return\" = \"foot\"\n\"Mod+X\" = \"x\"\n")
                .unwrap();
        let keys = vec![
            ("Mod+Return".to_owned(), KeyAction::Run("kitty".into())),
            ("Mod+C".to_owned(), KeyAction::niri("close-window").unwrap()),
        ];
        editor.set_keys(&keys);
        let text = editor.text();
        assert!(
            text.contains("# mine\n\"Mod+Return\" = \"kitty\""),
            "{text}"
        );
        assert!(
            text.contains("\"Mod+C\" = { niri = \"close-window\" }"),
            "{text}"
        );
        assert!(!text.contains("Mod+X"));
        let (config, problems) = crate::Config::parse(&text);
        assert!(problems.is_empty(), "{problems:?}");
        let mut sorted = keys.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(config.keys, sorted);
        editor.set_keys(&[]);
        assert!(!editor.text().contains("keys"));
    }
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
    fn nested_tables_are_written_as_sections() {
        let mut editor = Editor::parse("[layout]\ngaps = 4\n").unwrap();
        editor.set_in(&["style", "launcher"], "radius", 8);
        editor.set_in(&["style", "panel"], "size", 1.1);
        let text = editor.text();
        assert!(text.contains("[style.launcher]\nradius = 8"), "{text}");
        assert!(!text.contains("[style]\n"), "{text}");
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        assert_eq!(config.style.launcher.radius, Some(8));
        assert_eq!(config.style.panel.size, Some(1.1));

        let mut editor = Editor::parse("[style]\nlauncher = { size = 1.2 }\n").unwrap();
        editor.set_in(&["style", "launcher"], "grid", true);
        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(
            config.style.launcher.size,
            Some(1.2),
            "an inline table keeps what it had"
        );
        assert!(config.style.grid);
    }

    #[test]
    fn removal_counts_only_the_entries_that_were_read() {
        let text = "[[module]]\nevery = 5\n\n[[module]]\ncommand = \"date\"\n\n[[module]]\ncommand = \"uptime\"\n";
        let mut editor = Editor::parse(text).unwrap();
        editor.remove_module(1);
        let (config, _) = Config::parse(&editor.text());
        let left: Vec<&str> = config.modules.iter().map(|m| m.command.as_str()).collect();
        assert_eq!(left, ["date"]);

        let mut editor =
            Editor::parse("rule = [{ app = \"foot\" }, 5, { app = \"mpv\" }]\n").unwrap();
        editor.remove_rule(0);
        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(config.rules.len(), 1, "an inline list is kept, not wiped");
        assert_eq!(config.rules[0].app, "mpv");
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
    fn a_module_gets_a_name_and_a_rerun_in_place() {
        let mut editor =
            Editor::parse("[[module]]\nname = \"broken\"\n\n[[module]]\ncommand = \"date +%V\"\n")
                .unwrap();
        editor.set_module(0, "name", "week");
        editor.set_module(0, "every", 3600);
        let (config, _) = Config::parse(&editor.text());
        assert_eq!(config.modules[0].name.as_deref(), Some("week"));
        assert_eq!(config.modules[0].every, Some(3600));
        editor.unset_module(0, "every");
        let (config, _) = Config::parse(&editor.text());
        assert_eq!(config.modules[0].every, None);
        assert!(editor.text().starts_with("[[module]]\nname = \"broken\""));
    }

    #[test]
    fn sections_can_pick_a_part_of_a_table() {
        let editor = Editor::parse(
            "[bar]\nposition = \"left\"\nskip = [\"HDMI-A-1\"]\n\n[style.bar]\nsize = 0.9\n\n[style.launcher]\ngrid = true\n",
        )
        .unwrap();
        assert_eq!(
            editor
                .sections(&["style.bar", "bar.skip", "bar.screens"])
                .trim_start(),
            "[style.bar]\nsize = 0.9\n\n[bar]\nskip = [\"HDMI-A-1\"]\n"
        );
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
    fn calendars_of_both_kinds_are_written_and_read_back() {
        let calendars = [
            Calendar::Ics("~/calendars/home.ics".into()),
            Calendar::CalDav {
                url: "https://dav.example.org/calendars/me/work/".into(),
                user: "me".into(),
            },
        ];
        let mut editor = Editor::parse("[bar]\ncalendars = [\"old.ics\"] # mine\n").unwrap();
        editor.set_calendars(&calendars);
        let text = editor.text();
        assert!(
            text.contains("    { caldav = \"https://dav.example.org/calendars/me/work/\", user = \"me\" },\n] # mine"),
            "{text}"
        );
        let (config, problems) = Config::parse(&text);
        assert_eq!(problems, vec![]);
        assert_eq!(config.calendars, calendars);

        editor.set_calendars(&[]);
        assert!(!editor.text().contains("calendars"));
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

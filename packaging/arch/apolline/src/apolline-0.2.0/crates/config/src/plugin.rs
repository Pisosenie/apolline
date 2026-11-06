use std::collections::BTreeMap;

use toml::de::{DeTable, DeValue};

use crate::read::{Key, Reader, Value, suggest};
use crate::{Diagnostic, Module, slug};

const SECTIONS: [&str; 6] = ["plugin", "bar", "widget", "tile", "launcher", "setting"];
const ABOUT_OPTIONS: [&str; 4] = ["name", "version", "author", "about"];
const TILE_OPTIONS: [&str; 8] = [
    "name", "label", "icon", "state", "on", "off", "toggle", "every",
];
const LAUNCHER_OPTIONS: [&str; 2] = ["prefix", "command"];
// What the launcher answers itself: files after "/", emoji after ":", sums after "=".
const LAUNCHER_OWN: [char; 3] = ['/', ':', '='];
const SETTING_OPTIONS: [&str; 5] = ["key", "label", "kind", "choices", "default"];
const KINDS: [&str; 5] = ["text", "secret", "switch", "number", "choice"];

// A folder with a plugin.toml: pieces for the bar, the desktop, quick settings and the launcher,
// each a command in any language.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plugin {
    pub name: String,
    pub version: String,
    pub author: String,
    pub about: String,
    // [[bar]] and [[widget]]: the same as a [[module]], placed as "plugin.name".
    pub bar: Vec<Module>,
    pub widgets: Vec<Module>,
    pub tiles: Vec<Tile>,
    pub launchers: Vec<Source>,
    pub settings: Vec<Setting>,
}

// A quick settings tile: on or off as its state command says, switched by its own commands.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub name: String,
    pub label: String,
    pub icon: Option<String>,
    pub state: Option<String>,
    pub on: Option<String>,
    pub off: Option<String>,
    pub toggle: Option<String>,
    pub every: Option<u32>,
}

// Something the launcher asks while you type: its command gets the search and prints results.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Source {
    // Only searches that start with it, like "gh "; without one, every search.
    pub prefix: Option<String>,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub key: String,
    pub label: String,
    pub kind: Kind,
    pub default: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    // Kept in the keyring, never in the config file.
    Secret,
    Switch,
    Number,
    Choice(Vec<String>),
}

// [plugins]: which are on, and the settings of each from [plugins."name"], as text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plugins {
    pub enabled: Vec<String>,
    pub values: BTreeMap<String, BTreeMap<String, String>>,
}

impl Plugins {
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.iter().any(|n| n == name)
    }
}

impl Plugin {
    pub fn parse(src: &str) -> (Plugin, Vec<Diagnostic>) {
        let (doc, mut reader) = Reader::start(src);
        let mut plugin = Plugin::default();
        reader.manifest(doc.get_ref(), &mut plugin);
        reader.diagnostics.sort_by_key(|d| d.line);
        (plugin, reader.diagnostics)
    }

    // What the bar and the desktop can place, by the part after "plugin.".
    pub fn pieces(&self) -> Vec<&str> {
        self.bar
            .iter()
            .chain(&self.widgets)
            .filter_map(|m| m.name.as_deref())
            .collect()
    }

    pub fn tile_names(&self) -> Vec<&str> {
        self.tiles.iter().map(|t| t.name.as_str()).collect()
    }

    // Each setting but the secrets as APOLLINE_KEY, from the config or its default.
    pub fn environment(&self, values: Option<&BTreeMap<String, String>>) -> Vec<(String, String)> {
        self.settings
            .iter()
            .filter(|s| s.kind != Kind::Secret)
            .map(|s| {
                let value = values
                    .and_then(|v| v.get(&s.key))
                    .unwrap_or(&s.default)
                    .clone();
                (env_name(&s.key), value)
            })
            .collect()
    }

    pub fn secrets(&self) -> impl Iterator<Item = &Setting> {
        self.settings.iter().filter(|s| s.kind == Kind::Secret)
    }

    // Every command it would run, with what runs it, to show before it's installed.
    pub fn commands(&self) -> Vec<(String, &str)> {
        let mut all = Vec::new();
        for (what, modules) in [("bar", &self.bar), ("widget", &self.widgets)] {
            for module in modules {
                let name = module.name.as_deref().unwrap_or_default();
                all.push((format!("{what} {name}"), module.command.as_str()));
                let acts = [
                    &module.click,
                    &module.right_click,
                    &module.middle_click,
                    &module.scroll_up,
                    &module.scroll_down,
                ];
                for act in acts.into_iter().flatten() {
                    all.push((format!("{what} {name}"), act.as_str()));
                }
            }
        }
        for tile in &self.tiles {
            for command in [&tile.state, &tile.on, &tile.off, &tile.toggle]
                .into_iter()
                .flatten()
            {
                all.push((format!("tile {}", tile.name), command.as_str()));
            }
        }
        for source in &self.launchers {
            all.push(("launcher".to_owned(), source.command.as_str()));
        }
        all
    }
}

// A setting's variable: "token" is APOLLINE_TOKEN.
pub fn env_name(key: &str) -> String {
    let key: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("APOLLINE_{key}")
}

// A plugin's own name and its pieces' names: lowercase, so they read the same in any config.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Reader<'_> {
    fn manifest(&mut self, table: &DeTable, plugin: &mut Plugin) {
        let mut about = None;
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "plugin" => {
                    about = Some(self.line(key.span()));
                    if let Some(t) = self.section(key, value) {
                        self.about(t, plugin);
                    }
                }
                "bar" => plugin.bar = self.pieces(key, value, "bar"),
                "widget" => plugin.widgets = self.pieces(key, value, "widget"),
                "tile" => {
                    for (item, table) in self.items(key, value, "tile") {
                        plugin.tiles.extend(self.tile(item, table));
                    }
                }
                "launcher" => {
                    for (item, table) in self.items(key, value, "launcher") {
                        plugin.launchers.extend(self.source(item, table));
                    }
                }
                "setting" => {
                    for (item, table) in self.items(key, value, "setting") {
                        plugin.settings.extend(self.setting(item, table));
                    }
                }
                _ => self.unknown(key, "section", &SECTIONS),
            }
        }
        match about {
            None => self.say(1, "a plugin needs a [plugin] with its name", None),
            Some(line) if !valid_name(&plugin.name) => self.say(
                line,
                "the plugin needs a name of lowercase letters, digits, - and _",
                Some("like name = \"weather-eu\"".into()),
            ),
            Some(_) => {}
        }
        self.twice(plugin.pieces(), "a bar piece or widget");
        self.twice(plugin.tile_names(), "a tile");
        self.twice(
            plugin.settings.iter().map(|s| s.key.as_str()).collect(),
            "a setting",
        );
    }

    fn about(&mut self, table: &DeTable, plugin: &mut Plugin) {
        for (key, value) in table.iter() {
            let field = match key.get_ref().as_ref() {
                "name" => &mut plugin.name,
                "version" => &mut plugin.version,
                "author" => &mut plugin.author,
                "about" => &mut plugin.about,
                _ => {
                    self.unknown(key, "option", &ABOUT_OPTIONS);
                    continue;
                }
            };
            *field = self.text(key, value).unwrap_or_default();
        }
    }

    fn items<'a, 'i>(
        &mut self,
        key: &Key,
        value: &'a Value<'i>,
        what: &str,
    ) -> Vec<(&'a Value<'i>, &'a DeTable<'i>)> {
        let DeValue::Array(items) = value.get_ref() else {
            self.wrong::<()>(key, value, &format!("a list of [[{what}]] sections"));
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| match item.get_ref() {
                DeValue::Table(table) => Some((item, table)),
                _ => None,
            })
            .collect()
    }

    fn pieces(&mut self, key: &Key, value: &Value, what: &str) -> Vec<Module> {
        let mut pieces = Vec::new();
        for (item, table) in self.items(key, value, what) {
            let module = self.module(table);
            let line = self.line(item.span());
            if module.command.is_empty() {
                self.say(line, &format!("a [[{what}]] needs a command"), None);
            } else if !module.name.as_deref().is_some_and(valid_name) {
                self.say(
                    line,
                    &format!("a [[{what}]] needs a name of lowercase letters, digits, - and _"),
                    Some("it's placed as \"plugin.name\"".into()),
                );
            } else {
                pieces.push(module);
            }
        }
        pieces
    }

    fn tile(&mut self, item: &Value, table: &DeTable) -> Option<Tile> {
        let mut tile = Tile {
            name: String::new(),
            label: String::new(),
            icon: None,
            state: None,
            on: None,
            off: None,
            toggle: None,
            every: None,
        };
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "name" => tile.name = self.text(key, value).unwrap_or_default(),
                "label" => tile.label = self.text(key, value).unwrap_or_default(),
                "icon" => tile.icon = self.text(key, value),
                "state" => tile.state = self.text(key, value),
                "on" => tile.on = self.text(key, value),
                "off" => tile.off = self.text(key, value),
                "toggle" => tile.toggle = self.text(key, value),
                "every" => tile.every = self.number(key, value, 1..=65535).map(u32::from),
                _ => self.unknown(key, "option", &TILE_OPTIONS),
            }
        }
        let line = self.line(item.span());
        if tile.name.is_empty() {
            tile.name = slug(&tile.label);
        }
        let switches = tile.toggle.is_some() || (tile.on.is_some() && tile.off.is_some());
        if tile.label.is_empty() {
            self.say(line, "a tile needs a label", None);
        } else if !switches {
            self.say(line, "a tile needs a toggle command, or on and off", None);
        } else if !valid_name(&tile.name) {
            self.say(
                line,
                "a tile's name is lowercase letters, digits, - and _",
                Some("it's placed as \"plugin.name\" in [panel] tiles".into()),
            );
        } else {
            return Some(tile);
        }
        None
    }

    fn source(&mut self, item: &Value, table: &DeTable) -> Option<Source> {
        let mut source = Source {
            prefix: None,
            command: String::new(),
        };
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                // Kept as written, a space at its end too, so "gh " doesn't catch "ghost".
                "prefix" => {
                    source.prefix = self
                        .string(key, value)
                        .filter(|p| !p.trim().is_empty())
                        .map(|p| p.trim_start().to_owned())
                }
                "command" => source.command = self.text(key, value).unwrap_or_default(),
                _ => self.unknown(key, "option", &LAUNCHER_OPTIONS),
            }
        }
        if source.command.is_empty() {
            let line = self.line(item.span());
            self.say(line, "a [[launcher]] needs a command", None);
            return None;
        }
        // A prefix keeps every search that starts with it to its source, so one too short or the
        // launcher's own would take over the launcher.
        if let Some(prefix) = &source.prefix
            && (prefix.chars().count() < 2 || prefix.starts_with(LAUNCHER_OWN))
        {
            let line = self.line(item.span());
            self.say(
                line,
                "a [[launcher]] prefix is two characters or more, and not the launcher's own",
                Some("“/”, “:” and “=” start the launcher's own searches".into()),
            );
            return None;
        }
        Some(source)
    }

    fn setting(&mut self, item: &Value, table: &DeTable) -> Option<Setting> {
        let (mut key_name, mut label, mut kind, mut choices) =
            (String::new(), String::new(), "text".to_owned(), None);
        let mut default = None;
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "key" => key_name = self.text(key, value).unwrap_or_default(),
                "label" => label = self.text(key, value).unwrap_or_default(),
                "kind" => {
                    let kinds = KINDS.map(|k| (k, k));
                    if let Some(k) = self.choice(key, value, &kinds) {
                        kind = k.to_owned()
                    }
                }
                "choices" => choices = self.strings(key, value),
                "default" => default = Some((key, value)),
                _ => self.unknown(key, "option", &SETTING_OPTIONS),
            }
        }
        let line = self.line(item.span());
        if !valid_key(&key_name) {
            self.say(
                line,
                "a setting needs a key of letters, digits and _",
                Some("it reaches commands as APOLLINE_KEY".into()),
            );
            return None;
        }
        let kind = match (kind.as_str(), choices) {
            ("secret", _) => Kind::Secret,
            ("switch", _) => Kind::Switch,
            ("number", _) => Kind::Number,
            ("choice", Some(choices)) if !choices.is_empty() => Kind::Choice(choices),
            ("choice", _) => {
                self.say(line, "a choice needs its choices", None);
                return None;
            }
            _ => Kind::Text,
        };
        let mut setting = Setting {
            label: if label.is_empty() {
                key_name.clone()
            } else {
                label
            },
            key: key_name,
            kind,
            default: String::new(),
        };
        if let Some((key, value)) = default {
            setting.default = self.value(key, value, Some(&setting)).unwrap_or_default();
        }
        if setting.kind == Kind::Switch && setting.default.is_empty() {
            setting.default = "false".into();
        }
        Some(setting)
    }

    // A setting's value as text, checked against its kind when it's known.
    pub(crate) fn value(
        &mut self,
        key: &Key,
        value: &Value,
        setting: Option<&Setting>,
    ) -> Option<String> {
        let text = match value.get_ref() {
            DeValue::String(s) => s.to_string(),
            DeValue::Boolean(b) => b.to_string(),
            DeValue::Integer(n) => i64::from_str_radix(n.as_str(), n.radix()).ok()?.to_string(),
            DeValue::Float(f) => f.as_str().to_owned(),
            _ => return self.wrong(key, value, "text, a number, or true or false"),
        };
        let fits = match setting.map(|s| &s.kind) {
            Some(Kind::Switch) => matches!(value.get_ref(), DeValue::Boolean(_)),
            Some(Kind::Number) => {
                matches!(value.get_ref(), DeValue::Integer(_) | DeValue::Float(_))
            }
            Some(Kind::Text | Kind::Secret | Kind::Choice(_)) => {
                matches!(value.get_ref(), DeValue::String(_))
            }
            None => true,
        };
        if !fits {
            let expected = match setting.map(|s| &s.kind) {
                Some(Kind::Switch) => "true or false",
                Some(Kind::Number) => "a number",
                _ => "text in quotes",
            };
            return self.wrong(key, value, expected);
        }
        if let Some(Kind::Choice(choices)) = setting.map(|s| &s.kind)
            && !choices.contains(&text)
        {
            let known: Vec<&str> = choices.iter().map(String::as_str).collect();
            self.unknown_name(value, &text, "choice", &known);
            return None;
        }
        Some(text)
    }

    // [plugins]: enabled = [...] and a table of settings for each plugin.
    pub(crate) fn plugins(&mut self, table: &DeTable, plugins: &mut crate::Plugins) {
        for (key, value) in table.iter() {
            match (key.get_ref().as_ref(), value.get_ref()) {
                ("enabled", _) => {
                    let Some(names) = self.strings(key, value) else {
                        continue;
                    };
                    plugins.enabled.clear();
                    for name in names {
                        let line = self.line(value.span());
                        if self.installed(&name, line) && !plugins.enabled.contains(&name) {
                            plugins.enabled.push(name);
                        }
                    }
                }
                (name, DeValue::Table(values)) => {
                    let line = self.line(key.span());
                    self.installed(name, line);
                    let found = self
                        .plugins
                        .and_then(|all| all.iter().find(|p| p.name == name));
                    let values = self.plugin_values(values, found);
                    plugins.values.insert(name.to_owned(), values);
                }
                _ => {
                    let hint =
                        "[plugins] has enabled = [...], and [plugins.NAME] for a plugin's settings";
                    let line = self.line(key.span());
                    let name = key.get_ref();
                    self.say(line, &format!("unknown option “{name}”"), Some(hint.into()));
                }
            }
        }
    }

    fn plugin_values(
        &mut self,
        table: &DeTable,
        plugin: Option<&Plugin>,
    ) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for (key, value) in table.iter() {
            let name = key.get_ref().as_ref();
            let setting = plugin.and_then(|p| p.settings.iter().find(|s| s.key == name));
            match (plugin, setting) {
                (Some(plugin), None) => {
                    let known: Vec<&str> = plugin.settings.iter().map(|s| s.key.as_str()).collect();
                    self.unknown(key, "setting", &known);
                }
                (_, Some(setting)) if setting.kind == Kind::Secret => {
                    let line = self.line(key.span());
                    self.say(
                        line,
                        &format!("“{name}” is a secret and isn't read from this file"),
                        Some("set it in Settings → Plugins, which keeps it in the keyring".into()),
                    );
                }
                _ => {
                    if let Some(text) = self.value(key, value, setting) {
                        values.insert(name.to_owned(), text);
                    }
                }
            }
        }
        values
    }

    // Whether a plugin of that name is installed, said if not; trusted when that isn't known.
    fn installed(&mut self, name: &str, line: usize) -> bool {
        let Some(plugins) = self.plugins else {
            return true;
        };
        if plugins.iter().any(|p| p.name == name) {
            return true;
        }
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        let hint = match suggest(name, &names) {
            Some(close) => format!("did you mean “{close}”?"),
            None => "`apolline plugin list` shows what's installed".to_owned(),
        };
        self.say(
            line,
            &format!("no plugin called “{name}” is installed"),
            Some(hint),
        );
        false
    }

    // "plugin.part" in the bar, the desktop or quick settings: a problem if it names nothing.
    pub(crate) fn plugin_part(
        &mut self,
        placed: &str,
        line: usize,
        enabled: &[String],
        parts: fn(&Plugin) -> Vec<&str>,
        what: &str,
    ) -> Option<Diagnostic> {
        let all = self.plugins?;
        let (name, part) = placed.split_once('.').unwrap_or((placed, ""));
        let Some(plugin) = all.iter().find(|p| p.name == name) else {
            let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
            let hint = match suggest(name, &names) {
                Some(close) => format!("did you mean “{close}.{part}”?"),
                None => "`apolline plugin list` shows what's installed".to_owned(),
            };
            let message = format!("“{placed}” is from “{name}”, which isn't installed");
            return Some(Diagnostic::new(line, message).hint(hint));
        };
        let known = parts(plugin);
        if !known.contains(&part) {
            let hint = match suggest(part, &known) {
                Some(close) => format!("did you mean “{name}.{close}”?"),
                None if known.is_empty() => format!("“{name}” has none"),
                None => format!("“{name}” has {}", known.join(", ")),
            };
            let message = format!("“{name}” has no {what} called “{part}”");
            return Some(Diagnostic::new(line, message).hint(hint));
        }
        if !enabled.iter().any(|e| e == name) {
            let message = format!("“{placed}” is from “{name}”, which isn't enabled");
            let hint = format!("add “{name}” to [plugins] enabled");
            return Some(Diagnostic::new(line, message).hint(hint));
        }
        None
    }

    // Plugin tiles in [panel] tiles, once [plugins] is read wherever it is in the file.
    pub(crate) fn settle_plugins(&mut self, config: &crate::Config) {
        for (name, line) in std::mem::take(&mut self.plugin_tiles) {
            let enabled = &config.plugins.enabled;
            if let Some(problem) =
                self.plugin_part(&name, line, enabled, Plugin::tile_names, "tile")
                && !self.diagnostics.contains(&problem)
            {
                self.diagnostics.push(problem);
            }
        }
    }

    fn twice(&mut self, names: Vec<&str>, what: &str) {
        for (i, name) in names.iter().enumerate() {
            if names[..i].contains(name) {
                self.say(0, &format!("“{name}” is {what} twice"), None);
            }
        }
    }

    fn say(&mut self, line: usize, message: &str, hint: Option<String>) {
        let diagnostic = Diagnostic::new(line, message);
        self.diagnostics.push(match hint {
            Some(hint) => diagnostic.hint(hint),
            None => diagnostic,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    const IP: &str = r#"
[plugin]
name = "ip"
version = "1.0"
author = "Ann"
about = "Your public address"

[[bar]]
name = "address"
command = "./ip.sh"
every = 600
click = "./ip.sh --copy"

[[widget]]
name = "where"
command = "./where.sh"

[[tile]]
label = "Dummy"
icon = "globe"
state = "./state.sh"
toggle = "./toggle.sh"
every = 5

[[launcher]]
prefix = "man "
command = "./man.sh"

[[setting]]
key = "service"
label = "Service"
kind = "choice"
choices = ["ifconfig.me", "icanhazip.com"]
default = "ifconfig.me"

[[setting]]
key = "token"
kind = "secret"

[[setting]]
key = "ipv6"
kind = "switch"
"#;

    #[test]
    fn a_plugin_reads_with_every_part() {
        let (plugin, problems) = Plugin::parse(IP);
        assert_eq!(problems, []);
        assert_eq!(
            (plugin.name.as_str(), plugin.version.as_str()),
            ("ip", "1.0")
        );
        assert_eq!(plugin.pieces(), ["address", "where"]);
        assert_eq!(plugin.bar[0].every, Some(600));
        assert_eq!(plugin.bar[0].click.as_deref(), Some("./ip.sh --copy"));
        assert_eq!(plugin.tiles[0].name, "dummy");
        assert_eq!(plugin.tiles[0].every, Some(5));
        assert_eq!(plugin.launchers[0].prefix.as_deref(), Some("man "));
        assert_eq!(
            plugin.settings[0].kind,
            Kind::Choice(vec!["ifconfig.me".into(), "icanhazip.com".into()])
        );
        assert_eq!(plugin.settings[1].label, "token");
        assert_eq!(plugin.settings[2].default, "false");
        assert_eq!(plugin.commands().len(), 6);
    }

    #[test]
    fn a_broken_plugin_says_what_is_missing() {
        let (plugin, problems) = Plugin::parse(
            "[plugin]\nname = \"My Plugin\"\n\n[[bar]]\ncommand = \"x\"\n\n[[tile]]\nlabel = \"T\"\n\n[[launcher]]\nprefx = \"a\"\n\n[[setting]]\nkey = \"a b\"\n\n[[setting]]\nkey = \"k\"\nkind = \"choise\"\n",
        );
        let said: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert!(said[0].starts_with("the plugin needs a name"));
        assert!(said.contains(&"a [[bar]] needs a name of lowercase letters, digits, - and _"));
        assert!(said.contains(&"a tile needs a toggle command, or on and off"));
        assert!(said.contains(&"unknown option “prefx”"));
        assert!(said.contains(&"a [[launcher]] needs a command"));
        for prefix in ["g", "/f", ":x", "= "] {
            let src = format!(
                "[plugin]\nname = \"p\"\n\n[[launcher]]\nprefix = \"{prefix}\"\ncommand = \"x\"\n"
            );
            let (plugin, problems) = Plugin::parse(&src);
            assert!(plugin.launchers.is_empty(), "{prefix}");
            assert!(problems[0].message.starts_with("a [[launcher]] prefix"));
        }
        assert!(said.contains(&"a setting needs a key of letters, digits and _"));
        assert!(problems.iter().any(|p| p.to_string()
            == "“choise” is not a valid value for “kind” — did you mean “choice”?"));
        assert!(plugin.bar.is_empty() && plugin.tiles.is_empty());
        let (_, problems) = Plugin::parse("[[bar]]\nname = \"a\"\ncommand = \"x\"\n");
        assert_eq!(
            problems[0].message,
            "a plugin needs a [plugin] with its name"
        );
    }

    #[test]
    fn settings_reach_commands_as_variables() {
        let (plugin, _) = Plugin::parse(IP);
        let values = BTreeMap::from([("service".to_owned(), "icanhazip.com".to_owned())]);
        assert_eq!(
            plugin.environment(Some(&values)),
            [
                ("APOLLINE_SERVICE".to_owned(), "icanhazip.com".to_owned()),
                ("APOLLINE_IPV6".to_owned(), "false".to_owned()),
            ]
        );
        assert_eq!(
            plugin.environment(None)[0].1,
            "ifconfig.me",
            "the default stands in"
        );
        assert_eq!(
            plugin.secrets().map(|s| s.key.as_str()).collect::<Vec<_>>(),
            ["token"]
        );
        assert_eq!(env_name("api-key"), "APOLLINE_API_KEY");
    }

    #[test]
    fn plugins_in_the_config_are_checked_against_what_is_installed() {
        let (plugin, _) = Plugin::parse(IP);
        let src = "[bar]\nright = [\"ip.adress\", \"ip.address\", \"tray\"]\n\n[panel]\ntiles = [\"wifi\", \"ip.dummy\"]\n\n[desktop]\nwidgets = [\"ip.where\", \"weather\"]\n\n[plugins]\nenabled = [\"ip\", \"weathr\"]\n\n[plugins.ip]\nservice = \"nope\"\nipv6 = \"yes\"\ntoken = \"hunter2\"\nservise = \"x\"\n";
        let (config, problems) = Config::parse_with(src, std::slice::from_ref(&plugin));
        let said: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            said,
            [
                "“ip” has no bar piece called “adress” — did you mean “ip.address”?",
                "no plugin called “weathr” is installed — `apolline plugin list` shows what's installed",
                "unknown choice “nope” — known: ifconfig.me, icanhazip.com",
                "“ipv6” should be true or false",
                "“token” is a secret and isn't read from this file — set it in Settings → Plugins, which keeps it in the keyring",
                "unknown setting “servise” — did you mean “service”?",
            ]
        );
        assert_eq!(config.plugins.enabled, ["ip"]);
        assert_eq!(config.panel.tiles, ["wifi", "ip.dummy"]);
        assert_eq!(config.desktop.widgets, ["ip.where", "weather"]);

        // Without knowing what's installed, plugin names are taken as written.
        let (config, problems) = Config::parse(src);
        assert_eq!(problems.len(), 0, "{problems:?}");
        assert_eq!(config.plugins.values["ip"]["ipv6"], "yes");
        assert_eq!(config.bar.right.len(), 3);
    }

    #[test]
    fn a_piece_of_a_plugin_left_off_says_so() {
        let (plugin, _) = Plugin::parse(IP);
        let src = "[bar]\nleft = [\"ip.address\"]\n";
        let (_, problems) = Config::parse_with(src, std::slice::from_ref(&plugin));
        assert_eq!(
            problems[0].to_string(),
            "“ip.address” is from “ip”, which isn't enabled — add “ip” to [plugins] enabled"
        );
    }
}

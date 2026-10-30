use std::ops::{Range, RangeInclusive};

use apolline_theme::{Mode, Rgba};
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::{
    Accent, Alerts, Bar, Config, Diagnostic, Keyboard, Layout, ModeSettings, Mouse, Notifications,
    Shortcut, Shortcuts, Switch, Touchpad, Workspaces,
};

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

const MODES: [(&str, Mode); 3] = [
    ("work", Mode::Work),
    ("play", Mode::Play),
    ("evening", Mode::Evening),
];

const WORKSPACES: [(&str, Workspaces); 2] =
    [("dots", Workspaces::Dots), ("numbers", Workspaces::Numbers)];

const SECTIONS: [&str; 9] = [
    "general",
    "layout",
    "bar",
    "keyboard",
    "touchpad",
    "mouse",
    "notifications",
    "shortcuts",
    "mode",
];

const NOTIFICATIONS: [(&str, Notifications); 3] = [
    ("all", Notifications::All),
    ("mentions", Notifications::Mentions),
    ("hold", Notifications::Hold),
];

pub(crate) fn document(src: &str) -> (Config, Vec<Diagnostic>) {
    let (doc, errors) = DeTable::parse_recoverable(src);
    let mut reader = Reader {
        src,
        diagnostics: Vec::new(),
    };
    for error in &errors {
        let line = error.span().map_or(0, |span| reader.line(span));
        let message = error.message().lines().next().unwrap_or("syntax error");
        reader.diagnostics.push(Diagnostic::new(line, message));
    }

    let mut config = Config::default();
    reader.root(doc.get_ref(), &mut config);
    reader.diagnostics.sort_by_key(|d| d.line);
    (config, reader.diagnostics)
}

struct Reader<'s> {
    src: &'s str,
    diagnostics: Vec<Diagnostic>,
}

impl Reader<'_> {
    fn root(&mut self, table: &DeTable, config: &mut Config) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "general" => {
                    if let Some(t) = self.section(key, value) {
                        self.general(t, config)
                    }
                }
                "layout" => {
                    if let Some(t) = self.section(key, value) {
                        self.layout(t, &mut config.layout)
                    }
                }
                "bar" => {
                    if let Some(t) = self.section(key, value) {
                        self.bar(t, &mut config.bar)
                    }
                }
                "keyboard" => {
                    if let Some(t) = self.section(key, value) {
                        self.keyboard(t, &mut config.keyboard)
                    }
                }
                "touchpad" => {
                    if let Some(t) = self.section(key, value) {
                        self.touchpad(t, config.touchpad.get_or_insert_default())
                    }
                }
                "mouse" => {
                    if let Some(t) = self.section(key, value) {
                        self.mouse(t, config.mouse.get_or_insert_default())
                    }
                }
                "notifications" => {
                    if let Some(t) = self.section(key, value) {
                        self.alerts(t, &mut config.alerts)
                    }
                }
                "shortcuts" => {
                    if let Some(t) = self.section(key, value) {
                        self.shortcuts(t, &mut config.shortcuts)
                    }
                }
                "mode" => {
                    if let Some(t) = self.section(key, value) {
                        self.modes(t, config)
                    }
                }
                _ => self.unknown(key, "section", &SECTIONS),
            }
        }
    }

    fn general(&mut self, table: &DeTable, config: &mut Config) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "mode" => {
                    if let Some(mode) = self.choice(key, value, &MODES) {
                        config.mode = mode
                    }
                }
                _ => self.unknown(key, "option", &["mode"]),
            }
        }
    }

    fn layout(&mut self, table: &DeTable, layout: &mut Layout) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "gaps" => {
                    if let Some(v) = self.number(key, value, 0..=64) {
                        layout.gaps = v
                    }
                }
                "radius" => {
                    if let Some(v) = self.number(key, value, 0..=32) {
                        layout.radius = v
                    }
                }
                "borders" => {
                    if let Some(v) = self.boolean(key, value) {
                        layout.borders = v
                    }
                }
                "shadows" => {
                    if let Some(v) = self.boolean(key, value) {
                        layout.shadows = v
                    }
                }
                _ => self.unknown(key, "option", &["gaps", "radius", "borders", "shadows"]),
            }
        }
    }

    fn bar(&mut self, table: &DeTable, bar: &mut Bar) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "workspaces" => {
                    if let Some(v) = self.choice(key, value, &WORKSPACES) {
                        bar.workspaces = v
                    }
                }
                "seconds" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.seconds = v
                    }
                }
                "date" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.date = v
                    }
                }
                "battery_percent" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.battery_percent = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["workspaces", "seconds", "date", "battery_percent"],
                ),
            }
        }
    }

    fn keyboard(&mut self, table: &DeTable, keyboard: &mut Keyboard) {
        let switches = Switch::ALL.map(|s| (s.key(), s));
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "layouts" => {
                    if let Some(v) = self.layouts(key, value) {
                        keyboard.layouts = v
                    }
                }
                "switch" => {
                    if let Some(v) = self.choice(key, value, &switches) {
                        keyboard.switch = Some(v)
                    }
                }
                "repeat_delay" => {
                    if let Some(v) = self.number(key, value, 150..=2000) {
                        keyboard.repeat_delay = Some(v)
                    }
                }
                "repeat_rate" => {
                    if let Some(v) = self.number(key, value, 5..=80) {
                        keyboard.repeat_rate = Some(v)
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["layouts", "switch", "repeat_delay", "repeat_rate"],
                ),
            }
        }
    }

    fn touchpad(&mut self, table: &DeTable, touchpad: &mut Touchpad) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "tap" => {
                    if let Some(v) = self.boolean(key, value) {
                        touchpad.tap = v
                    }
                }
                "natural_scroll" => {
                    if let Some(v) = self.boolean(key, value) {
                        touchpad.natural_scroll = v
                    }
                }
                "disable_while_typing" => {
                    if let Some(v) = self.boolean(key, value) {
                        touchpad.disable_while_typing = v
                    }
                }
                "speed" => {
                    if let Some(v) = self.speed(key, value) {
                        touchpad.speed = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["tap", "natural_scroll", "disable_while_typing", "speed"],
                ),
            }
        }
    }

    fn mouse(&mut self, table: &DeTable, mouse: &mut Mouse) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "natural_scroll" => {
                    if let Some(v) = self.boolean(key, value) {
                        mouse.natural_scroll = v
                    }
                }
                "acceleration" => {
                    if let Some(v) = self.boolean(key, value) {
                        mouse.acceleration = v
                    }
                }
                "speed" => {
                    if let Some(v) = self.speed(key, value) {
                        mouse.speed = v
                    }
                }
                _ => self.unknown(key, "option", &["natural_scroll", "acceleration", "speed"]),
            }
        }
    }

    fn shortcuts(&mut self, table: &DeTable, shortcuts: &mut Shortcuts) {
        let known = Shortcut::ALL.map(Shortcut::key);
        for (key, value) in table.iter() {
            let Some(shortcut) = Shortcut::ALL
                .into_iter()
                .find(|s| s.key() == key.get_ref().as_ref())
            else {
                self.unknown(key, "shortcut", &known);
                continue;
            };
            let Some(combo) = self.string(key, value) else {
                continue;
            };
            let valid = !combo.is_empty()
                && !combo.starts_with('+')
                && !combo.ends_with('+')
                && combo
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '_');
            if valid {
                *shortcuts.get_mut(shortcut) = combo.to_owned();
            } else {
                self.wrong::<()>(key, value, r#"a key combination like "Mod+Space""#);
            }
        }
    }

    fn alerts(&mut self, table: &DeTable, alerts: &mut Alerts) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "do_not_disturb" => {
                    if let Some(v) = self.boolean(key, value) {
                        alerts.do_not_disturb = v
                    }
                }
                "seconds" => {
                    if let Some(v) = self.number(key, value, 2..=30) {
                        alerts.seconds = v
                    }
                }
                _ => self.unknown(key, "option", &["do_not_disturb", "seconds"]),
            }
        }
    }

    fn modes(&mut self, table: &DeTable, config: &mut Config) {
        for (key, value) in table.iter() {
            match Mode::from_key(key.get_ref()) {
                Some(mode) => {
                    if let Some(t) = self.section(key, value) {
                        self.mode(t, config.mode_mut(mode))
                    }
                }
                None => self.unknown(key, "mode", &["work", "play", "evening"]),
            }
        }
    }

    fn mode(&mut self, table: &DeTable, settings: &mut ModeSettings) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "wallpaper" => {
                    if let Some(v) = self.string(key, value) {
                        settings.wallpaper = v.to_owned()
                    }
                }
                "accent" => {
                    if let Some(v) = self.accent(key, value) {
                        settings.accent = v
                    }
                }
                "notifications" => {
                    if let Some(v) = self.choice(key, value, &NOTIFICATIONS) {
                        settings.notifications = v
                    }
                }
                "open" => {
                    if let Some(v) = self.strings(key, value) {
                        settings.open = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["wallpaper", "accent", "notifications", "open"],
                ),
            }
        }
    }

    fn section<'a, 'i>(&mut self, key: &Key, value: &'a Value<'i>) -> Option<&'a DeTable<'i>> {
        match value.get_ref() {
            DeValue::Table(table) => Some(table),
            _ => self.wrong(key, value, "a section like [name]"),
        }
    }

    fn string<'a>(&mut self, key: &Key, value: &'a Value) -> Option<&'a str> {
        value
            .get_ref()
            .as_str()
            .or_else(|| self.wrong(key, value, "text in quotes"))
    }

    fn boolean(&mut self, key: &Key, value: &Value) -> Option<bool> {
        value
            .get_ref()
            .as_bool()
            .or_else(|| self.wrong(key, value, "true or false"))
    }

    fn number(&mut self, key: &Key, value: &Value, range: RangeInclusive<u16>) -> Option<u16> {
        let (low, high) = (*range.start(), *range.end());
        let parsed = value
            .get_ref()
            .as_integer()
            .and_then(|n| i64::from_str_radix(n.as_str(), n.radix()).ok());
        match parsed {
            Some(n) if (i64::from(low)..=i64::from(high)).contains(&n) => Some(n as u16),
            _ => self.wrong(key, value, &format!("a whole number from {low} to {high}")),
        }
    }

    fn speed(&mut self, key: &Key, value: &Value) -> Option<f32> {
        let parsed = match value.get_ref() {
            DeValue::Float(f) => f.as_str().parse::<f32>().ok(),
            DeValue::Integer(n) => n.as_str().parse::<f32>().ok(),
            _ => None,
        };
        match parsed {
            Some(v) if (-1.0..=1.0).contains(&v) => Some(v),
            _ => self.wrong(key, value, "a number from -1.0 (slow) to 1.0 (fast)"),
        }
    }

    fn layouts(&mut self, key: &Key, value: &Value) -> Option<Vec<String>> {
        let list = self.strings(key, value)?;
        let valid = |code: &String| {
            !code.is_empty()
                && code
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        match list.iter().find(|code| !valid(code)) {
            None => Some(list),
            Some(bad) => {
                let message = format!("“{bad}” is not a keyboard layout code");
                self.diagnostics.push(
                    Diagnostic::new(self.line(value.span()), message)
                        .hint("use short codes like \"us\", \"ru\" or \"de\""),
                );
                None
            }
        }
    }

    fn strings(&mut self, key: &Key, value: &Value) -> Option<Vec<String>> {
        let list = value.get_ref().as_array().and_then(|items| {
            items
                .iter()
                .map(|item| item.get_ref().as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        });
        list.or_else(|| self.wrong(key, value, r#"a list of names like ["steam", "discord"]"#))
    }

    fn accent(&mut self, key: &Key, value: &Value) -> Option<Accent> {
        match value.get_ref().as_str() {
            Some("auto") => Some(Accent::Auto),
            Some(text) if Rgba::parse(text).is_some() => Rgba::parse(text).map(Accent::Fixed),
            _ => self.wrong(key, value, r##""auto" or a color like "#ff6a3d""##),
        }
    }

    fn choice<T: Copy>(&mut self, key: &Key, value: &Value, options: &[(&str, T)]) -> Option<T> {
        let names: Vec<&str> = options.iter().map(|(name, _)| *name).collect();
        let Some(text) = value.get_ref().as_str() else {
            return self.wrong(key, value, &format!("one of {}", quoted(&names)));
        };
        if let Some((_, choice)) = options.iter().find(|(name, _)| *name == text) {
            return Some(*choice);
        }
        let message = format!("“{text}” is not a valid value for “{}”", key.get_ref());
        let hint = match suggest(text, &names) {
            Some(close) => format!("did you mean “{close}”?"),
            None => format!("use one of {}", quoted(&names)),
        };
        self.diagnostics
            .push(Diagnostic::new(self.line(value.span()), message).hint(hint));
        None
    }

    fn wrong<T>(&mut self, key: &Key, value: &Value, expected: &str) -> Option<T> {
        let message = format!("“{}” should be {expected}", key.get_ref());
        self.diagnostics
            .push(Diagnostic::new(self.line(value.span()), message));
        None
    }

    fn unknown(&mut self, key: &Key, what: &str, known: &[&str]) {
        let name = key.get_ref();
        let hint = match suggest(name, known) {
            Some(close) => format!("did you mean “{close}”?"),
            None => format!("known: {}", known.join(", ")),
        };
        let diagnostic =
            Diagnostic::new(self.line(key.span()), format!("unknown {what} “{name}”")).hint(hint);
        self.diagnostics.push(diagnostic);
    }

    fn line(&self, span: Range<usize>) -> usize {
        let end = span.start.min(self.src.len());
        self.src.as_bytes()[..end]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1
    }
}

fn suggest<'a>(input: &str, known: &[&'a str]) -> Option<&'a str> {
    known
        .iter()
        .map(|candidate| (*candidate, strsim::jaro_winkler(input, candidate)))
        .filter(|(_, score)| *score >= 0.85)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(candidate, _)| candidate)
}

fn quoted(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("“{name}”"))
        .collect::<Vec<_>>()
        .join(", ")
}

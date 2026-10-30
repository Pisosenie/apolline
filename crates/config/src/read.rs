use std::ops::{Range, RangeInclusive};

use apolline_theme::{Mode, Rgba};
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::{Accent, Config, Diagnostic, Layout, ModeSettings, Notifications};

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

const MODES: [(&str, Mode); 3] = [
    ("work", Mode::Work),
    ("play", Mode::Play),
    ("evening", Mode::Evening),
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
                "mode" => {
                    if let Some(t) = self.section(key, value) {
                        self.modes(t, config)
                    }
                }
                _ => self.unknown(key, "section", &["general", "layout", "mode"]),
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
                _ => self.unknown(key, "option", &["gaps", "radius", "borders"]),
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

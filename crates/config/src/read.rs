use std::ops::{Range, RangeInclusive};

use apolline_theme::Rgba;
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::{
    Accent, Alerts, Bar, Center, Config, Desktop, Diagnostic, Edge, Idle, Keyboard, Layout, Lock,
    ModeSettings, Mouse, Notifications, Rule, Shortcut, Shortcuts, Switch, Touchpad, Width,
    Windows, Workspaces, slug,
};

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

const WORKSPACES: [(&str, Workspaces); 2] =
    [("dots", Workspaces::Dots), ("numbers", Workspaces::Numbers)];

const SECTIONS: [&str; 15] = [
    "general",
    "theme",
    "desktop",
    "layout",
    "bar",
    "keyboard",
    "touchpad",
    "mouse",
    "notifications",
    "shortcuts",
    "lock",
    "idle",
    "windows",
    "rule",
    "mode",
];

const MODE_OPTIONS: [&str; 9] = [
    "name",
    "icon",
    "wallpaper",
    "accent",
    "notifications",
    "open",
    "performance",
    "keep_awake",
    "keys",
];

const WINDOW_OPTIONS: [&str; 10] = [
    "width",
    "presets",
    "center",
    "center_lone",
    "focus_follows_mouse",
    "warp_mouse",
    "opacity",
    "inactive_opacity",
    "animations",
    "switcher",
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
        start: None,
    };
    for error in &errors {
        let line = error.span().map_or(0, |span| reader.line(span));
        let message = error.message().lines().next().unwrap_or("syntax error");
        reader.diagnostics.push(Diagnostic::new(line, message));
    }

    let mut config = Config::default();
    reader.root(doc.get_ref(), &mut config);
    if let (Some(key), Some(line)) = (config.mode.clone(), reader.start)
        && config.find(&key).is_none()
    {
        let names: Vec<&str> = config.modes.iter().map(|m| m.name.as_str()).collect();
        let hint = if names.is_empty() {
            "there are no modes yet; add a [[mode]] or leave mode empty".to_owned()
        } else {
            format!("modes you have: {}", names.join(", "))
        };
        reader
            .diagnostics
            .push(Diagnostic::new(line, format!("there is no mode called “{key}”")).hint(hint));
        config.mode = None;
    }
    reader.diagnostics.sort_by_key(|d| d.line);
    (config, reader.diagnostics)
}

struct Reader<'s> {
    src: &'s str,
    diagnostics: Vec<Diagnostic>,
    start: Option<usize>,
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
                "lock" => {
                    if let Some(t) = self.section(key, value) {
                        self.lock(t, &mut config.lock)
                    }
                }
                "idle" => {
                    if let Some(t) = self.section(key, value) {
                        self.idle(t, &mut config.idle)
                    }
                }
                "desktop" => {
                    if let Some(t) = self.section(key, value) {
                        self.desktop(t, &mut config.desktop)
                    }
                }
                "theme" => {
                    if let Some(t) = self.section(key, value) {
                        let mut name = None;
                        let mut author = None;
                        for (key, value) in t.iter() {
                            match key.get_ref().as_ref() {
                                "name" => name = self.string(key, value).map(str::to_owned),
                                "author" => author = self.string(key, value).map(str::to_owned),
                                _ => self.unknown(key, "option", &["name", "author"]),
                            }
                        }
                        config.theme = name.map(|name| (name, author));
                    }
                }
                "windows" => {
                    if let Some(t) = self.section(key, value) {
                        self.windows(t, &mut config.windows)
                    }
                }
                "rule" => self.rules(key, value, config),
                "mode" => self.modes(key, value, config),
                _ => self.unknown(key, "section", &SECTIONS),
            }
        }
    }

    fn general(&mut self, table: &DeTable, config: &mut Config) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "mode" => {
                    if let Some(name) = self.string(key, value) {
                        config.mode = (!name.trim().is_empty()).then(|| slug(name));
                        self.start = Some(self.line(value.span()));
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
                "position" => {
                    let edges = [("top", Edge::Top), ("bottom", Edge::Bottom)];
                    if let Some(v) = self.choice(key, value, &edges) {
                        bar.position = v
                    }
                }
                "floating" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.floating = v
                    }
                }
                "opacity" => {
                    if let Some(v) = self.fraction(key, value, 0.0..=1.0) {
                        bar.opacity = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &[
                        "workspaces",
                        "seconds",
                        "date",
                        "battery_percent",
                        "position",
                        "floating",
                        "opacity",
                    ],
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
            if combo_is_valid(combo) {
                *shortcuts.get_mut(shortcut) = combo.to_owned();
            } else {
                self.wrong::<()>(key, value, r#"a key combination like "Mod+Space""#);
            }
        }
    }

    fn lock(&mut self, table: &DeTable, lock: &mut Lock) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "before_sleep" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.before_sleep = v
                    }
                }
                "show_media" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.show_media = v
                    }
                }
                _ => self.unknown(key, "option", &["before_sleep", "show_media"]),
            }
        }
    }

    fn idle(&mut self, table: &DeTable, idle: &mut Idle) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "lock_after" => {
                    if let Some(v) = self.number(key, value, 0..=240) {
                        idle.lock_after = v
                    }
                }
                "screen_off_after" => {
                    if let Some(v) = self.number(key, value, 0..=240) {
                        idle.screen_off_after = v
                    }
                }
                "suspend_after" => {
                    if let Some(v) = self.number(key, value, 0..=240) {
                        idle.suspend_after = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["lock_after", "screen_off_after", "suspend_after"],
                ),
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

    fn windows(&mut self, table: &DeTable, windows: &mut Windows) {
        let widths = Width::ALL.map(|w| (w.key(), w));
        let centers = Center::ALL.map(|c| (c.key(), c));
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "width" => windows.width = self.choice(key, value, &widths).or(windows.width),
                "presets" => {
                    if let Some(names) = self.strings(key, value) {
                        let found: Vec<Width> = names
                            .iter()
                            .filter_map(|n| widths.iter().find(|(k, _)| k == n).map(|(_, w)| *w))
                            .collect();
                        if found.len() == names.len() && !found.is_empty() {
                            windows.presets = Some(found);
                        } else {
                            self.wrong::<()>(key, value, r#"a list like ["1/3", "1/2", "2/3"]"#);
                        }
                    }
                }
                "center" => windows.center = self.choice(key, value, &centers).or(windows.center),
                "center_lone" => {
                    if let Some(v) = self.boolean(key, value) {
                        windows.center_lone = v
                    }
                }
                "focus_follows_mouse" => {
                    if let Some(v) = self.boolean(key, value) {
                        windows.focus_follows_mouse = v
                    }
                }
                "warp_mouse" => {
                    if let Some(v) = self.boolean(key, value) {
                        windows.warp_mouse = v
                    }
                }
                "opacity" => {
                    windows.opacity = self.fraction(key, value, 0.2..=1.0).or(windows.opacity)
                }
                "inactive_opacity" => {
                    windows.inactive_opacity = self
                        .fraction(key, value, 0.2..=1.0)
                        .or(windows.inactive_opacity)
                }
                "switcher" => {
                    if let Some(v) = self.boolean(key, value) {
                        windows.switcher = v
                    }
                }
                "animations" => {
                    windows.animations = self.fraction(key, value, 0.0..=4.0).or(windows.animations)
                }
                _ => self.unknown(key, "option", &WINDOW_OPTIONS),
            }
        }
    }

    fn rules(&mut self, key: &Key, value: &Value, config: &mut Config) {
        let DeValue::Array(items) = value.get_ref() else {
            self.wrong::<()>(key, value, "a list of [[rule]] sections");
            return;
        };
        let widths = Width::ALL.map(|w| (w.key(), w));
        for item in items {
            let DeValue::Table(table) = item.get_ref() else {
                continue;
            };
            let mut rule = Rule {
                app: String::new(),
                floating: None,
                width: None,
                opacity: None,
            };
            for (key, value) in table.iter() {
                match key.get_ref().as_ref() {
                    "app" => {
                        if let Some(v) = self.string(key, value) {
                            rule.app = v.trim().to_owned()
                        }
                    }
                    "floating" => rule.floating = self.boolean(key, value),
                    "width" => rule.width = self.choice(key, value, &widths),
                    "opacity" => rule.opacity = self.fraction(key, value, 0.2..=1.0),
                    _ => self.unknown(key, "option", &["app", "floating", "width", "opacity"]),
                }
            }
            if rule.app.is_empty() {
                self.diagnostics.push(
                    Diagnostic::new(self.line(item.span()), "a rule needs an app")
                        .hint("add app = \"pavucontrol\", the app id niri shows"),
                );
            } else {
                config.rules.push(rule);
            }
        }
    }

    fn desktop(&mut self, table: &DeTable, desktop: &mut Desktop) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "wallpaper" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.wallpaper = v.to_owned()
                    }
                }
                "accent" => {
                    if let Some(v) = self.accent(key, value) {
                        desktop.accent = v
                    }
                }
                "theme" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.theme = Some(v.to_owned())
                    }
                }
                _ => self.unknown(key, "option", &["wallpaper", "accent", "theme"]),
            }
        }
    }

    fn modes(&mut self, key: &Key, value: &Value, config: &mut Config) {
        match value.get_ref() {
            DeValue::Array(items) => {
                for item in items {
                    let DeValue::Table(table) = item.get_ref() else {
                        self.wrong::<()>(key, item, "a [[mode]] section");
                        continue;
                    };
                    let mut mode = ModeSettings::blank("");
                    self.mode(table, &mut mode);
                    if mode.name.trim().is_empty() {
                        self.diagnostics.push(
                            Diagnostic::new(self.line(item.span()), "a mode needs a name")
                                .hint("add name = \"Work\" or any other name"),
                        );
                        continue;
                    }
                    self.add(config, mode, item.span());
                }
            }
            DeValue::Table(table) => {
                let mut entries: Vec<_> = table.iter().collect();
                entries.sort_by_key(|(name, _)| name.span().start);
                for (name, value) in entries {
                    if let Some(t) = self.section(name, value) {
                        let mut title = name.get_ref().to_string();
                        if let Some(first) = title.get_mut(..1) {
                            first.make_ascii_uppercase();
                        }
                        let mut mode = ModeSettings::template(&title);
                        self.mode(t, &mut mode);
                        self.add(config, mode, value.span());
                    }
                }
            }
            _ => {
                self.wrong::<()>(key, value, "a list of [[mode]] sections");
            }
        }
    }

    fn add(&mut self, config: &mut Config, mode: ModeSettings, span: Range<usize>) {
        if config.find(&mode.name).is_some() {
            let message = format!("two modes are called “{}”", mode.name.trim());
            self.diagnostics.push(
                Diagnostic::new(self.line(span), message).hint("give each mode its own name"),
            );
            return;
        }
        config.modes.push(mode);
    }

    fn mode(&mut self, table: &DeTable, settings: &mut ModeSettings) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "name" => {
                    if let Some(v) = self.string(key, value) {
                        settings.name = v.trim().to_owned()
                    }
                }
                "icon" => {
                    if let Some(v) = self.string(key, value) {
                        settings.icon = v.to_owned()
                    }
                }
                "wallpaper" => {
                    if let Some(v) = self.string(key, value) {
                        settings.wallpaper = Some(v.to_owned())
                    }
                }
                "accent" => {
                    if let Some(v) = self.accent(key, value) {
                        settings.accent = Some(v)
                    }
                }
                "performance" => {
                    if let Some(v) = self.boolean(key, value) {
                        settings.performance = v
                    }
                }
                "keep_awake" => {
                    if let Some(v) = self.boolean(key, value) {
                        settings.keep_awake = v
                    }
                }
                "keys" => {
                    if let Some(v) = self.keys(key, value) {
                        settings.keys = v
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
                _ => self.unknown(key, "option", &MODE_OPTIONS),
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

    fn keys(&mut self, key: &Key, value: &Value) -> Option<Vec<(String, String)>> {
        let DeValue::Table(table) = value.get_ref() else {
            return self.wrong(key, value, r#"keys like { "Mod+T" = "foot" }"#);
        };
        let mut keys = Vec::new();
        for (combo, command) in table.iter() {
            let Some(command) = self.string(combo, command) else {
                continue;
            };
            if combo_is_valid(combo.get_ref()) {
                keys.push((combo.get_ref().to_string(), command.to_owned()));
            } else {
                self.diagnostics.push(
                    Diagnostic::new(
                        self.line(combo.span()),
                        format!("“{}” is not a key combination", combo.get_ref()),
                    )
                    .hint("write it like Mod+T or Mod+Shift+Return"),
                );
            }
        }
        Some(keys)
    }

    fn fraction(&mut self, key: &Key, value: &Value, range: RangeInclusive<f32>) -> Option<f32> {
        let parsed = match value.get_ref() {
            DeValue::Float(f) => f.as_str().parse::<f32>().ok(),
            DeValue::Integer(n) => n.as_str().parse::<f32>().ok(),
            _ => None,
        };
        match parsed {
            Some(v) if range.contains(&v) => Some(v),
            _ => self.wrong(
                key,
                value,
                &format!("a number from {} to {}", range.start(), range.end()),
            ),
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

fn combo_is_valid(combo: &str) -> bool {
    !combo.is_empty()
        && !combo.starts_with('+')
        && !combo.ends_with('+')
        && combo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '_')
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

use std::ops::{Range, RangeInclusive};

use apolline_theme::Rgba;
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::{
    Accent, Alerts, Appear, Blur, Calendar, Center, Config, Corner, Desktop, Diagnostic, Display,
    Edge, Idle, Keyboard, Layout, Lock, ModeSettings, Module, Motion, Mouse, Notifications, Piece,
    Rule, Shortcut, Shortcuts, Switch, TRANSFORMS, Touchpad, TrayItem, Trigger, Width, Windows,
    Workspaces, slug,
};

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

const WORKSPACES: [(&str, Workspaces); 2] =
    [("dots", Workspaces::Dots), ("numbers", Workspaces::Numbers)];

const SECTIONS: [&str; 21] = [
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
    "apps",
    "night",
    "style",
    "windows",
    "rule",
    "display",
    "module",
    "template",
    "mode",
];

const MODE_OPTIONS: [&str; 11] = [
    "name",
    "icon",
    "wallpaper",
    "accent",
    "notifications",
    "open",
    "performance",
    "keep_awake",
    "keys",
    "when",
    "theme",
];

const WINDOW_OPTIONS: [&str; 13] = [
    "width",
    "presets",
    "center",
    "center_lone",
    "focus_follows_mouse",
    "warp_mouse",
    "opacity",
    "inactive_opacity",
    "animations",
    "motion",
    "appear",
    "blur",
    "switcher",
];

const CALENDARS: &str = r#"a list of files and links, or { caldav = "…", user = "…" }"#;

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
        placed: Vec::new(),
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
    reader.settle_bar(&mut config);
    reader.diagnostics.sort_by_key(|d| d.line);
    (config, reader.diagnostics)
}

struct Reader<'s> {
    src: &'s str,
    diagnostics: Vec<Diagnostic>,
    start: Option<usize>,
    // Names placed on the bar and their lines, checked once [[module]]s are known.
    placed: Vec<(String, usize)>,
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
                        self.bar(t, config)
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
                "style" => {
                    if let Some(t) = self.section(key, value) {
                        for (key, value) in t.iter() {
                            let surface = match key.get_ref().as_ref() {
                                "launcher" => &mut config.style.launcher,
                                "panel" => &mut config.style.panel,
                                "notifications" => &mut config.style.cards,
                                "osd" => &mut config.style.osd,
                                "bar" => &mut config.style.bar,
                                _ => {
                                    self.unknown(
                                        key,
                                        "surface",
                                        &["launcher", "panel", "bar", "notifications", "osd"],
                                    );
                                    continue;
                                }
                            };
                            let launcher = key.get_ref().as_ref() == "launcher";
                            let Some(table) = self.section(key, value) else {
                                continue;
                            };
                            let mut grid = None;
                            for (key, value) in table.iter() {
                                match key.get_ref().as_ref() {
                                    "radius" => surface.radius = self.number(key, value, 0..=40),
                                    "opacity" => {
                                        surface.opacity = self.fraction(key, value, 0.3..=1.0)
                                    }
                                    "size" => surface.size = self.fraction(key, value, 0.8..=1.5),
                                    "grid" if launcher => grid = self.boolean(key, value),
                                    _ => self.unknown(
                                        key,
                                        "option",
                                        if launcher {
                                            &["radius", "opacity", "size", "grid"]
                                        } else {
                                            &["radius", "opacity", "size"]
                                        },
                                    ),
                                }
                            }
                            if let Some(grid) = grid {
                                config.style.grid = grid;
                            }
                        }
                    }
                }
                "night" => {
                    if let Some(t) = self.section(key, value) {
                        for (key, value) in t.iter() {
                            match key.get_ref().as_ref() {
                                "hours" => match self.string(key, value).and_then(Trigger::parse) {
                                    Some(Trigger::Hours { from, until }) => {
                                        config.night.hours = Some((from, until))
                                    }
                                    _ => {
                                        self.wrong::<()>(key, value, r#"hours like "21:00-07:00""#);
                                    }
                                },
                                "temperature" => {
                                    if let Some(v) = self.number(key, value, 1000..=6500) {
                                        config.night.temperature = u32::from(v)
                                    }
                                }
                                _ => self.unknown(key, "option", &["hours", "temperature"]),
                            }
                        }
                    }
                }
                "apps" => {
                    if let Some(t) = self.section(key, value) {
                        for (key, value) in t.iter() {
                            match key.get_ref().as_ref() {
                                "colors" => {
                                    if let Some(v) = self.boolean(key, value) {
                                        config.apps.colors = v
                                    }
                                }
                                _ => self.unknown(key, "option", &["colors"]),
                            }
                        }
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
                "display" => self.displays(key, value, config),
                "module" => self.modules(key, value, config),
                "template" => self.templates(key, value, config),
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
                "gradient" => {
                    if let Some(v) = self.boolean(key, value) {
                        layout.gradient = v
                    }
                }
                "shadows" => {
                    if let Some(v) = self.boolean(key, value) {
                        layout.shadows = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["gaps", "radius", "borders", "gradient", "shadows"],
                ),
            }
        }
    }

    fn bar(&mut self, table: &DeTable, config: &mut Config) {
        let bar = &mut config.bar;
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
                "shape" => {
                    let shapes = [
                        ("bar", crate::Shape::Bar),
                        ("islands", crate::Shape::Islands),
                    ];
                    if let Some(v) = self.choice(key, value, &shapes) {
                        bar.shape = v
                    }
                }
                "opacity" => {
                    if let Some(v) = self.fraction(key, value, 0.0..=1.0) {
                        bar.opacity = v
                    }
                }
                "blur" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.blur = v
                    }
                }
                "left" | "center" | "right" => {
                    if let Some(names) = self.strings(key, value) {
                        let line = self.line(value.span());
                        self.placed
                            .extend(names.iter().map(|name| (name.clone(), line)));
                        let pieces = names.iter().map(|name| Piece::named(name)).collect();
                        match key.get_ref().as_ref() {
                            "left" => bar.left = pieces,
                            "center" => bar.center = pieces,
                            _ => bar.right = pieces,
                        }
                    }
                }
                "tray" => {
                    if let Some(names) = self.strings(key, value) {
                        let mut tray = Vec::new();
                        for name in &names {
                            match TrayItem::named(name) {
                                Some(item) if !tray.contains(&item) => tray.push(item),
                                Some(_) => {}
                                None => self.unknown_name(
                                    value,
                                    name,
                                    "tray item",
                                    &TrayItem::ALL.map(|(name, _)| name),
                                ),
                            }
                        }
                        bar.tray = tray;
                    }
                }
                "stats" => {
                    if let Some(names) = self.strings(key, value) {
                        let mut stats = crate::Stats::default();
                        for name in &names {
                            match name.as_str() {
                                "cpu" => stats.cpu = true,
                                "memory" => stats.memory = true,
                                "temperature" => stats.temperature = true,
                                _ => {
                                    self.wrong::<()>(
                                        key,
                                        value,
                                        r#"a list of "cpu", "memory" and "temperature""#,
                                    );
                                }
                            }
                        }
                        bar.stats = stats;
                    }
                }
                "pinned" => {
                    if let Some(v) = self.strings(key, value) {
                        config.pinned = v
                    }
                }
                "weather" => {
                    if let Some(v) = self.string(key, value) {
                        config.weather = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "calendars" => {
                    if let Some(v) = self.calendars(key, value) {
                        config.calendars = v
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
                        "shape",
                        "opacity",
                        "blur",
                        "pinned",
                        "weather",
                        "calendars",
                        "stats",
                        "left",
                        "center",
                        "right",
                        "tray",
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
                "fingerprint" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.fingerprint = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["before_sleep", "show_media", "fingerprint"],
                ),
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
                "position" => {
                    if let Some(v) = self.choice(key, value, &Corner::ALL) {
                        alerts.position = v
                    }
                }
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
                "mute" => {
                    if let Some(v) = self.strings(key, value) {
                        alerts.mute = v
                    }
                }
                "always" => {
                    if let Some(v) = self.strings(key, value) {
                        alerts.always = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &["position", "do_not_disturb", "seconds", "mute", "always"],
                ),
            }
        }
    }

    fn windows(&mut self, table: &DeTable, windows: &mut Windows) {
        let widths = Width::ALL.map(|w| (w.key(), w));
        let centers = Center::ALL.map(|c| (c.key(), c));
        let motions = Motion::ALL.map(|m| (m.key(), m));
        let appears = Appear::ALL.map(|a| (a.key(), a));
        let blurs = Blur::ALL.map(|b| (b.key(), b));
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
                "motion" => windows.motion = self.choice(key, value, &motions).or(windows.motion),
                "appear" => windows.appear = self.choice(key, value, &appears).or(windows.appear),
                "blur" => windows.blur = self.choice(key, value, &blurs).or(windows.blur),
                _ => self.unknown(key, "option", &WINDOW_OPTIONS),
            }
        }
    }

    fn modules(&mut self, key: &Key, value: &Value, config: &mut Config) {
        let DeValue::Array(items) = value.get_ref() else {
            self.wrong::<()>(key, value, "a list of [[module]] sections");
            return;
        };
        for item in items {
            let DeValue::Table(table) = item.get_ref() else {
                continue;
            };
            let mut module = Module {
                name: None,
                command: String::new(),
                every: None,
                click: None,
            };
            for (key, value) in table.iter() {
                match key.get_ref().as_ref() {
                    "name" => {
                        module.name = self
                            .string(key, value)
                            .map(|v| v.trim().to_owned())
                            .filter(|v| !v.is_empty())
                    }
                    "command" => {
                        if let Some(v) = self.string(key, value) {
                            module.command = v.trim().to_owned()
                        }
                    }
                    "every" => module.every = self.number(key, value, 1..=65535).map(u32::from),
                    "click" => {
                        module.click = self
                            .string(key, value)
                            .map(|v| v.trim().to_owned())
                            .filter(|v| !v.is_empty())
                    }
                    _ => self.unknown(key, "option", &["name", "command", "every", "click"]),
                }
            }
            if module.command.is_empty() {
                self.diagnostics.push(
                    Diagnostic::new(self.line(item.span()), "a module needs a command")
                        .hint("add command = \"date +%A\"; each line it prints shows on the bar"),
                );
            } else {
                config.modules.push(module);
            }
        }
    }

    fn templates(&mut self, key: &Key, value: &Value, config: &mut Config) {
        let DeValue::Array(items) = value.get_ref() else {
            self.wrong::<()>(key, value, "a list of [[template]] sections");
            return;
        };
        for item in items {
            let DeValue::Table(table) = item.get_ref() else {
                continue;
            };
            let mut template = crate::Template {
                from: String::new(),
                to: String::new(),
                after: None,
            };
            for (key, value) in table.iter() {
                let text = |reader: &mut Self| {
                    reader
                        .string(key, value)
                        .map(|v| v.trim().to_owned())
                        .filter(|v| !v.is_empty())
                };
                match key.get_ref().as_ref() {
                    "from" => template.from = text(self).unwrap_or_default(),
                    "to" => template.to = text(self).unwrap_or_default(),
                    "after" => template.after = text(self),
                    _ => self.unknown(key, "option", &["from", "to", "after"]),
                }
            }
            if template.from.is_empty() || template.to.is_empty() {
                self.diagnostics.push(
                    Diagnostic::new(self.line(item.span()), "a template needs from and to")
                        .hint("from = \"~/.config/apolline/templates/btop.theme\", to = \"~/.config/btop/themes/apolline.theme\""),
                );
            } else if template.from == template.to {
                self.diagnostics.push(Diagnostic::new(
                    self.line(item.span()),
                    "a template can't be written over itself",
                ));
            } else {
                config.templates.push(template);
            }
        }
    }

    fn displays(&mut self, key: &Key, value: &Value, config: &mut Config) {
        let DeValue::Array(items) = value.get_ref() else {
            self.wrong::<()>(key, value, "a list of [[display]] sections");
            return;
        };
        for item in items {
            let DeValue::Table(table) = item.get_ref() else {
                continue;
            };
            let mut display = Display {
                name: String::new(),
                mode: None,
                scale: None,
                transform: None,
                position: None,
                vrr: None,
            };
            for (key, value) in table.iter() {
                match key.get_ref().as_ref() {
                    "name" => {
                        if let Some(v) = self.string(key, value) {
                            display.name = v.trim().to_owned()
                        }
                    }
                    "mode" => match self.string(key, value) {
                        Some(v) if mode_is_valid(v) => display.mode = Some(v.to_owned()),
                        Some(_) => {
                            self.wrong::<()>(
                                key,
                                value,
                                r#"a mode like "2560x1440" or "2560x1440@143.912""#,
                            );
                        }
                        None => {}
                    },
                    "scale" => {
                        display.scale = self
                            .fraction(key, value, 0.5..=4.0)
                            .map(|s| (f64::from(s) * 1000.0).round() / 1000.0);
                    }
                    "transform" => match self.string(key, value) {
                        Some(v) if TRANSFORMS.contains(&v) => {
                            display.transform = Some(v.to_owned())
                        }
                        Some(_) => {
                            self.wrong::<()>(key, value, r#"one of "normal", "90", "180", "270", or "flipped" and its turns"#);
                        }
                        None => {}
                    },
                    "position" => match value.get_ref() {
                        DeValue::Array(xy) if xy.len() == 2 => {
                            let number = |v: &Spanned<DeValue>| match v.get_ref() {
                                DeValue::Integer(i) => i.as_str().parse::<i32>().ok(),
                                _ => None,
                            };
                            display.position = number(&xy[0]).zip(number(&xy[1]));
                        }
                        _ => {
                            self.wrong::<()>(key, value, "two numbers, like [1920, 0]");
                        }
                    },
                    "vrr" => display.vrr = self.boolean(key, value),
                    _ => self.unknown(
                        key,
                        "option",
                        &["name", "mode", "scale", "transform", "position", "vrr"],
                    ),
                }
            }
            if display.name.is_empty() {
                self.diagnostics.push(
                    Diagnostic::new(self.line(item.span()), "a display needs a name")
                        .hint("add name = \"HDMI-A-1\", as niri msg outputs shows it"),
                );
            } else {
                config.displays.push(display);
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
                "palette" => {
                    let names: Vec<(&str, ())> = apolline_theme::palettes::NAMED
                        .iter()
                        .map(|(name, ..)| *name)
                        .chain(crate::WALLPAPER_PALETTES)
                        .map(|name| (name, ()))
                        .collect();
                    if self.choice(key, value, &names).is_some() {
                        desktop.palette = value.get_ref().as_str().unwrap_or("dark").to_owned()
                    }
                }
                "theme" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.theme = Some(v.to_owned())
                    }
                }
                "folders" => {
                    if let Some(v) = self.strings(key, value) {
                        desktop.folders = v
                    }
                }
                "rotate" => {
                    if let Some(v) = self.number(key, value, 0..=10080) {
                        desktop.rotate = u32::from(v)
                    }
                }
                "shuffle" => {
                    if let Some(v) = self.boolean(key, value) {
                        desktop.shuffle = v
                    }
                }
                "cursor" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.cursor = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "cursor_size" => desktop.cursor_size = self.number(key, value, 12..=96),
                "icons" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.icons = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "font" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.font = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "gtk" => {
                    if let Some(v) = self.string(key, value) {
                        desktop.gtk = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "outputs" => {
                    let DeValue::Table(table) = value.get_ref() else {
                        self.wrong::<()>(
                            key,
                            value,
                            r#"monitors and pictures, like { "HDMI-A-1" = "wide.jpg" }"#,
                        );
                        continue;
                    };
                    desktop.outputs = table
                        .iter()
                        .filter_map(|(output, picture)| {
                            let picture = self.string(output, picture)?;
                            Some((output.get_ref().to_string(), picture.to_owned()))
                        })
                        .collect();
                }
                _ => self.unknown(
                    key,
                    "option",
                    &[
                        "wallpaper",
                        "accent",
                        "palette",
                        "theme",
                        "folders",
                        "rotate",
                        "shuffle",
                        "outputs",
                        "cursor",
                        "cursor_size",
                        "icons",
                        "font",
                        "gtk",
                    ],
                ),
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
                "theme" => {
                    if let Some(v) = self.string(key, value) {
                        settings.theme = Some(v.trim().to_owned()).filter(|v| !v.is_empty())
                    }
                }
                "when" => {
                    if let Some(items) = self.strings(key, value) {
                        let parsed: Vec<Option<Trigger>> =
                            items.iter().map(|item| Trigger::parse(item)).collect();
                        if parsed.iter().all(Option::is_some) {
                            settings.when = parsed.into_iter().flatten().collect();
                        } else {
                            self.wrong::<()>(
                                key,
                                value,
                                r#"a list like ["game"], ["battery"], ["20:00-07:00"] or ["monitor HDMI-A-1"]"#,
                            );
                        }
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

    // Files and links as text, CalDAV collections as { caldav = "…", user = "…" }.
    fn calendars(&mut self, key: &Key, value: &Value) -> Option<Vec<Calendar>> {
        let Some(items) = value.get_ref().as_array() else {
            return self.wrong(key, value, CALENDARS);
        };
        let mut calendars = Vec::new();
        for item in items {
            match item.get_ref() {
                DeValue::String(link) => calendars.push(Calendar::Ics(link.to_string())),
                DeValue::Table(table) => calendars.extend(self.caldav(item, table)),
                _ => return self.wrong(key, value, CALENDARS),
            }
        }
        Some(calendars)
    }

    fn caldav(&mut self, item: &Value, table: &DeTable) -> Option<Calendar> {
        let (mut url, mut user) = (String::new(), String::new());
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "caldav" => {
                    url = self
                        .string(key, value)
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                }
                "user" => {
                    user = self
                        .string(key, value)
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                }
                "password" => self.diagnostics.push(
                    Diagnostic::new(
                        self.line(key.span()),
                        "passwords aren't read from this file",
                    )
                    .hint("add the calendar in Settings → Bar; its password goes to your keyring"),
                ),
                _ => self.unknown(key, "option", &["caldav", "user"]),
            }
        }
        if url.is_empty() || user.is_empty() {
            self.diagnostics.push(
                Diagnostic::new(
                    self.line(item.span()),
                    "a CalDAV calendar needs caldav and user",
                )
                .hint(r#"write it like { caldav = "https://…/calendars/me/home/", user = "me" }"#),
            );
            return None;
        }
        Some(Calendar::CalDav { url, user })
    }

    fn accent(&mut self, key: &Key, value: &Value) -> Option<Accent> {
        match value.get_ref().as_str() {
            Some("auto") => Some(Accent::Auto),
            Some("palette") => Some(Accent::Palette),
            Some(text) if Rgba::parse(text).is_some() => Rgba::parse(text).map(Accent::Fixed),
            _ => self.wrong(
                key,
                value,
                r##""auto", "palette" or a color like "#ff6a3d""##,
            ),
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

    // Pieces on the bar that name nothing, or that are there twice, come off it.
    fn settle_bar(&mut self, config: &mut Config) {
        let modules: Vec<&str> = config
            .modules
            .iter()
            .filter_map(|m| m.name.as_deref())
            .collect();
        let mut known: Vec<&str> = Piece::NAMED.iter().map(|(name, _)| *name).collect();
        known.extend(modules.iter().copied());
        let mut seen: Vec<Piece> = Vec::new();
        let bar = &mut config.bar;
        for zone in [&mut bar.left, &mut bar.center, &mut bar.right] {
            let mut kept = Vec::new();
            for piece in std::mem::take(zone) {
                let line = self
                    .placed
                    .iter()
                    .find(|(name, _)| name == piece.name())
                    .map_or(0, |(_, line)| *line);
                let missing =
                    matches!(&piece, Piece::Module(name) if !modules.contains(&name.as_str()));
                if missing {
                    let name = piece.name();
                    let hint = match suggest(name, &known) {
                        Some(close) => format!("did you mean “{close}”?"),
                        None => format!(
                            "known: {}, or the name of a [[module]]",
                            Piece::NAMED.map(|(name, _)| name).join(", ")
                        ),
                    };
                    self.diagnostics.push(
                        Diagnostic::new(line, format!("nothing on the bar is called “{name}”"))
                            .hint(hint),
                    );
                } else if seen.contains(&piece) {
                    self.diagnostics.push(Diagnostic::new(
                        line,
                        format!(
                            "“{}” is on the bar twice; it stays where it's first",
                            piece.name()
                        ),
                    ));
                } else {
                    seen.push(piece.clone());
                    kept.push(piece);
                }
            }
            *zone = kept;
        }
    }

    fn unknown_name(&mut self, value: &Value, name: &str, what: &str, known: &[&str]) {
        let hint = match suggest(name, known) {
            Some(close) => format!("did you mean “{close}”?"),
            None => format!("known: {}", known.join(", ")),
        };
        self.diagnostics.push(
            Diagnostic::new(self.line(value.span()), format!("unknown {what} “{name}”")).hint(hint),
        );
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

fn mode_is_valid(mode: &str) -> bool {
    let (size, rate) = mode
        .split_once('@')
        .map_or((mode, None), |(s, r)| (s, Some(r)));
    let size_ok = size
        .split_once('x')
        .is_some_and(|(w, h)| w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok());
    size_ok && rate.is_none_or(|r| r.parse::<f64>().is_ok())
}

use std::ops::{Range, RangeInclusive};

use apolline_theme::Rgba;
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::{
    Accent, Alerts, Appear, Bar, Blur, Calendar, Center, Config, Corner, Desktop, Diagnostic,
    Display, Edge, Idle, IdleTimes, Keyboard, Layout, Lock, ModeSettings, Module, Motion, Mouse,
    Notifications, Piece, Plugin, Rule, Shortcut, Shortcuts, Switch, TRANSFORMS, Touchpad,
    TrayItem, Trigger, Width, Windows, Workspaces, slug,
};

pub(crate) type Key<'i> = Spanned<DeString<'i>>;
pub(crate) type Value<'i> = Spanned<DeValue<'i>>;

const LANGUAGES: [(&str, &str); 3] = [("auto", "auto"), ("en", "en"), ("ru", "ru")];

const WORKSPACES: [(&str, Workspaces); 2] =
    [("dots", Workspaces::Dots), ("numbers", Workspaces::Numbers)];

const SECTIONS: [&str; 26] = [
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
    "colors",
    "panel",
    "clock",
    "launcher",
    "plugins",
];

const MODULE_OPTIONS: [&str; 8] = [
    "name",
    "command",
    "every",
    "click",
    "right_click",
    "middle_click",
    "scroll_up",
    "scroll_down",
];

const MODE_OPTIONS: [&str; 12] = [
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
    "replay",
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

// Plugins, when given, are what names like "ip.address" are checked against; without them such
// names are taken on trust, since what's installed is the shell's to know.
pub(crate) fn document(src: &str, plugins: Option<&[Plugin]>) -> (Config, Vec<Diagnostic>) {
    let (doc, mut reader) = Reader::start(src);
    reader.plugins = plugins;

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
    reader.settle_widgets(&mut config);
    reader.settle_plugins(&config);
    reader.diagnostics.sort_by_key(|d| d.line);
    (config, reader.diagnostics)
}

pub(crate) struct Reader<'s> {
    src: &'s str,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) plugins: Option<&'s [Plugin]>,
    start: Option<usize>,
    // Names placed on the bar and their lines, checked once [[module]]s are known.
    placed: Vec<(String, usize)>,
    // Where [desktop] widgets is, checked the same way.
    widgets_line: usize,
    // [style.bar] opacity, used when [bar] opacity isn't given.
    style_bar_opacity: Option<f32>,
    bar_opacity_set: bool,
    // Plugin tiles in [panel] tiles and their line, checked once [plugins] is known.
    pub(crate) plugin_tiles: Vec<(String, usize)>,
}

impl<'s> Reader<'s> {
    pub(crate) fn start(src: &'s str) -> (Spanned<DeTable<'s>>, Self) {
        let (doc, errors) = DeTable::parse_recoverable(src);
        let mut reader = Reader {
            src,
            diagnostics: Vec::new(),
            plugins: None,
            start: None,
            placed: Vec::new(),
            widgets_line: 0,
            style_bar_opacity: None,
            bar_opacity_set: false,
            plugin_tiles: Vec::new(),
        };
        for error in &errors {
            let line = error.span().map_or(0, |span| reader.line(span));
            let message = error.message().lines().next().unwrap_or("syntax error");
            reader.diagnostics.push(Diagnostic::new(line, message));
        }
        (doc, reader)
    }
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
                            let bar = key.get_ref().as_ref() == "bar";
                            let Some(table) = self.section(key, value) else {
                                continue;
                            };
                            let mut grid = None;
                            for (key, value) in table.iter() {
                                match key.get_ref().as_ref() {
                                    "radius" => surface.radius = self.number(key, value, 0..=40),
                                    // The bar's glass is [bar] opacity; this is the same, for
                                    // themes that wrote it here, and [bar] wins if both are set.
                                    "opacity" if bar => {
                                        self.style_bar_opacity =
                                            self.fraction(key, value, 0.0..=1.0)
                                    }
                                    "opacity" => {
                                        surface.opacity = self.fraction(key, value, 0.3..=1.0)
                                    }
                                    "size" => surface.size = self.fraction(key, value, 0.8..=1.5),
                                    "font" => {
                                        surface.font = self
                                            .string(key, value)
                                            .filter(|v| !v.trim().is_empty())
                                            .map(crate::FontName::new)
                                    }
                                    "grid" if launcher => grid = self.boolean(key, value),
                                    _ => self.unknown(
                                        key,
                                        "option",
                                        if launcher {
                                            &["radius", "opacity", "size", "font", "grid"]
                                        } else {
                                            &["radius", "opacity", "size", "font"]
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
                "colors" => {
                    if let Some(t) = self.section(key, value) {
                        self.colors(t, &mut config.colors)
                    }
                }
                "panel" => {
                    if let Some(t) = self.section(key, value) {
                        self.panel(t, &mut config.panel)
                    }
                }
                "clock" => {
                    if let Some(t) = self.section(key, value) {
                        self.clock(t, &mut config.clock)
                    }
                }
                "launcher" => {
                    if let Some(t) = self.section(key, value) {
                        self.launcher(t, &mut config.launcher)
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
                                "terminal" => match self.string(key, value) {
                                    Some(command) if !command.trim().is_empty() => {
                                        config.terminal = command.trim().to_owned()
                                    }
                                    Some(_) => {
                                        self.wrong::<()>(
                                            key,
                                            value,
                                            "a terminal's command, like \"vif\"",
                                        );
                                    }
                                    None => {}
                                },
                                _ => self.unknown(key, "option", &["colors", "terminal"]),
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
                "plugins" => {
                    if let Some(t) = self.section(key, value) {
                        self.plugins(t, &mut config.plugins)
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
                    if let Some(name) = self.string(key, value) {
                        config.mode = (!name.trim().is_empty()).then(|| slug(name));
                        self.start = Some(self.line(value.span()));
                    }
                }
                "language" => {
                    if let Some(code) = self.choice(key, value, &LANGUAGES) {
                        config.language = (code != "auto").then(|| code.to_owned());
                    }
                }
                _ => self.unknown(key, "option", &["mode", "language"]),
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
                "border_width" => {
                    if let Some(v) = self.number(key, value, 1..=8) {
                        layout.border_width = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &[
                        "gaps",
                        "radius",
                        "borders",
                        "border_width",
                        "gradient",
                        "shadows",
                    ],
                ),
            }
        }
    }

    fn bar(&mut self, table: &DeTable, config: &mut Config) {
        let bar = &mut config.bar;
        let mut screens = Vec::new();
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "screens" => {
                    if let Some(v) = self.strings(key, value) {
                        bar.screens = Some(v)
                    }
                }
                "skip" => {
                    if let Some(v) = self.strings(key, value) {
                        bar.skip = v
                    }
                }
                name if matches!(value.get_ref(), DeValue::Table(_)) => {
                    if BAR_OPTIONS.contains(&name) {
                        self.diagnostics.push(
                            Diagnostic::new(
                                self.line(key.span()),
                                format!("“{name}” is a [bar] option, not a screen"),
                            )
                            .hint(format!("write it as {name} = … under [bar]")),
                        );
                        continue;
                    }
                    if let Some(close) = suggest(name, &BAR_OPTIONS) {
                        self.diagnostics.push(
                            Diagnostic::new(
                                self.line(key.span()),
                                format!("[bar.{name}] is the bar of a screen called “{name}”"),
                            )
                            .hint(format!("did you mean the option “{close}”?")),
                        );
                    }
                    if let Some(t) = self.section(key, value) {
                        let pieces = self.screen_pieces(t);
                        screens.push((name.to_owned(), pieces));
                    }
                }
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
                    if let Some(v) = self.choice(key, value, &Edge::ALL) {
                        bar.position = v
                    }
                }
                "floating" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.floating = v
                    }
                }
                "autohide" => {
                    if let Some(v) = self.boolean(key, value) {
                        bar.autohide = v
                    }
                }
                "shape" => {
                    let shapes = [
                        ("bar", crate::Shape::Bar),
                        ("islands", crate::Shape::Islands),
                        ("pill", crate::Shape::Pill),
                    ];
                    if let Some(v) = self.choice(key, value, &shapes) {
                        bar.shape = v
                    }
                }
                "opacity" => {
                    if let Some(v) = self.fraction(key, value, 0.0..=1.0) {
                        bar.opacity = v;
                        self.bar_opacity_set = true;
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
                                _ => self.unknown_name(
                                    value,
                                    name,
                                    "stat",
                                    &["cpu", "memory", "temperature"],
                                ),
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
                _ => self.unknown(key, "option", &BAR_OPTIONS),
            }
        }
        // Each screen's bar is the common one with its own pieces, so a screen named later
        // than an option still gets that option.
        let common = Bar {
            per_screen: Vec::new(),
            ..bar.clone()
        };
        bar.per_screen = screens
            .into_iter()
            .map(|(name, [left, center, right])| {
                let screen = Bar {
                    left: left.unwrap_or_else(|| common.left.clone()),
                    center: center.unwrap_or_else(|| common.center.clone()),
                    right: right.unwrap_or_else(|| common.right.clone()),
                    ..common.clone()
                };
                (name, screen)
            })
            .collect();
    }

    // left, center and right for one screen, in [bar."DP-1"].
    fn screen_pieces(&mut self, table: &DeTable) -> [Option<Vec<Piece>>; 3] {
        let mut sides = [None, None, None];
        for (key, value) in table.iter() {
            let side = match key.get_ref().as_ref() {
                "left" => 0,
                "center" => 1,
                "right" => 2,
                _ => {
                    self.unknown(key, "option", &["left", "center", "right"]);
                    continue;
                }
            };
            if let Some(names) = self.strings(key, value) {
                let line = self.line(value.span());
                self.placed
                    .extend(names.iter().map(|name| (name.clone(), line)));
                sides[side] = Some(names.iter().map(|name| Piece::named(name)).collect());
            }
        }
        sides
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
            if let Some(combo) = key_combo(combo) {
                *shortcuts.get_mut(shortcut) = combo;
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
                "show_weather" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.show_weather = v
                    }
                }
                "show_next" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.show_next = v
                    }
                }
                "show_count" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.show_count = v
                    }
                }
                "fingerprint" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.fingerprint = v
                    }
                }
                "layout" => {
                    let layouts = [("corner", false), ("center", true)];
                    if let Some(v) = self.choice(key, value, &layouts) {
                        lock.centered = v
                    }
                }
                "blur" => {
                    if let Some(v) = self.boolean(key, value) {
                        lock.blur = v
                    }
                }
                _ => self.unknown(
                    key,
                    "option",
                    &[
                        "before_sleep",
                        "show_media",
                        "show_weather",
                        "show_next",
                        "show_count",
                        "fingerprint",
                        "layout",
                        "blur",
                    ],
                ),
            }
        }
    }

    fn idle(&mut self, table: &DeTable, idle: &mut Idle) {
        let times = self.idle_times(table, Some(&mut idle.battery));
        idle.lock_after = times.lock_after.unwrap_or(idle.lock_after);
        idle.screen_off_after = times.screen_off_after.unwrap_or(idle.screen_off_after);
        idle.suspend_after = times.suspend_after.unwrap_or(idle.suspend_after);
    }

    // The minutes in [idle], and in [idle.battery] when there's a place for them.
    fn idle_times(&mut self, table: &DeTable, battery: Option<&mut IdleTimes>) -> IdleTimes {
        let mut times = IdleTimes::default();
        let mut battery = battery;
        for (key, value) in table.iter() {
            let slot = match key.get_ref().as_ref() {
                "lock_after" => &mut times.lock_after,
                "screen_off_after" => &mut times.screen_off_after,
                "suspend_after" => &mut times.suspend_after,
                "battery" if battery.is_some() => {
                    if let (Some(t), Some(battery)) = (self.section(key, value), battery.as_mut()) {
                        **battery = self.idle_times(t, None);
                    }
                    continue;
                }
                _ => {
                    let known = ["lock_after", "screen_off_after", "suspend_after", "battery"];
                    let known = if battery.is_some() {
                        &known[..]
                    } else {
                        &known[..3]
                    };
                    self.unknown(key, "option", known);
                    continue;
                }
            };
            if let Some(v) = self.number(key, value, 0..=240) {
                *slot = Some(v)
            }
        }
        times
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
                "silent" => {
                    if let Some(v) = self.strings(key, value) {
                        alerts.silent = v
                    }
                }
                "cards" => {
                    if let Some(v) = self.number(key, value, 1..=8) {
                        alerts.cards = v
                    }
                }
                "width" => {
                    if let Some(v) = self.number(key, value, 280..=560) {
                        alerts.width = v
                    }
                }
                "keep" => {
                    if let Some(v) = self.boolean(key, value) {
                        alerts.keep = v
                    }
                }
                "quiet_hours" => match self.string(key, value).map(crate::Trigger::parse) {
                    Some(Some(crate::Trigger::Hours { from, until })) => {
                        alerts.quiet_hours = Some((from, until))
                    }
                    Some(_) => {
                        self.wrong::<()>(key, value, r#"hours like "23:00-07:00""#);
                    }
                    None => {}
                },
                "sound" => match value.get_ref() {
                    DeValue::Boolean(on) => {
                        alerts.sound = if *on {
                            crate::Sound::Theme
                        } else {
                            crate::Sound::Off
                        }
                    }
                    DeValue::String(path) if !path.trim().is_empty() => {
                        alerts.sound = crate::Sound::File(path.trim().to_owned())
                    }
                    _ => {
                        self.wrong::<()>(key, value, "true, false, or a sound file");
                    }
                },
                _ => self.unknown(
                    key,
                    "option",
                    &[
                        "position",
                        "do_not_disturb",
                        "seconds",
                        "mute",
                        "always",
                        "silent",
                        "cards",
                        "width",
                        "sound",
                        "quiet_hours",
                        "keep",
                    ],
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
            let module = self.module(table);
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

    // A [[module]], or a plugin's [[bar]] or [[widget]], which are the same thing.
    pub(crate) fn module(&mut self, table: &DeTable) -> Module {
        let mut module = Module {
            name: None,
            command: String::new(),
            every: None,
            click: None,
            right_click: None,
            middle_click: None,
            scroll_up: None,
            scroll_down: None,
            plugin: None,
        };
        for (key, value) in table.iter() {
            let text = |me: &mut Self| me.text(key, value);
            match key.get_ref().as_ref() {
                "name" => module.name = text(self),
                "command" => module.command = text(self).unwrap_or_default(),
                "every" => module.every = self.number(key, value, 1..=65535).map(u32::from),
                "click" => module.click = text(self),
                "right_click" => module.right_click = text(self),
                "middle_click" => module.middle_click = text(self),
                "scroll_up" => module.scroll_up = text(self),
                "scroll_down" => module.scroll_down = text(self),
                _ => self.unknown(key, "option", &MODULE_OPTIONS),
            }
        }
        module
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
                "widgets" => {
                    if let Some(names) = self.strings(key, value) {
                        self.widgets_line = self.line(value.span());
                        desktop.widgets = names
                            .into_iter()
                            .map(|n| n.trim().to_owned())
                            .filter(|n| !n.is_empty())
                            .collect();
                    }
                }
                "clock" => {
                    let mut spots: Vec<(&str, Option<crate::Spot>)> =
                        vec![("off", None), ("center", Some(crate::Spot::Center))];
                    spots.extend(
                        Corner::ALL
                            .iter()
                            .map(|(name, c)| (*name, Some(crate::Spot::Corner(*c)))),
                    );
                    if let Some(v) = self.choice(key, value, &spots) {
                        desktop.clock = v
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
                        "clock",
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
                        "widgets",
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
                "replay" => {
                    if let Some(v) = self.number(key, value, 0..=600) {
                        settings.replay = v
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

    fn launcher(&mut self, table: &DeTable, launcher: &mut crate::Launcher) {
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "finds" => {
                    if let Some(names) = self.strings(key, value) {
                        let mut kept = Vec::new();
                        for name in names {
                            if !crate::Launcher::FINDS.contains(&name.as_str()) {
                                self.unknown_name(value, &name, "source", &crate::Launcher::FINDS);
                            } else if !kept.contains(&name) {
                                kept.push(name);
                            }
                        }
                        launcher.finds = kept;
                    }
                }
                "web" => match self.string(key, value) {
                    Some(link)
                        if (link.starts_with("https://") || link.starts_with("http://"))
                            && link.contains("{query}") =>
                    {
                        launcher.web = Some(link.to_owned());
                    }
                    Some(_) => {
                        self.wrong::<()>(
                            key,
                            value,
                            r#"a link with {query} in it, like "https://www.startpage.com/do/search?q={query}""#,
                        );
                    }
                    None => {}
                },
                _ => self.unknown(key, "option", &["finds", "web"]),
            }
        }
    }

    fn clock(&mut self, table: &DeTable, clock: &mut crate::Clock) {
        for (key, value) in table.iter() {
            let slot = match key.get_ref().as_ref() {
                "time" => &mut clock.time,
                "date" => &mut clock.date,
                _ => {
                    self.unknown(key, "option", &["time", "date"]);
                    continue;
                }
            };
            match self.string(key, value) {
                Some(format) if crate::Clock::readable(format) && !format.trim().is_empty() => {
                    *slot = Some(format.to_owned());
                }
                Some(_) => {
                    self.wrong::<()>(key, value, r#"a strftime format like "%H:%M""#);
                }
                None => {}
            }
        }
    }

    fn panel(&mut self, table: &DeTable, panel: &mut crate::Panel) {
        for (key, value) in table.iter() {
            let (list, known, what): (&mut Vec<String>, &[&str], &str) =
                match key.get_ref().as_ref() {
                    "tiles" => (&mut panel.tiles, &crate::Panel::TILES, "tile"),
                    "sliders" => (&mut panel.sliders, &crate::Panel::SLIDERS, "slider"),
                    "parts" => (&mut panel.parts, &crate::Panel::PARTS, "part"),
                    "lyrics" => {
                        if let Some(v) = self.boolean(key, value) {
                            panel.lyrics = v
                        }
                        continue;
                    }
                    _ => {
                        self.unknown(key, "option", &["tiles", "sliders", "parts", "lyrics"]);
                        continue;
                    }
                };
            let Some(names) = self.strings(key, value) else {
                continue;
            };
            let mut kept = Vec::new();
            for name in names {
                // A plugin's tile, checked once [plugins] says which are on.
                if what == "tile" && name.contains('.') && !kept.contains(&name) {
                    self.plugin_tiles
                        .push((name.clone(), self.line(value.span())));
                    kept.push(name);
                } else if !known.contains(&name.as_str()) {
                    self.unknown_name(value, &name, what, known);
                } else if !kept.contains(&name) {
                    kept.push(name);
                }
            }
            *list = kept;
        }
    }

    fn colors(&mut self, table: &DeTable, colors: &mut crate::Colors) {
        for (key, value) in table.iter() {
            let Some(slot) = colors.get_mut(key.get_ref().as_ref()) else {
                self.unknown(key, "color", &crate::Colors::ROLES);
                continue;
            };
            match self.string(key, value).map(|text| Rgba::parse(text.trim())) {
                Some(Some(color)) => *slot = Some(color),
                Some(None) => {
                    self.wrong::<()>(key, value, r##"a color like "#1e1e2e""##);
                }
                None => {}
            }
        }
    }

    pub(crate) fn section<'a, 'i>(
        &mut self,
        key: &Key,
        value: &'a Value<'i>,
    ) -> Option<&'a DeTable<'i>> {
        match value.get_ref() {
            DeValue::Table(table) => Some(table),
            _ => self.wrong(key, value, "a section like [name]"),
        }
    }

    pub(crate) fn string<'a>(&mut self, key: &Key, value: &'a Value) -> Option<&'a str> {
        value
            .get_ref()
            .as_str()
            .or_else(|| self.wrong(key, value, "text in quotes"))
    }

    // Text with its ends trimmed; blank is none.
    pub(crate) fn text(&mut self, key: &Key, value: &Value) -> Option<String> {
        self.string(key, value)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }

    pub(crate) fn boolean(&mut self, key: &Key, value: &Value) -> Option<bool> {
        value
            .get_ref()
            .as_bool()
            .or_else(|| self.wrong(key, value, "true or false"))
    }

    pub(crate) fn number(
        &mut self,
        key: &Key,
        value: &Value,
        range: RangeInclusive<u16>,
    ) -> Option<u16> {
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
            if let Some(written) = key_combo(combo.get_ref()) {
                keys.push((written, command.to_owned()));
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

    pub(crate) fn strings(&mut self, key: &Key, value: &Value) -> Option<Vec<String>> {
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

    pub(crate) fn choice<T: Copy>(
        &mut self,
        key: &Key,
        value: &Value,
        options: &[(&str, T)],
    ) -> Option<T> {
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
        if let Some(opacity) = self.style_bar_opacity.filter(|_| !self.bar_opacity_set) {
            config.bar.opacity = opacity;
            for (_, bar) in &mut config.bar.per_screen {
                bar.opacity = opacity;
            }
        }
        let modules: Vec<String> = config
            .modules
            .iter()
            .filter_map(|m| m.name.clone())
            .collect();
        let enabled = &config.plugins.enabled;
        self.settle_pieces(&mut config.bar, &modules, enabled);
        for (_, bar) in &mut config.bar.per_screen {
            self.settle_pieces(bar, &modules, enabled);
        }
    }

    // One bar's pieces; a screen's own bar gets the same checks, each problem said once.
    fn settle_pieces(&mut self, bar: &mut crate::Bar, modules: &[String], enabled: &[String]) {
        let mut known: Vec<&str> = Piece::NAMED.iter().map(|(name, _)| *name).collect();
        known.extend(modules.iter().map(String::as_str));
        let mut seen: Vec<Piece> = Vec::new();
        for zone in [&mut bar.left, &mut bar.center, &mut bar.right] {
            let mut kept = Vec::new();
            for piece in std::mem::take(zone) {
                let line = self
                    .placed
                    .iter()
                    .find(|(name, _)| name == piece.name())
                    .map_or(0, |(_, line)| *line);
                // A plugin's piece stays even if it names nothing yet: it may be enabled later.
                let plugin = matches!(&piece, Piece::Module(name) if name.contains('.'));
                let missing =
                    !plugin && matches!(&piece, Piece::Module(name) if !modules.contains(name));
                let problem = if plugin && !seen.contains(&piece) {
                    seen.push(piece.clone());
                    kept.push(piece.clone());
                    self.plugin_part(piece.name(), line, enabled, Plugin::pieces, "bar piece")
                } else if missing {
                    let name = piece.name();
                    let hint = match suggest(name, &known) {
                        Some(close) => format!("did you mean “{close}”?"),
                        None => format!(
                            "known: {}, or the name of a [[module]]",
                            Piece::NAMED.map(|(name, _)| name).join(", ")
                        ),
                    };
                    Some(
                        Diagnostic::new(line, format!("nothing on the bar is called “{name}”"))
                            .hint(hint),
                    )
                } else if seen.contains(&piece) {
                    Some(Diagnostic::new(
                        line,
                        format!(
                            "“{}” is on the bar twice; it stays where it's first",
                            piece.name()
                        ),
                    ))
                } else {
                    seen.push(piece.clone());
                    kept.push(piece);
                    None
                };
                if let Some(problem) = problem.filter(|p| !self.diagnostics.contains(p)) {
                    self.diagnostics.push(problem);
                }
            }
            *zone = kept;
        }
    }

    // Widgets that name nothing, or that are there twice, come off the desktop.
    fn settle_widgets(&mut self, config: &mut Config) {
        let mut known: Vec<&str> = crate::Desktop::WIDGETS.to_vec();
        known.extend(config.modules.iter().filter_map(|m| m.name.as_deref()));
        let mut kept: Vec<String> = Vec::new();
        for name in &config.desktop.widgets {
            if name.contains('.') {
                let enabled = &config.plugins.enabled;
                let line = self.widgets_line;
                if let Some(problem) =
                    self.plugin_part(name, line, enabled, Plugin::pieces, "widget")
                {
                    self.diagnostics.push(problem);
                }
                if !kept.contains(name) {
                    kept.push(name.clone());
                }
            } else if !known.contains(&name.as_str()) {
                let hint = match suggest(name, &known) {
                    Some(close) => format!("did you mean “{close}”?"),
                    None => format!("known: {}, or the name of a [[module]]", known.join(", ")),
                };
                self.diagnostics.push(
                    Diagnostic::new(self.widgets_line, format!("no widget is called “{name}”"))
                        .hint(hint),
                );
            } else if !kept.contains(name) {
                kept.push(name.clone());
            }
        }
        config.desktop.widgets = kept;
    }

    pub(crate) fn unknown_name(&mut self, value: &Value, name: &str, what: &str, known: &[&str]) {
        let hint = match suggest(name, known) {
            Some(close) => format!("did you mean “{close}”?"),
            None => format!("known: {}", known.join(", ")),
        };
        self.diagnostics.push(
            Diagnostic::new(self.line(value.span()), format!("unknown {what} “{name}”")).hint(hint),
        );
    }

    pub(crate) fn wrong<T>(&mut self, key: &Key, value: &Value, expected: &str) -> Option<T> {
        let message = format!("“{}” should be {expected}", key.get_ref());
        self.diagnostics
            .push(Diagnostic::new(self.line(value.span()), message));
        None
    }

    pub(crate) fn unknown(&mut self, key: &Key, what: &str, known: &[&str]) {
        let name = key.get_ref();
        let hint = match suggest(name, known) {
            Some(close) => format!("did you mean “{close}”?"),
            None => format!("known: {}", known.join(", ")),
        };
        let diagnostic =
            Diagnostic::new(self.line(key.span()), format!("unknown {what} “{name}”")).hint(hint);
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn line(&self, span: Range<usize>) -> usize {
        let end = span.start.min(self.src.len());
        self.src.as_bytes()[..end]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1
    }
}

// Punctuation is spelled the way niri spells it, so Mod+/ reads as Mod+Slash.
fn key_combo(combo: &str) -> Option<String> {
    let (modifiers, key) = combo.rsplit_once('+').unwrap_or(("", combo));
    let key = match key {
        "/" => "Slash",
        "\\" => "Backslash",
        "," => "Comma",
        "." => "Period",
        ";" => "Semicolon",
        "'" => "Apostrophe",
        "[" => "BracketLeft",
        "]" => "BracketRight",
        "-" => "Minus",
        "=" => "Equal",
        "`" => "Grave",
        other => other,
    };
    let combo = if modifiers.is_empty() {
        key.to_owned()
    } else {
        format!("{modifiers}+{key}")
    };
    combo_is_valid(&combo).then_some(combo)
}

const BAR_OPTIONS: [&str; 20] = [
    "workspaces",
    "seconds",
    "date",
    "battery_percent",
    "position",
    "floating",
    "autohide",
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
    "screens",
    "skip",
];

fn combo_is_valid(combo: &str) -> bool {
    !combo.is_empty()
        && !combo.starts_with('+')
        && !combo.ends_with('+')
        && combo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '_')
}

pub(crate) fn suggest<'a>(input: &str, known: &[&'a str]) -> Option<&'a str> {
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

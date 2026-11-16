// A Hyprland config said again as a niri config.kdl, with a note for every line looked at.

use crate::conf::{self, Line};
use crate::era::{self, Era};
use crate::kdl::{Node, quote};
use crate::keys::{self, Action};
use crate::report::{Note, Outcome};
use crate::rules::{self, Rule, truthy};

pub struct Converted {
    pub kdl: String,
    pub notes: Vec<Note>,
    pub era: Era,
}

// What niri says about the screens it has, when it's running: exact modes for `mode`.
#[derive(Clone, Debug, Default)]
pub struct Screens {
    // Connector, then its modes as niri wants them: 2560x1440@164.999.
    pub modes: Vec<(String, Vec<String>)>,
}

#[derive(Default)]
struct Out {
    input: Vec<Node>,
    keyboard: Vec<Node>,
    xkb: Vec<Node>,
    touchpad: Vec<Node>,
    mouse: Vec<Node>,
    outputs: Vec<Node>,
    border: Vec<Node>,
    shadow: Vec<Node>,
    startup: Vec<Node>,
    environment: Vec<Node>,
    cursor: Vec<Node>,
    animations_off: bool,
    gaps_in: Option<u32>,
    gaps_out: Option<u32>,
    border_size: Option<u32>,
    rounding: Option<u32>,
    opacity: (Option<f64>, Option<f64>),
    blur: bool,
    rules: Vec<Node>,
    // Named workspaces rules open windows on; niri wants them declared.
    workspaces: Vec<String>,
    layer_rules: Vec<Node>,
    // Combo, then the whole bind line; the last one written wins, as in Hyprland.
    binds: Vec<(String, Node)>,
    notes: Vec<Note>,
}

mod look;
mod render;
mod rules_out;

pub fn convert(lines: &[Line], lua: bool, source: &str, screens: &Screens) -> Converted {
    let era = era::of(lines, lua);
    let mut out = Out::default();
    let mut submap = false;
    for line in lines.iter().filter(|l| !in_block(&l.key)) {
        let outcome = out.line(line, &mut submap, screens);
        if let Some(outcome) = outcome {
            out.note(line, outcome);
        }
    }
    for (at, said) in conf::blocks(lines, "monitorv2") {
        let outcome = out.monitor_block(&said, screens);
        out.notes.push(Note {
            at,
            line: "monitorv2 { … }".into(),
            outcome,
        });
    }
    for keyword in ["windowrule", "layerrule"] {
        for (at, rule) in rules::blocks(lines, keyword) {
            let outcome = if keyword == "windowrule" {
                out.window_rule(&rule)
            } else {
                out.layer_rule(&rule)
            };
            out.notes.push(Note {
                at,
                line: format!("{keyword} {{ … }}"),
                outcome,
            });
        }
    }
    for (key, what) in [
        ("Mod+Shift+Slash", "show-hotkey-overlay"),
        ("Mod+O", "toggle-overview"),
    ] {
        if !out.binds.iter().any(|(combo, _)| combo == key) {
            out.binds.push((
                key.to_owned(),
                Node::leaf(format!("{key} {{ {what}; }}"))
                    .noted("niri's own, on a key that was free"),
            ));
        }
    }
    let kdl = out.render(source, &era);
    Converted {
        kdl,
        notes: out.notes,
        era,
    }
}

// Read a block at a time, after the lines.
fn in_block(key: &str) -> bool {
    ["windowrule:", "layerrule:", "monitorv2:"]
        .iter()
        .any(|block| key.starts_with(block))
}

// Puts a node in, replacing one that sets the same thing: the later line wins, as in Hyprland.
fn put(list: &mut Vec<Node>, node: Node) {
    let name = |n: &Node| {
        n.head
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    let same = name(&node);
    list.retain(|had| name(had) != same);
    list.push(node);
}

fn here(why: &str) -> Option<Outcome> {
    Some(Outcome::Here(why.to_owned()))
}

fn left(why: &str) -> Option<Outcome> {
    Some(Outcome::Left(why.to_owned()))
}

fn moved(said: impl Into<String>) -> Option<Outcome> {
    Some(Outcome::Moved(said.into()))
}

impl Out {
    fn note(&mut self, line: &Line, outcome: Outcome) {
        self.notes.push(Note {
            at: line.at.clone(),
            line: format!("{} = {}", line.key, line.value),
            outcome,
        });
    }

    fn line(&mut self, line: &Line, submap: &mut bool, screens: &Screens) -> Option<Outcome> {
        let (key, value) = (line.key.as_str(), line.value.trim());
        if key == "submap" {
            *submap = value != "reset";
            return (*submap).then(|| Outcome::Left("niri has no key chords (submaps)".into()));
        }
        if let Some(flags) = key.strip_prefix("bind")
            && flags.chars().all(|c| c.is_ascii_lowercase())
        {
            if *submap {
                return left("it's in a submap, and niri has none");
            }
            return Some(self.bind(flags, value));
        }
        if key.starts_with("device:") {
            return left("niri sets the keyboard, mouse and touchpad as kinds, not one device");
        }
        match key {
            "monitor" => Some(self.monitor(value, screens)),
            "windowrule" | "windowrulev2" => {
                Some(rules::window(value).map_or_else(Outcome::Left, |r| self.window_rule(&r)))
            }
            "layerrule" => {
                Some(rules::layer(value).map_or_else(Outcome::Left, |r| self.layer_rule(&r)))
            }
            "blurls" => Some(self.layer_rule(&Rule {
                matches: vec![("namespace".into(), value.to_owned())],
                effects: vec![("blur".into(), "on".into())],
            })),
            "exec-once" | "execr-once" => Some(self.start(value, false)),
            "exec" | "execr" => Some(self.start(value, true)),
            "exec-shutdown" => left("niri runs nothing as it quits"),
            "env" | "envd" => Some(self.env(value)),
            "gesture" | "gesturep" => Some(gesture(value)),
            "workspace" => {
                left("niri's workspaces are made as they're used; name one in niri to keep it")
            }
            "permission" => left("niri asks nothing before a program reads the screen"),
            "source" => None,
            "lua:skipped" => left("this part of the config didn't run outside Hyprland"),
            "bezier" | "animations:bezier" => {
                left("niri's animations take curves of their own in animations { }")
            }
            "animation" | "animations:animation" => {
                left("niri's animations are set in animations { }, by what moves")
            }
            "animations:enabled" if !truthy(value) => {
                self.animations_off = true;
                moved("animations off")
            }
            "animations:enabled" => here("niri animates by default"),
            _ => self.setting(key, value),
        }
    }

    fn bind(&mut self, flags: &str, value: &str) -> Outcome {
        if flags.contains('m') {
            return Outcome::Here(
                "niri moves and resizes windows with Mod and the mouse already".into(),
            );
        }
        for (flag, why) in [
            ('o', "niri has no long presses"),
            ('c', "niri has no click binds"),
            ('g', "niri has no drag binds"),
            ('s', "niri binds no key sequences"),
        ] {
            if flags.contains(flag) {
                return Outcome::Left(why.into());
            }
        }
        let bind = match keys::parse(flags, value) {
            Ok(bind) => bind,
            Err(why) => return Outcome::Left(why),
        };
        let action = match keys::action(&bind.dispatcher, &bind.params) {
            Ok(Action::Exec(line)) if line.contains("hyprctl") => {
                return Outcome::Left("it talks to Hyprland, which isn't there under niri".into());
            }
            Ok(Action::Exec(line)) if line.is_empty() => {
                return Outcome::Left("it runs nothing".into());
            }
            Ok(Action::Exec(line)) => format!("spawn-sh {}", quote(&line)),
            Ok(Action::Niri(action)) => action,
            Err(why) => return Outcome::Left(why),
        };
        let mut head = bind.combo.clone();
        if flags.contains('l') || bind.key.starts_with("XF86") {
            head.push_str(" allow-when-locked=true");
        }
        if let Some(title) = &bind.description {
            head.push_str(&format!(" hotkey-overlay-title={}", quote(title)));
        }
        let said = format!("{} {{ {action}; }}", bind.combo);
        let node = Node::leaf(format!("{head} {{ {action}; }}"));
        let again = self
            .binds
            .iter()
            .position(|(combo, _)| *combo == bind.combo);
        if let Some(index) = again {
            self.binds.remove(index);
        }
        self.binds.push((bind.combo, node));
        Outcome::Moved(if flags.contains('r') {
            format!("{said}, on press: niri has no release binds")
        } else {
            said
        })
    }

    // monitor = NAME, RES[@HZ], POS, SCALE[, transform, N][, vrr, N][, mirror, NAME]…
    fn monitor(&mut self, value: &str, screens: &Screens) -> Outcome {
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        let name = parts.first().copied().unwrap_or("");
        if name.is_empty() {
            return Outcome::Here("niri gives every screen its preferred mode itself".into());
        }
        let name = name.strip_prefix("desc:").unwrap_or(name);
        let second = parts.get(1).copied().unwrap_or("preferred");
        if second == "addreserved" {
            return Outcome::Left("niri keeps no reserved area; panels take theirs".into());
        }
        if matches!(second, "disable" | "disabled") {
            self.outputs.push(Node::block(
                format!("output {}", quote(name)),
                vec![Node::leaf("off")],
            ));
            return Outcome::Moved(format!("{name} off"));
        }
        let pair = |key: &str| {
            parts
                .iter()
                .position(|p| *p == key)
                .and_then(|i| parts.get(i + 1))
                .copied()
        };
        self.output(
            name,
            Some(second),
            parts.get(2).copied(),
            parts.get(3).copied(),
            pair("transform"),
            pair("vrr"),
            pair("mirror").is_some(),
            screens,
        )
    }

    // monitorv2 { output = …; mode = …; position = …; scale = …; transform = …; vrr = … }
    fn monitor_block(&mut self, said: &[(String, String)], screens: &Screens) -> Outcome {
        let get = |key: &str| {
            said.iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let Some(name) = get("output") else {
            return Outcome::Left("this monitor block names no screen".into());
        };
        let name = name.strip_prefix("desc:").unwrap_or(name);
        if get("disabled").is_some_and(truthy) {
            self.outputs.push(Node::block(
                format!("output {}", quote(name)),
                vec![Node::leaf("off")],
            ));
            return Outcome::Moved(format!("{name} off"));
        }
        self.output(
            name,
            get("mode"),
            get("position"),
            get("scale"),
            get("transform"),
            get("vrr"),
            get("mirror").is_some(),
            screens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn output(
        &mut self,
        name: &str,
        mode: Option<&str>,
        position: Option<&str>,
        scale: Option<&str>,
        transform: Option<&str>,
        vrr: Option<&str>,
        mirror: bool,
        screens: &Screens,
    ) -> Outcome {
        let mut children = Vec::new();
        let mut said = vec![name.to_owned()];
        if let Some(mode) =
            mode.filter(|m| m.contains('x') && m.chars().next().is_some_and(|c| c.is_ascii_digit()))
        {
            let (size, rate) = mode.split_once('@').unwrap_or((mode, ""));
            match exact(screens, name, size, rate) {
                Some(exact) => children.push(Node::leaf(format!("mode {}", quote(&exact)))),
                None if rate.is_empty() => children.push(Node::leaf(format!("mode {}", quote(size)))),
                None => children.push(
                    Node::leaf(format!("mode {}", quote(size))).noted(format!(
                        "{mode} in Hyprland. niri wants the rate exactly as `niri msg outputs` lists it\n(like {size}@{rate}.000); without it niri takes the fastest"
                    )),
                ),
            }
            said.push(mode.to_owned());
        }
        if let Some((x, y)) = position
            .and_then(|p| p.split_once('x'))
            .and_then(|(x, y)| Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?)))
        {
            children.push(Node::leaf(format!("position x={x} y={y}")));
            said.push(format!("at {x},{y}"));
        }
        if let Some(scale) = scale
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|s| *s > 0.0)
        {
            children.push(Node::leaf(format!("scale {}", number(scale))));
            said.push(format!("scale {}", number(scale)));
        }
        const TRANSFORMS: [&str; 8] = [
            "normal",
            "90",
            "180",
            "270",
            "flipped",
            "flipped-90",
            "flipped-180",
            "flipped-270",
        ];
        if let Some(transform) = transform
            .and_then(|t| t.parse::<usize>().ok())
            .and_then(|t| TRANSFORMS.get(t))
            .filter(|t| **t != "normal")
        {
            children.push(Node::leaf(format!("transform {}", quote(transform))));
            said.push(format!("turned {transform}"));
        }
        match vrr.map(str::trim) {
            Some("1") => children.push(Node::leaf("variable-refresh-rate")),
            Some("2" | "3") => children.push(Node::leaf("variable-refresh-rate on-demand=true")),
            _ => {}
        }
        self.outputs
            .push(Node::block(format!("output {}", quote(name)), children));
        let said = said.join(", ");
        if mirror {
            Outcome::Moved(format!("{said}; niri mirrors no screens, so not that"))
        } else {
            Outcome::Moved(said)
        }
    }
}

// `niri msg outputs` read for each connector's modes:
//   Output "Some Company Monitor 1234" (DP-1)
//     Available modes:
//       2560x1440@164.999 (current, preferred)
pub fn screens(text: &str) -> Screens {
    let mut screens = Screens::default();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("Output ") {
            if let Some(name) = line.rsplit_once('(').and_then(|(_, n)| n.strip_suffix(')')) {
                screens.modes.push((name.to_owned(), Vec::new()));
            }
            continue;
        }
        let mode = line.split_whitespace().next().unwrap_or_default();
        let is_mode = mode.contains('@')
            && mode
                .split('x')
                .next()
                .is_some_and(|w| w.parse::<u32>().is_ok());
        if is_mode && let Some((_, modes)) = screens.modes.last_mut() {
            modes.push(mode.to_owned());
        }
    }
    screens
}

// The mode niri lists for this screen closest to what Hyprland asked for.
fn exact(screens: &Screens, name: &str, size: &str, rate: &str) -> Option<String> {
    let (_, modes) = screens.modes.iter().find(|(n, _)| n == name)?;
    let wanted: f64 = rate.parse().ok()?;
    modes
        .iter()
        .filter_map(|m| {
            let (s, r) = m.split_once('@')?;
            let off = (r.parse::<f64>().ok()? - wanted).abs();
            (s == size).then_some((m, off))
        })
        .filter(|(_, off)| *off < 1.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(m, _)| m.clone())
}

fn number(n: f64) -> String {
    let text = format!("{n:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_owned()
}

fn gesture(value: &str) -> Outcome {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    match parts.as_slice() {
        [_, "horizontal", "workspace", ..] | [_, "vertical", "workspace", ..] => {
            Outcome::Here("niri switches workspaces with a three-finger swipe itself".into())
        }
        _ => Outcome::Left("niri's touchpad gestures are its own".into()),
    }
}

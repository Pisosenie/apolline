use apolline_config::{Config, Display, KeyAction, Rule, Shortcut, Width};

use super::binds::{self, Becomes};
use super::conf::Line;

// Everything a Hyprland config says that has a place here, and a note for each line looked at.
#[derive(Debug, Default)]
pub struct Plan {
    pub keys: Vec<(String, KeyAction)>,
    pub shortcuts: Vec<(Shortcut, String)>,
    pub displays: Vec<Display>,
    pub rules: Vec<Rule>,
    pub autostart: Vec<String>,
    pub settings: Vec<Setting>,
    pub notes: Vec<Note>,
}

#[derive(Debug, PartialEq)]
pub enum Setting {
    Flag(&'static str, &'static str, bool),
    Number(&'static str, &'static str, i64),
    Fraction(&'static str, &'static str, f64),
    Text(&'static str, &'static str, String),
}

#[derive(Debug, PartialEq)]
pub struct Note {
    pub at: String,
    pub line: String,
    pub outcome: Outcome,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Moved(String),
    // Apolline or niri does this already.
    Here(String),
    Left(String),
}

impl Outcome {
    // Which part of the report it goes in, and what it says.
    pub fn group(&self) -> (usize, &str) {
        match self {
            Self::Moved(said) => (0, said),
            Self::Here(why) => (1, why),
            Self::Left(why) => (2, why),
        }
    }
}

// Started at login by Hyprland's users, done here by Apolline or by niri itself.
const STARTED_HERE: [&str; 20] = [
    "waybar",
    "swww",
    "swww-daemon",
    "hyprpaper",
    "mako",
    "dunst",
    "swaync",
    "hypridle",
    "swayidle",
    "nm-applet",
    "blueman-applet",
    "polkit",
    "hyprpolkitagent",
    "wl-paste",
    "cliphist",
    "dbus-update-activation-environment",
    "eww",
    "ags",
    "quickshell",
    "swayosd-server",
];

// The screens niri has now, so a mode written by hand lands on one they really have.
pub fn plan(
    lines: &[Line],
    niri_binds: &[(String, String)],
    config: &Config,
    outputs: &[apolline_niri::Output],
) -> Plan {
    let mut plan = Plan::default();
    let bound: Vec<(&Line, Becomes)> = lines
        .iter()
        .filter_map(|line| {
            let flags = line.key.strip_prefix("bind")?;
            flags
                .chars()
                .all(|c| c.is_ascii_lowercase())
                .then(|| (line, binds::bind(flags, &line.value)))
        })
        .collect();
    // Apolline's keys move first, so the keys they leave are free for yours.
    for (line, becomes) in &bound {
        if let Becomes::Shortcut(combo, shortcut) = becomes {
            let outcome = plan.shortcut(config, *shortcut, combo);
            plan.note(line, outcome);
        }
    }
    let shortcuts = plan.final_shortcuts(config);
    for (line, becomes) in bound {
        let outcome = match becomes {
            Becomes::Shortcut(..) => continue,
            Becomes::Key {
                combo,
                action,
                note,
            } => plan.key(config, niri_binds, &shortcuts, combo, action, note),
            Becomes::Covered(why) => Outcome::Here(why.to_owned()),
            Becomes::Left(why) => Outcome::Left(why),
        };
        plan.note(line, outcome);
    }
    for line in lines {
        let outcome = match line.key.as_str() {
            "monitor" => plan.display(config, &line.value, outputs),
            "windowrule" | "windowrulev2" => plan.rule(config, &line.value),
            "exec-once" | "exec" => plan.start(&line.value),
            key => match setting(key, &line.value) {
                Some(Ok(setting)) => {
                    let said = describe(&setting);
                    plan.settings.push(setting);
                    Outcome::Moved(said)
                }
                Some(Err(why)) => Outcome::Left(why),
                None => continue,
            },
        };
        plan.note(line, outcome);
    }
    plan
}

impl Plan {
    fn note(&mut self, line: &Line, outcome: Outcome) {
        self.notes.push(Note {
            at: line.at.clone(),
            line: format!("{} = {}", line.key, line.value),
            outcome,
        });
    }

    fn shortcut(&mut self, config: &Config, shortcut: Shortcut, combo: &str) -> Outcome {
        let title = crate::fragment::action(shortcut).0;
        let now = config.shortcuts.get(shortcut);
        if let Some((_, taken)) = self.shortcuts.iter().find(|(s, _)| *s == shortcut) {
            return Outcome::Left(format!("“{title}” comes over on {taken} already"));
        }
        if crate::keys::same(now, combo) {
            return Outcome::Here(format!("“{title}” is on this key here too"));
        }
        if now != Config::default().shortcuts.get(shortcut) {
            return Outcome::Left(format!("you put “{title}” on {now} yourself"));
        }
        self.shortcuts.push((shortcut, combo.to_owned()));
        Outcome::Moved(format!("Apolline's “{title}” moves to {combo}"))
    }

    fn final_shortcuts(&self, config: &Config) -> Vec<(Shortcut, String)> {
        Shortcut::ALL
            .into_iter()
            .map(|shortcut| {
                let moved = self.shortcuts.iter().find(|(s, _)| *s == shortcut);
                let combo = moved.map_or(config.shortcuts.get(shortcut), |(_, c)| c.as_str());
                (shortcut, combo.to_owned())
            })
            .collect()
    }

    fn key(
        &mut self,
        config: &Config,
        niri_binds: &[(String, String)],
        shortcuts: &[(Shortcut, String)],
        combo: String,
        action: KeyAction,
        note: Option<String>,
    ) -> Outcome {
        if let Some((shortcut, _)) = shortcuts
            .iter()
            .find(|(_, own)| crate::keys::same(own, &combo))
        {
            let title = crate::fragment::action(*shortcut).0;
            return Outcome::Left(format!(
                "Apolline's “{title}” is on {combo}; Settings → Shortcuts moves it"
            ));
        }
        if config
            .keys
            .iter()
            .any(|(own, _)| crate::keys::same(own, &combo))
        {
            return Outcome::Left(format!("[keys] has {combo} already"));
        }
        if let KeyAction::Niri(niri) = &action
            && niri_binds.iter().any(|(their, does)| {
                crate::keys::same(their, &combo) && does.replace('"', "") == *niri
            })
        {
            return Outcome::Here("niri's config does the same".into());
        }
        let said = match &action {
            KeyAction::Run(command) => format!("{combo} runs {command}"),
            KeyAction::Niri(niri) => format!("{combo} does {niri}"),
        };
        self.keys.retain(|(had, _)| !crate::keys::same(had, &combo));
        self.keys.push((combo, action));
        Outcome::Moved(match note {
            Some(note) => format!("{said}; {note}"),
            None => said,
        })
    }

    // monitor = NAME, RES[@HZ], POS, SCALE[, transform, N][, vrr, N]
    fn display(
        &mut self,
        config: &Config,
        value: &str,
        outputs: &[apolline_niri::Output],
    ) -> Outcome {
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        let name = parts.first().copied().unwrap_or("");
        if name.is_empty() {
            return Outcome::Here("niri gives every screen its best mode itself".into());
        }
        if name.starts_with("desc:") {
            return Outcome::Left(
                "screens by description don't come over; name it like DP-1".into(),
            );
        }
        if config.displays.iter().any(|d| d.name == name) {
            return Outcome::Left(format!("Apolline has {name} set up already"));
        }
        let resolution = parts.get(1).copied().unwrap_or("preferred");
        if resolution == "disable" {
            return Outcome::Left("a screen that's off stays off from Settings → Displays".into());
        }
        let mode = resolution
            .split_once('x')
            .filter(|(w, _)| w.chars().all(|c| c.is_ascii_digit()))
            .map(|_| {
                crate::arrange::exact_mode(outputs, name, resolution)
                    .unwrap_or_else(|| resolution.to_owned())
            });
        let position = parts.get(2).and_then(|pos| {
            let (x, y) = pos.split_once('x')?;
            Some((x.parse().ok()?, y.parse().ok()?))
        });
        let scale = parts
            .get(3)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|s| (0.5..=4.0).contains(s));
        let extra = |name: &str| {
            parts
                .iter()
                .position(|p| *p == name)
                .and_then(|i| parts.get(i + 1))
                .copied()
        };
        let transform = extra("transform")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| apolline_config::TRANSFORMS.get(n))
            .map(|t| (*t).to_owned());
        let vrr = extra("vrr").map(|n| n != "0");
        let display = Display {
            name: name.to_owned(),
            mode,
            scale,
            transform,
            position,
            vrr,
        };
        let said = format!(
            "{name}{}{}{}",
            display
                .mode
                .as_ref()
                .map_or(String::new(), |m| format!(" at {m}")),
            display
                .scale
                .map_or(String::new(), |s| format!(", scale {s}")),
            display
                .position
                .map_or(String::new(), |(x, y)| format!(", at {x},{y}")),
        );
        self.displays.push(display);
        Outcome::Moved(said)
    }

    // windowrulev2 = float, class:^(pavucontrol)$ ; windowrule = float, ^(pavucontrol)$
    fn rule(&mut self, config: &Config, value: &str) -> Outcome {
        let Some((effect, matched)) = value.split_once(',') else {
            return Outcome::Left("this rule has no window to match".into());
        };
        let class = matched.split(',').map(str::trim).find_map(|m| {
            m.strip_prefix("class:")
                .or_else(|| m.strip_prefix("match:class "))
                .or_else(|| (!m.contains(':')).then_some(m))
        });
        let Some(apps) = class.and_then(app_names) else {
            return Outcome::Left("only rules matching an app by its plain name come over".into());
        };
        let mut words = effect.split_whitespace();
        let mut rule = Rule {
            app: String::new(),
            floating: None,
            width: None,
            opacity: None,
        };
        match words.next().unwrap_or_default() {
            "float" => rule.floating = Some(true),
            "tile" => rule.floating = Some(false),
            "opacity" => match words.next().and_then(|o| o.parse::<f32>().ok()) {
                Some(opacity) if (0.1..=1.0).contains(&opacity) => rule.opacity = Some(opacity),
                _ => return Outcome::Left("this opacity has no match here".into()),
            },
            "size" => match words.next().and_then(width) {
                Some(width) => rule.width = Some(width),
                None => {
                    return Outcome::Left(
                        "niri sizes windows by column width: a third, half, two thirds…".into(),
                    );
                }
            },
            other => return Outcome::Left(format!("“{other}” has no window rule here")),
        }
        let mut said = Vec::new();
        for app in apps {
            if config.rules.iter().any(|r| r.app == app) {
                said.push(format!("{app} has a rule here already"));
                continue;
            }
            let merged = match self.rules.iter_mut().find(|r| r.app == app) {
                Some(had) => had,
                None => {
                    self.rules.push(Rule {
                        app: app.clone(),
                        ..rule.clone()
                    });
                    said.push(format!("a rule for {app}"));
                    continue;
                }
            };
            merged.floating = rule.floating.or(merged.floating);
            merged.width = rule.width.or(merged.width);
            merged.opacity = rule.opacity.or(merged.opacity);
            said.push(format!("into the rule for {app}"));
        }
        Outcome::Moved(said.join(", "))
    }

    // `waybar & hyprpaper & firefox` is three programs: firefox comes over, the rest are here.
    fn start(&mut self, line: &str) -> Outcome {
        let mut started = Vec::new();
        let mut hyprland = false;
        for command in line
            .split("&&")
            .flat_map(|part| part.split([';', '&']))
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            let program = command
                .split_whitespace()
                .next()
                .map(|w| w.rsplit('/').next().unwrap_or(w))
                .unwrap_or_default();
            if STARTED_HERE.iter().any(|p| program.starts_with(p)) {
                continue;
            }
            if program == "hyprctl" || program == "systemctl" || command.contains("hypr") {
                hyprland = true;
                continue;
            }
            if !self.autostart.iter().any(|c| c == command) {
                self.autostart.push(command.to_owned());
            }
            started.push(command);
        }
        match (started.is_empty(), hyprland) {
            (false, _) => Outcome::Moved(format!("starts with niri: {}", started.join(", "))),
            (true, true) => Outcome::Left("it's for Hyprland's own session".into()),
            (true, false) => Outcome::Here("Apolline or niri starts what this did".into()),
        }
    }
}

// ^(pavucontrol|org\.kde\.dolphin)$ → the plain names in it; None if it's more of a pattern.
fn app_names(pattern: &str) -> Option<Vec<String>> {
    let inner = pattern
        .trim()
        .trim_start_matches('^')
        .trim_end_matches('$')
        .trim_start_matches('(')
        .trim_end_matches(')');
    let names: Vec<String> = inner
        .split('|')
        .map(|name| name.replace("\\.", "."))
        .collect();
    names
        .iter()
        .all(|n| {
            !n.is_empty()
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        })
        .then_some(names)
}

fn width(size: &str) -> Option<Width> {
    let percent: f32 = size.strip_suffix('%')?.parse().ok()?;
    [
        (33.0, Width::Third),
        (50.0, Width::Half),
        (66.0, Width::TwoThirds),
        (75.0, Width::ThreeQuarters),
        (100.0, Width::Full),
    ]
    .into_iter()
    .find(|(at, _)| (percent - at).abs() <= 1.5)
    .map(|(_, width)| width)
}

// The settings that have a place in Apolline's config; None for the rest, which stay quietly.
fn setting(key: &str, value: &str) -> Option<Result<Setting, String>> {
    let number = || value.split([',', ' ']).next()?.trim().parse::<i64>().ok();
    let flag = || match value.trim() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    };
    let unreadable = || Err(format!("“{value}” didn't read"));
    let ranged = |section, name, low: i64, high: i64, n: Option<i64>| {
        n.map_or_else(unreadable, |n| {
            Ok(Setting::Number(section, name, n.clamp(low, high)))
        })
    };
    Some(match key {
        "input:repeat_rate" => ranged("keyboard", "repeat_rate", 5, 80, number()),
        "input:repeat_delay" => ranged("keyboard", "repeat_delay", 150, 2000, number()),
        "input:follow_mouse" => number().map_or_else(unreadable, |n| {
            Ok(Setting::Flag("windows", "focus_follows_mouse", n > 0))
        }),
        "input:natural_scroll" => flag().map_or_else(unreadable, |on| {
            Ok(Setting::Flag("mouse", "natural_scroll", on))
        }),
        "input:sensitivity" => value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|s| (-1.0..=1.0).contains(s))
            .map_or_else(unreadable, |s| Ok(Setting::Fraction("mouse", "speed", s))),
        "input:touchpad:natural_scroll" => flag().map_or_else(unreadable, |on| {
            Ok(Setting::Flag("touchpad", "natural_scroll", on))
        }),
        "input:touchpad:tap-to-click" => {
            flag().map_or_else(unreadable, |on| Ok(Setting::Flag("touchpad", "tap", on)))
        }
        "input:touchpad:disable_while_typing" => flag().map_or_else(unreadable, |on| {
            Ok(Setting::Flag("touchpad", "disable_while_typing", on))
        }),
        "general:gaps_in" => ranged("layout", "gaps", 0, 64, number().map(|n| n * 2)),
        "general:border_size" => match number() {
            Some(0) => Ok(Setting::Flag("layout", "borders", false)),
            n => ranged("layout", "border_width", 1, 8, n),
        },
        "decoration:rounding" => ranged("layout", "radius", 0, 32, number()),
        "decoration:shadow:enabled" | "decoration:drop_shadow" => {
            flag().map_or_else(unreadable, |on| Ok(Setting::Flag("layout", "shadows", on)))
        }
        "general:col.active_border" => color(value).map_or_else(unreadable, |hex| {
            Ok(Setting::Text("desktop", "accent", hex))
        }),
        _ => return None,
    })
}

// rgba(33ccffee), rgb(33ccff) or 0xee33ccff, the first of a gradient: as #33ccff.
fn color(value: &str) -> Option<String> {
    let first = value.split_whitespace().next()?;
    let hex = if let Some(inner) = first
        .strip_prefix("rgba(")
        .or_else(|| first.strip_prefix("rgb("))
    {
        inner.trim_end_matches(')').get(..6)?.to_owned()
    } else {
        first.strip_prefix("0x")?.get(2..8)?.to_owned()
    };
    hex.chars()
        .all(|c| c.is_ascii_hexdigit())
        .then(|| format!("#{}", hex.to_lowercase()))
}

fn describe(setting: &Setting) -> String {
    match setting {
        Setting::Flag(section, key, on) => format!("[{section}] {key} = {on}"),
        Setting::Number(section, key, n) => format!("[{section}] {key} = {n}"),
        Setting::Fraction(section, key, n) => format!("[{section}] {key} = {n}"),
        Setting::Text(section, key, text) => format!("[{section}] {key} = \"{text}\""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(key: &str, value: &str) -> Line {
        Line {
            key: key.into(),
            value: value.into(),
            at: "hyprland.conf:1".into(),
        }
    }

    #[test]
    fn a_typical_config_comes_over_with_a_note_for_each_line() {
        let lines = [
            line("monitor", "DP-1, 2560x1440@165, 0x0, 1.25"),
            line("monitor", "HDMI-A-1, preferred, auto, 1, transform, 1"),
            line("monitor", ", preferred, auto, 1"),
            line("bind", "SUPER, R, exec, rofi -show drun"),
            line("bind", "SUPER, Space, exec, kitty"),
            line("bind", "SUPER, Q, exec, kitty"),
            line("bind", "SUPER, C, killactive"),
            line("bind", "SUPER, 1, workspace, 1"),
            line("bind", "SUPER, J, togglesplit"),
            line(
                "windowrulev2",
                "float, class:^(pavucontrol|org\\.gnome\\.Calculator)$",
            ),
            line("windowrulev2", "opacity 0.9 0.8, class:^(pavucontrol)$"),
            line("windowrulev2", "workspace 2, class:^(firefox)$"),
            line("exec-once", "waybar & hyprpaper & firefox"),
            line("exec-once", "fcitx5 -d"),
            line("input:touchpad:natural_scroll", "true"),
            line(
                "general:col.active_border",
                "rgba(33ccffee) rgba(00ff99ee) 45deg",
            ),
            line("misc:disable_hyprland_logo", "true"),
        ];
        let niri = [("Mod+1".to_owned(), "focus-workspace 1".to_owned())];
        let plan = plan(&lines, &niri, &Config::default(), &[]);
        assert_eq!(plan.shortcuts, [(Shortcut::Launcher, "Mod+R".to_owned())]);
        assert_eq!(
            plan.keys,
            [
                ("Mod+Space".to_owned(), KeyAction::Run("kitty".into())),
                ("Mod+Q".to_owned(), KeyAction::Run("kitty".into())),
                ("Mod+C".to_owned(), KeyAction::Niri("close-window".into())),
            ]
        );
        assert_eq!(plan.displays.len(), 2);
        assert_eq!(plan.displays[0].mode.as_deref(), Some("2560x1440@165"));
        assert_eq!(plan.displays[0].scale, Some(1.25));
        assert_eq!(plan.displays[0].position, Some((0, 0)));
        assert_eq!(plan.displays[1].transform.as_deref(), Some("90"));
        assert_eq!(plan.rules.len(), 2);
        assert_eq!(plan.rules[0].app, "pavucontrol");
        assert_eq!(plan.rules[0].floating, Some(true));
        assert_eq!(plan.rules[0].opacity, Some(0.9));
        assert_eq!(plan.rules[1].app, "org.gnome.Calculator");
        assert_eq!(plan.autostart, ["firefox", "fcitx5 -d"]);
        assert!(
            plan.settings
                .contains(&Setting::Flag("touchpad", "natural_scroll", true))
        );
        assert!(
            plan.settings
                .contains(&Setting::Text("desktop", "accent", "#33ccff".into()))
        );
        // Every line looked at has its note; the logo setting stays quietly.
        assert_eq!(plan.notes.len(), lines.len() - 1);
        let outcome = |at: usize| {
            let wanted = format!("{} = {}", lines[at].key, lines[at].value);
            plan.notes
                .iter()
                .find(|n| n.line == wanted)
                .map(|n| &n.outcome)
        };
        let left = |at: usize| matches!(outcome(at), Some(Outcome::Left(_)));
        let here = |at: usize| matches!(outcome(at), Some(Outcome::Here(_)));
        assert!(here(7), "niri has Mod+1 already");
        assert!(left(8) && left(11));
        assert!(here(2));
    }

    #[test]
    fn keys_apolline_holds_are_left_and_yours_are_kept() {
        let (config, _) =
            Config::parse("[keys]\n\"Mod+C\" = \"mine\"\n[shortcuts]\nlauncher = \"Mod+D\"\n");
        let lines = [
            line("bind", "SUPER, C, killactive"),
            line("bind", "SUPER, A, exec, foot"),
            line("bind", "SUPER, R, exec, wofi --show drun"),
        ];
        let plan = plan(&lines, &[], &config, &[]);
        assert!(
            plan.keys.is_empty() && plan.shortcuts.is_empty(),
            "{plan:?}"
        );
        assert!(
            plan.notes
                .iter()
                .all(|n| matches!(n.outcome, Outcome::Left(_)))
        );
    }

    #[test]
    fn colors_and_widths_read_as_written() {
        assert_eq!(color("rgb(AABBCC)").as_deref(), Some("#aabbcc"));
        assert_eq!(color("0xee33ccff").as_deref(), Some("#33ccff"));
        assert_eq!(color("red"), None);
        assert_eq!(width("50%"), Some(Width::Half));
        assert_eq!(width("800"), None);
    }
}

// Which Hyprland a config was written for, told by the syntax it uses.

use crate::conf::Line;

// The oldest and newest Hyprland that read every line as written; None is open-ended.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Era {
    pub from: Option<(u32, u32)>,
    pub until: Option<(u32, u32)>,
}

impl Era {
    fn since(&mut self, version: (u32, u32)) {
        self.from = Some(self.from.map_or(version, |from| from.max(version)));
    }

    fn up_to(&mut self, version: (u32, u32)) {
        self.until = Some(self.until.map_or(version, |until| until.min(version)));
    }

    // Lines written for Hyprlands that never were the same one: v1 rules beside 0.53 ones, say.
    pub fn mixed(&self) -> bool {
        matches!((self.from, self.until), (Some(from), Some(until)) if from > until)
    }

    pub fn said(&self) -> String {
        let v = |(major, minor): (u32, u32)| format!("{major}.{minor}");
        match (self.from, self.until) {
            _ if self.mixed() => "a mix of Hyprland versions".into(),
            (Some(from), Some(until)) if from == until => format!("Hyprland {}", v(from)),
            (Some(from), Some(until)) => format!("Hyprland {}–{}", v(from), v(until)),
            (Some(from), None) => format!("Hyprland {} or newer", v(from)),
            (None, Some(until)) => format!("Hyprland {} or older", v(until)),
            (None, None) => "any Hyprland".into(),
        }
    }
}

pub fn of(lines: &[Line], lua: bool) -> Era {
    let mut era = Era::default();
    // A Lua config is 0.55 or newer whatever it calls; its lines are the sandbox's wording.
    if lua {
        era.since((0, 55));
        return era;
    }
    for line in lines {
        let key = line.key.as_str();
        let value = line.value.as_str();
        match key {
            "windowrule" | "layerrule" if value.contains("match:") => era.since((0, 53)),
            "windowrulev2" => era.up_to((0, 52)),
            "layerrule" => era.up_to((0, 52)),
            "blurls" => era.up_to((0, 52)),
            "windowrule" => match crate::rules::window(value) {
                Ok(rule) if v1(value, &rule) => era.up_to((0, 47)),
                // title: reads the same in v1 and v2; class: and the rest only in v2.
                Ok(rule) if rule.matches.iter().all(|(prop, _)| prop == "title") => {}
                _ => era.since((0, 48)),
            },
            "gesture" | "gesturep" => era.since((0, 51)),
            "gestures:workspace_swipe" | "gestures:workspace_swipe_fingers" => era.up_to((0, 50)),
            "decoration:drop_shadow" | "decoration:shadow_range" | "decoration:col.shadow" => {
                era.up_to((0, 44))
            }
            "decoration:rounding_power" => era.since((0, 47)),
            "decoration:shadow:ignore_window" => {
                era.since((0, 45));
                era.up_to((0, 54));
            }
            _ if key.starts_with("decoration:shadow:") => era.since((0, 45)),
            "exec-shutdown" => era.since((0, 43)),
            "execr-once" => era.since((0, 47)),
            "permission" => era.since((0, 49)),
            "device:name" => era.since((0, 36)),
            _ if key.starts_with("windowrule:") || key.starts_with("layerrule:") => {
                era.since((0, 53))
            }
            _ if key.starts_with("monitorv2:") => era.since((0, 50)),
            _ if key.starts_with("cursor:") => era.since((0, 41)),
            _ if key
                .strip_prefix("bind")
                .is_some_and(|flags| flags.contains('d')) =>
            {
                era.since((0, 41))
            }
            // device:NAME { } until 0.35, device { name = NAME } after.
            _ if key.starts_with("device:") && key.matches(':').count() >= 2 => era.up_to((0, 35)),
            _ => {}
        }
    }
    era
}

// v1 named the class by a bare pattern: nothing in the line says which prop it is.
fn v1(value: &str, rule: &crate::rules::Rule) -> bool {
    !value.contains("match:")
        && rule.matches.len() == 1
        && rule.matches[0].0 == "class"
        && !value.contains("class:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(pairs: &[(&str, &str)]) -> Vec<Line> {
        pairs
            .iter()
            .map(|(key, value)| Line {
                key: (*key).into(),
                value: (*value).into(),
                ..Line::default()
            })
            .collect()
    }

    #[test]
    fn the_syntax_tells_which_hyprland_it_was_for() {
        let old = of(
            &lines(&[
                ("windowrule", "float, ^(kitty)$"),
                ("decoration:drop_shadow", "true"),
            ]),
            false,
        );
        assert_eq!(old.said(), "Hyprland 0.44 or older");
        let middle = of(
            &lines(&[
                ("windowrulev2", "float, class:kitty"),
                ("gesture", "3, horizontal, workspace"),
            ]),
            false,
        );
        assert_eq!(middle.said(), "Hyprland 0.51–0.52");
        let new = of(
            &lines(&[("windowrule", "match:class kitty, float on")]),
            false,
        );
        assert_eq!(new.said(), "Hyprland 0.53 or newer");
        assert_eq!(of(&[], true).said(), "Hyprland 0.55 or newer");
        let mixed = of(
            &lines(&[
                ("windowrule", "float, ^(kitty)$"),
                ("windowrule", "match:class kitty, float on"),
            ]),
            false,
        );
        assert!(mixed.mixed());
    }
}

// Window and layer rules as niri's window-rule and layer-rule.

use super::Out;
use crate::kdl::{Node, quote};
use crate::report::Outcome;
use crate::rules::{Rule, truthy};

impl Out {
    pub(super) fn window_rule(&mut self, rule: &Rule) -> Outcome {
        let mut matching = Vec::new();
        for (prop, value) in &rule.matches {
            let field = match prop.as_str() {
                "class" | "initial_class" => "app-id",
                "title" | "initial_title" => "title",
                "float" => {
                    matching.push(Node::leaf(format!("match is-floating={}", truthy(value))));
                    continue;
                }
                other => {
                    return Outcome::Left(format!("niri can't tell windows apart by {other}"));
                }
            };
            matching.push(matched(field, value));
        }
        let mut children = matching;
        let mut took = Vec::new();
        let mut not = Vec::new();
        for (effect, value) in &rule.effects {
            match self.window_effect(effect, value) {
                Ok((node, said)) => {
                    children.extend(node);
                    took.push(said);
                }
                Err(why) => not.push(format!("{effect}: {why}")),
            }
        }
        if took.is_empty() {
            return Outcome::Left(not.join("; "));
        }
        self.rules.push(Node::block("window-rule", children));
        let mut said = took.join(", ");
        if !not.is_empty() {
            said.push_str(&format!(" (not {})", not.join("; ")));
        }
        Outcome::Moved(said)
    }

    fn window_effect(&mut self, effect: &str, value: &str) -> Result<(Vec<Node>, String), String> {
        let on = truthy(value);
        let leaf = |head: String, said: &str| Ok((vec![Node::leaf(head)], said.to_owned()));
        match effect {
            "float" => leaf(
                format!("open-floating {on}"),
                if on { "floats" } else { "tiles" },
            ),
            "tile" => leaf(format!("open-floating {}", !on), "tiles"),
            "maximize" => leaf(format!("open-maximized {on}"), "opens maximized"),
            "fullscreen" => leaf(format!("open-fullscreen {on}"), "opens fullscreen"),
            "no_initial_focus" => leaf(format!("open-focused {}", !on), "opens unfocused"),
            "center" => Err("niri centers new floating windows itself".into()),
            "opacity" => {
                let first = value
                    .split_whitespace()
                    .find_map(|w| w.parse::<f64>().ok())
                    .ok_or("this opacity didn't read")?;
                leaf(
                    format!("opacity {}", super::number(first.clamp(0.0, 1.0))),
                    &format!("opacity {}", super::number(first)),
                )
            }
            "size" => size(value),
            "move" => {
                let mut xy = value.split_whitespace().map(|n| n.parse::<i64>());
                match (xy.next(), xy.next()) {
                    (Some(Ok(x)), Some(Ok(y))) => leaf(
                        format!("default-floating-position x={x} y={y} relative-to=\"top-left\""),
                        "placed where it was",
                    ),
                    _ => Err("niri places floating windows by numbers only".into()),
                }
            }
            "monitor" => leaf(
                format!("open-on-output {}", quote(value.trim())),
                &format!("opens on {}", value.trim()),
            ),
            "workspace" => {
                let name = value.split_whitespace().next().unwrap_or_default();
                match name.strip_prefix("name:") {
                    Some(name) => {
                        if !self.workspaces.iter().any(|w| w == name) {
                            self.workspaces.push(name.to_owned());
                        }
                        leaf(
                            format!("open-on-workspace {}", quote(name)),
                            &format!("opens on workspace {name}"),
                        )
                    }
                    None => Err("niri opens windows on named workspaces only".into()),
                }
            }
            "rounding" => {
                let radius: u32 = value.trim().parse().map_err(|_| "this didn't read")?;
                Ok((
                    vec![
                        Node::leaf(format!("geometry-corner-radius {radius}")),
                        Node::leaf("clip-to-geometry true"),
                    ],
                    format!("corners {radius} px"),
                ))
            }
            "no_rounding" if on => Ok((
                vec![Node::leaf("geometry-corner-radius 0")],
                "square corners".into(),
            )),
            "no_border" if on => Ok((
                vec![Node::block("border", vec![Node::leaf("off")])],
                "no border".into(),
            )),
            "border_size" if value.trim() == "0" => Ok((
                vec![Node::block("border", vec![Node::leaf("off")])],
                "no border".into(),
            )),
            "no_shadow" if on => Ok((
                vec![Node::block("shadow", vec![Node::leaf("off")])],
                "no shadow".into(),
            )),
            "no_blur" if on => Ok((
                vec![Node::block(
                    "background-effect",
                    vec![Node::leaf("blur false")],
                )],
                "no blur".into(),
            )),
            "no_vrr" if on => leaf("variable-refresh-rate false".into(), "no VRR"),
            "no_screen_share" if on => leaf(
                "block-out-from \"screencast\"".into(),
                "hidden from screencasts",
            ),
            "idle_inhibit" => Err("niri keeps the screen on only when the app asks".into()),
            "pin" => Err("niri has no windows pinned over every workspace".into()),
            "pseudo" => Err("niri has no pseudotiling".into()),
            "no_anim" => Err("niri animates every window alike".into()),
            "no_focus" | "stay_focused" => Err("niri focuses windows as it sees fit".into()),
            _ => Err("Hyprland's own".into()),
        }
    }

    pub(super) fn layer_rule(&mut self, rule: &Rule) -> Outcome {
        let mut children = Vec::new();
        for (prop, value) in &rule.matches {
            if prop != "namespace" {
                return Outcome::Left(format!("niri can't tell layers apart by {prop}"));
            }
            children.push(matched("namespace", value));
        }
        let mut took = Vec::new();
        let mut not = Vec::new();
        for (effect, value) in &rule.effects {
            let on = truthy(value);
            match effect.as_str() {
                "blur" if on => {
                    children.push(Node::block(
                        "background-effect",
                        vec![Node::leaf("blur true")],
                    ));
                    took.push("blurred behind (niri 26.04 or newer)");
                }
                "xray" if on => {
                    children.push(Node::block(
                        "background-effect",
                        vec![Node::leaf("xray true")],
                    ));
                    took.push("sees through to the wallpaper");
                }
                "no_screen_share" if on => {
                    children.push(Node::leaf("block-out-from \"screencast\""));
                    took.push("hidden from screencasts");
                }
                "ignore_alpha" | "ignorezero" => {
                    took.push("blurred by its shape, as niri does");
                }
                other => not.push(format!("{other}: Hyprland's own")),
            }
        }
        if children.iter().all(|c| c.head.starts_with("match")) {
            return Outcome::Left(if !not.is_empty() {
                not.join("; ")
            } else {
                "nothing in it has a place in niri".to_owned()
            });
        }
        self.layer_rules.push(Node::block("layer-rule", children));
        Outcome::Moved(took.join(", "))
    }
}

// Hyprland matches the whole name (since 0.46), niri anywhere in it: anchored, they agree.
// `negative:` in Hyprland is niri's exclude.
fn matched(field: &str, pattern: &str) -> Node {
    let (keyword, pattern) = match pattern.strip_prefix("negative:") {
        Some(rest) => ("exclude", rest),
        None => ("match", pattern),
    };
    let anchored = if pattern.starts_with('^') && pattern.ends_with('$') {
        pattern.to_owned()
    } else {
        format!("^(?:{pattern})$")
    };
    Node::leaf(format!("{keyword} {field}={}", quote(&anchored)))
}

// size W H: pixels, or a share of the screen as N%.
fn size(value: &str) -> Result<(Vec<Node>, String), String> {
    let mut words = value.split_whitespace();
    let (width, height) = (words.next(), words.next());
    let measure = |text: Option<&str>| -> Option<String> {
        let text = text?;
        match text.strip_suffix('%') {
            Some(percent) => {
                let share = percent.parse::<f64>().ok()? / 100.0;
                Some(format!("proportion {}", super::number(share)))
            }
            None => Some(format!("fixed {}", text.parse::<u32>().ok()?)),
        }
    };
    let mut nodes = Vec::new();
    if let Some(width) = measure(width) {
        nodes.push(Node::block("default-column-width", vec![Node::leaf(width)]));
    }
    if let Some(height) = measure(height) {
        nodes.push(Node::block(
            "default-window-height",
            vec![Node::leaf(height)],
        ));
    }
    if nodes.is_empty() {
        return Err("niri sizes windows by pixels or a share of the screen".into());
    }
    Ok((nodes, format!("sized {value}")))
}

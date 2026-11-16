// Settings: keyboard, pointer, gaps, borders, corners, shadows, cursor, environment, startup.

use super::{Out, here, left, moved, put};
use crate::kdl::{Node, quote};
use crate::report::Outcome;
use crate::rules::truthy;

impl Out {
    pub(super) fn setting(&mut self, key: &str, value: &str) -> Option<Outcome> {
        let int = || value.split([',', ' ']).next()?.trim().parse::<i64>().ok();
        let float = || value.trim().parse::<f64>().ok();
        let section = key.split(':').next().unwrap_or_default();
        match key {
            "input:kb_layout" => self.xkb("layout", value),
            "input:kb_variant" => self.xkb("variant", value),
            "input:kb_options" => self.xkb("options", value),
            "input:kb_model" => self.xkb("model", value),
            "input:kb_rules" => self.xkb("rules", value),
            "input:repeat_rate" => self.number(Part::Keyboard, "repeat-rate", int()),
            "input:repeat_delay" => self.number(Part::Keyboard, "repeat-delay", int()),
            "input:numlock_by_default" => self.toggle(Part::Keyboard, "numlock", value),
            "input:sensitivity" => match float() {
                Some(speed) => {
                    let speed = super::number(speed.clamp(-1.0, 1.0));
                    put(&mut self.mouse, Node::leaf(format!("accel-speed {speed}")));
                    put(
                        &mut self.touchpad,
                        Node::leaf(format!("accel-speed {speed}")),
                    );
                    moved(format!("pointer speed {speed}"))
                }
                None => left("this speed didn't read"),
            },
            "input:accel_profile" if !matches!(value, "flat" | "adaptive") => {
                left("niri knows flat and adaptive acceleration")
            }
            "input:accel_profile" => {
                put(
                    &mut self.mouse,
                    Node::leaf(format!("accel-profile {}", quote(value))),
                );
                moved(format!("{value} acceleration"))
            }
            "input:natural_scroll" => self.toggle(Part::Mouse, "natural-scroll", value),
            "input:left_handed" => self.toggle(Part::Mouse, "left-handed", value),
            "input:scroll_factor" => self.fraction(Part::Mouse, "scroll-factor", float()),
            "input:follow_mouse" => match int() {
                Some(0) => here("niri doesn't move the focus with the pointer unless told"),
                Some(_) => {
                    put(&mut self.input, Node::leaf("focus-follows-mouse"));
                    moved("focus follows the pointer")
                }
                None => left("this didn't read"),
            },
            "input:touchpad:natural_scroll" => self.toggle(Part::Touchpad, "natural-scroll", value),
            "input:touchpad:tap-to-click" => self.toggle(Part::Touchpad, "tap", value),
            "input:touchpad:disable_while_typing" => self.toggle(Part::Touchpad, "dwt", value),
            "input:touchpad:drag_lock" => self.toggle(Part::Touchpad, "drag-lock", value),
            "input:touchpad:middle_button_emulation" => {
                self.toggle(Part::Touchpad, "middle-emulation", value)
            }
            "input:touchpad:clickfinger_behavior" if !truthy(value) => {
                here("niri clicks by where the finger is, unless told")
            }
            "input:touchpad:clickfinger_behavior" => {
                put(
                    &mut self.touchpad,
                    Node::leaf("click-method \"clickfinger\""),
                );
                moved("clicks by the number of fingers")
            }
            "input:touchpad:scroll_factor" => {
                self.fraction(Part::Touchpad, "scroll-factor", float())
            }
            "general:gaps_in" => {
                self.gaps_in = int().map(|n| n.max(0) as u32);
                moved("gaps between windows")
            }
            "general:gaps_out" => {
                self.gaps_out = int().map(|n| n.max(0) as u32);
                moved("gaps at the edges")
            }
            "general:border_size" => {
                self.border_size = int().map(|n| n.max(0) as u32);
                moved(format!("borders {value} px"))
            }
            "general:col.active_border" => self.color("active", value),
            "general:col.inactive_border" => self.color("inactive", value),
            "general:layout" => left("niri lays windows out in scrolling columns"),
            "decoration:rounding" => {
                self.rounding = int().map(|n| n.max(0) as u32);
                moved(format!("corners {value} px"))
            }
            "decoration:active_opacity" => {
                self.opacity.0 = float();
                moved(format!("windows at {value} opacity"))
            }
            "decoration:inactive_opacity" => {
                self.opacity.1 = float();
                moved(format!("inactive windows at {value} opacity"))
            }
            "decoration:shadow:enabled" | "decoration:drop_shadow" => {
                put(
                    &mut self.shadow,
                    Node::leaf(if truthy(value) { "on" } else { "off" }),
                );
                moved(format!(
                    "shadows {}",
                    if truthy(value) { "on" } else { "off" }
                ))
            }
            "decoration:shadow:range" | "decoration:shadow_range" => match int() {
                Some(n) => {
                    put(&mut self.shadow, Node::leaf(format!("softness {n}")));
                    moved(format!("shadow softness {n}"))
                }
                None => left("this didn't read"),
            },
            "decoration:shadow:color" | "decoration:col.shadow" => match color(value) {
                Some(hex) => {
                    put(
                        &mut self.shadow,
                        Node::leaf(format!("color {}", quote(&hex))),
                    );
                    moved(format!("shadow color {hex}"))
                }
                None => left("this color didn't read"),
            },
            "decoration:shadow:offset" | "decoration:shadow_offset" => {
                let mut xy = value
                    .split_whitespace()
                    .filter_map(|n| n.parse::<i64>().ok());
                match (xy.next(), xy.next()) {
                    (Some(x), Some(y)) => {
                        put(&mut self.shadow, Node::leaf(format!("offset x={x} y={y}")));
                        moved("shadow offset")
                    }
                    _ => left("this offset didn't read"),
                }
            }
            "decoration:blur:enabled" => {
                self.blur = truthy(value);
                moved(format!(
                    "blur {}",
                    if self.blur {
                        "behind windows (niri 26.04 or newer)"
                    } else {
                        "off"
                    }
                ))
            }
            "cursor:inactive_timeout" | "general:cursor_inactive_timeout" => match float() {
                Some(seconds) if seconds > 0.0 => {
                    let ms = (seconds * 1000.0) as u64;
                    put(
                        &mut self.cursor,
                        Node::leaf(format!("hide-after-inactive-ms {ms}")),
                    );
                    moved(format!("the pointer hides after {ms} ms"))
                }
                _ => here("niri keeps the pointer shown"),
            },
            "cursor:hide_on_key_press" if !truthy(value) => {
                here("niri keeps the pointer while typing")
            }
            "cursor:hide_on_key_press" => {
                put(&mut self.cursor, Node::leaf("hide-when-typing"));
                moved("the pointer hides while typing")
            }
            "cursor:no_warps" | "general:no_cursor_warps" => {
                here("niri doesn't move the pointer to the focus unless told")
            }
            "misc:disable_hyprland_logo"
            | "misc:disable_splash_rendering"
            | "misc:force_default_wallpaper"
            | "misc:disable_hypr_chan" => here("niri draws no logo or wallpaper of its own"),
            _ if key.starts_with("decoration:blur:") => {
                left("niri's blur takes its own settings in background-effect { }")
            }
            _ if key.starts_with("decoration:shadow") => here("niri's shadows need nothing more"),
            _ => Some(match section {
                "dwindle" | "master" | "group" | "scrolling" => {
                    Outcome::Left("niri lays windows out in scrolling columns".into())
                }
                "xwayland" => Outcome::Here("niri runs X11 apps through xwayland-satellite".into()),
                "gestures" => Outcome::Here("niri's touchpad gestures are built in".into()),
                _ => Outcome::Left("it's Hyprland's own setting".into()),
            }),
        }
    }

    fn xkb(&mut self, key: &str, value: &str) -> Option<Outcome> {
        if value.is_empty() {
            return here("niri takes the system's keyboard layout");
        }
        put(&mut self.xkb, Node::leaf(format!("{key} {}", quote(value))));
        moved(format!("keyboard {key} {value}"))
    }

    fn part(&mut self, part: Part) -> &mut Vec<Node> {
        match part {
            Part::Keyboard => &mut self.keyboard,
            Part::Mouse => &mut self.mouse,
            Part::Touchpad => &mut self.touchpad,
        }
    }

    // On writes the flag; off is how niri starts anyway.
    fn toggle(&mut self, part: Part, name: &str, value: &str) -> Option<Outcome> {
        if !truthy(value) {
            return here(&format!("{name} is off in niri too"));
        }
        put(self.part(part), Node::leaf(name));
        moved(format!("{} {name}", part.name()))
    }

    fn number(&mut self, part: Part, name: &str, n: Option<i64>) -> Option<Outcome> {
        let n = n?;
        put(self.part(part), Node::leaf(format!("{name} {n}")));
        moved(format!("{} {name} {n}", part.name()))
    }

    fn fraction(&mut self, part: Part, name: &str, n: Option<f64>) -> Option<Outcome> {
        let n = super::number(n?);
        put(self.part(part), Node::leaf(format!("{name} {n}")));
        moved(format!("{} {name} {n}", part.name()))
    }

    // rgba(33ccffee) rgba(00ff99ee) 45deg is a gradient; one color is a color.
    fn color(&mut self, which: &str, value: &str) -> Option<Outcome> {
        let words: Vec<&str> = value.split_whitespace().collect();
        let colors: Vec<String> = words.iter().filter_map(|w| color(w)).collect();
        let angle = words
            .iter()
            .find_map(|w| w.strip_suffix("deg")?.parse::<i64>().ok());
        match colors.as_slice() {
            [] => left("this color didn't read"),
            [one] => {
                put(
                    &mut self.border,
                    Node::leaf(format!("{which}-color {}", quote(one))),
                );
                moved(format!("{which} border {one}"))
            }
            [from, .., to] => {
                put(
                    &mut self.border,
                    Node::leaf(format!(
                        "{which}-gradient from={} to={} angle={}",
                        quote(from),
                        quote(to),
                        angle.unwrap_or(0)
                    )),
                );
                moved(format!("{which} border {from} to {to}"))
            }
        }
    }

    pub(super) fn env(&mut self, value: &str) -> Outcome {
        let Some((name, said)) = value.split_once(',') else {
            return Outcome::Left("this has no value".into());
        };
        let (name, said) = (name.trim(), said.trim());
        match name {
            "XCURSOR_THEME" | "HYPRCURSOR_THEME" => {
                put(
                    &mut self.cursor,
                    Node::leaf(format!("xcursor-theme {}", quote(said))),
                );
                return Outcome::Moved(format!("cursor theme {said}"));
            }
            "XCURSOR_SIZE" | "HYPRCURSOR_SIZE" => {
                if let Ok(size) = said.parse::<u32>() {
                    put(&mut self.cursor, Node::leaf(format!("xcursor-size {size}")));
                    return Outcome::Moved(format!("cursor size {size}"));
                }
            }
            _ if name.starts_with("HYPRLAND") || name.starts_with("AQ_") => {
                return Outcome::Left("it's for Hyprland itself".into());
            }
            _ => {}
        }
        put(
            &mut self.environment,
            Node::leaf(format!("{name} {}", quote(said))),
        );
        Outcome::Moved(format!("{name}={said}"))
    }

    // exec-once runs at login; exec ran again on every reload, which niri doesn't do.
    pub(super) fn start(&mut self, value: &str, every: bool) -> Outcome {
        let line = match value.strip_prefix('[') {
            Some(rest) => rest.split_once(']').map_or(rest, |(_, after)| after).trim(),
            None => value.trim(),
        };
        if line.is_empty() {
            return Outcome::Left("it runs nothing".into());
        }
        if line.contains("hyprctl") || line.contains("hyprpm") {
            return Outcome::Left("it talks to Hyprland, which isn't there under niri".into());
        }
        self.startup
            .push(Node::leaf(format!("spawn-sh-at-startup {}", quote(line))));
        Outcome::Moved(if every {
            format!("starts with niri: {line} (once; niri doesn't run it again on reload)")
        } else {
            format!("starts with niri: {line}")
        })
    }
}

#[derive(Clone, Copy)]
enum Part {
    Keyboard,
    Mouse,
    Touchpad,
}

impl Part {
    fn name(self) -> &'static str {
        match self {
            Part::Keyboard => "keyboard",
            Part::Mouse => "mouse",
            Part::Touchpad => "touchpad",
        }
    }
}

// rgba(33ccffee), rgb(33ccff), 0xee33ccff or rgba(51, 204, 255, 0.93) as #33ccffee.
pub(super) fn color(value: &str) -> Option<String> {
    let value = value.trim();
    // hyprland.lua writes 0xee1a1a1a, which reads back as a plain number.
    if let Ok(argb) = value.parse::<u32>() {
        return color(&format!("0x{argb:08x}"));
    }
    if let Some(argb) = value.strip_prefix("0x") {
        let hex = argb.get(..8)?;
        return all_hex(hex).then(|| format!("#{}{}", &hex[2..], &hex[..2]).to_lowercase());
    }
    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .trim_end_matches(')');
    if inner.contains(',') {
        let parts: Vec<f64> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f64>().ok())
            .collect();
        let alpha = |a: f64| ((a.clamp(0.0, 1.0)) * 255.0).round() as u8;
        return match parts.as_slice() {
            [r, g, b] => Some(format!("#{:02x}{:02x}{:02x}", *r as u8, *g as u8, *b as u8)),
            [r, g, b, a] => Some(format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                *r as u8,
                *g as u8,
                *b as u8,
                alpha(*a)
            )),
            _ => None,
        };
    }
    (all_hex(inner) && matches!(inner.len(), 6 | 8)).then(|| format!("#{}", inner.to_lowercase()))
}

fn all_hex(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_read_in_every_way_hyprland_writes_them() {
        assert_eq!(color("rgba(33ccffee)").as_deref(), Some("#33ccffee"));
        assert_eq!(color("rgb(33CCFF)").as_deref(), Some("#33ccff"));
        assert_eq!(color("0xee33ccff").as_deref(), Some("#33ccffee"));
        assert_eq!(
            color("rgba(51, 204, 255, 1.0)").as_deref(),
            Some("#33ccffff")
        );
        assert_eq!(color("3994688026").as_deref(), Some("#1a1a1aee"));
        assert_eq!(color("red"), None);
    }
}

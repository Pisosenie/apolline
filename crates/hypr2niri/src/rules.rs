// Window and layer rules as every Hyprland since 0.30 wrote them, read into today's names:
//
// ≤0.47   windowrule = float, ^(kitty)$                v1, a bare pattern is the class
// ≤0.52   windowrulev2 = float, class:^(kitty)$        v2; 0.48–0.52 also as `windowrule`
// 0.53+   windowrule = match:class kitty, float on     and `windowrule { match:class = … }`
// ≤0.52   layerrule = blur, waybar  ·  blurls = waybar
// 0.53+   layerrule = blur on, match:namespace waybar

use crate::conf::Line;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    // Today's prop names (class, title, initial_class, namespace, float, xwayland…), the value
    // as written.
    pub matches: Vec<(String, String)>,
    // Today's effect names; one that took no value before reads "on".
    pub effects: Vec<(String, String)>,
}

// v2 prop → today's.
const PROPS: [(&str, &str); 18] = [
    ("class", "class"),
    ("title", "title"),
    ("initialClass", "initial_class"),
    ("initialclass", "initial_class"),
    ("initialTitle", "initial_title"),
    ("initialtitle", "initial_title"),
    ("tag", "tag"),
    ("xwayland", "xwayland"),
    ("floating", "float"),
    ("fullscreen", "fullscreen"),
    ("pinned", "pin"),
    ("focus", "focus"),
    ("group", "group"),
    ("modal", "modal"),
    ("workspace", "workspace"),
    ("onworkspace", "workspace"),
    ("content", "content"),
    ("xdgTag", "xdg_tag"),
];

// Effects that 0.53 gave snake_case names; the rest kept theirs.
const RENAMED: [(&str, &str); 36] = [
    ("fullscreenstate", "fullscreen_state"),
    ("noinitialfocus", "no_initial_focus"),
    ("suppressevent", "suppress_event"),
    ("noclosefor", "no_close_for"),
    ("persistentsize", "persistent_size"),
    ("bordercolor", "border_color"),
    ("bordersize", "border_size"),
    ("roundingpower", "rounding_power"),
    ("idleinhibit", "idle_inhibit"),
    ("maxsize", "max_size"),
    ("minsize", "min_size"),
    ("allowsinput", "allows_input"),
    ("dimaround", "dim_around"),
    ("focusonactivate", "focus_on_activate"),
    ("keepaspectratio", "keep_aspect_ratio"),
    ("nearestneighbor", "nearest_neighbor"),
    ("noanim", "no_anim"),
    ("noblur", "no_blur"),
    ("nodim", "no_dim"),
    ("nofocus", "no_focus"),
    ("nofollowmouse", "no_follow_mouse"),
    ("nomaxsize", "no_max_size"),
    ("noshadow", "no_shadow"),
    ("noshortcutsinhibit", "no_shortcuts_inhibit"),
    ("noscreenshare", "no_screen_share"),
    ("novrr", "no_vrr"),
    ("forcergbx", "force_rgbx"),
    ("syncfullscreen", "sync_fullscreen"),
    ("renderunfocused", "render_unfocused"),
    ("stayfocused", "stay_focused"),
    ("scrollmouse", "scroll_mouse"),
    ("scrolltouchpad", "scroll_touchpad"),
    ("noborder", "no_border"),
    ("norounding", "no_rounding"),
    ("blurpopups", "blur_popups"),
    ("ignorealpha", "ignore_alpha"),
];

fn effect(word: &str) -> String {
    let lower = word.to_lowercase();
    RENAMED
        .iter()
        .find(|(old, _)| *old == lower)
        .map_or(lower, |(_, new)| (*new).to_owned())
}

// "float" → (float, on); "opacity 0.9 0.8" → (opacity, "0.9 0.8").
fn effect_of(part: &str) -> (String, String) {
    let (name, value) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
    let value = value.trim();
    (
        effect(name),
        if value.is_empty() { "on" } else { value }.to_owned(),
    )
}

// The value of a windowrule or windowrulev2 line, whichever syntax it's in.
pub fn window(value: &str) -> Result<Rule, String> {
    let parts: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.iter().any(|p| p.starts_with("match:")) {
        return Ok(today(&parts));
    }
    let mut rule = Rule::default();
    for part in &parts {
        let prop = part
            .split_once(':')
            .and_then(|(name, value)| Some((PROPS.iter().find(|(p, _)| *p == name)?.1, value)));
        match prop {
            Some((name, value)) => rule
                .matches
                .push((name.to_owned(), value.trim().to_owned())),
            None => rule.effects.push(effect_of(part)),
        }
    }
    // v1: the last part is a pattern for the class.
    if rule.matches.is_empty() && rule.effects.len() >= 2 {
        let pattern = parts.last().copied().unwrap_or_default().to_owned();
        rule.effects.pop();
        rule.matches.push(("class".into(), pattern));
    }
    if rule.matches.is_empty() {
        return Err("this rule names no window to match".into());
    }
    Ok(rule)
}

// layerrule = blur, waybar (≤0.52) or layerrule = blur on, match:namespace waybar (0.53+).
pub fn layer(value: &str) -> Result<Rule, String> {
    let parts: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.iter().any(|p| p.starts_with("match:")) {
        return Ok(today(&parts));
    }
    match parts.as_slice() {
        [said, namespace] => Ok(Rule {
            matches: vec![("namespace".into(), (*namespace).to_owned())],
            effects: vec![effect_of(said)],
        }),
        _ => Err("this layer rule names no layer to match".into()),
    }
}

// 0.53+: `match:class kitty, float on, opacity 0.9`.
fn today(parts: &[&str]) -> Rule {
    let mut rule = Rule::default();
    for part in parts {
        match part.strip_prefix("match:") {
            Some(said) => {
                let (name, value) = said.split_once(char::is_whitespace).unwrap_or((said, ""));
                rule.matches
                    .push((name.to_owned(), value.trim().to_owned()));
            }
            None => rule.effects.push(effect_of(part)),
        }
    }
    rule
}

// 0.53+ rules as blocks: `windowrule { name = x; match:class = kitty; float = yes }`, by where
// each starts.
pub fn blocks(lines: &[Line], keyword: &str) -> Vec<(String, Rule)> {
    crate::conf::blocks(lines, keyword)
        .into_iter()
        .map(|(at, said)| {
            let mut rule = Rule::default();
            for (key, value) in said {
                if key == "name" || key == "enable" {
                    continue;
                }
                match key.strip_prefix("match:") {
                    Some(prop) => rule.matches.push((prop.to_owned(), value)),
                    None => rule.effects.push((effect(&key), value)),
                }
            }
            (at, rule)
        })
        .collect()
}

// Hyprland's yes: 1, or what starts with true, yes or on.
pub fn truthy(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    value == "1" || ["true", "yes", "on"].iter().any(|t| value.starts_with(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(matches: &[(&str, &str)], effects: &[(&str, &str)]) -> Rule {
        let own = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect()
        };
        Rule {
            matches: own(matches),
            effects: own(effects),
        }
    }

    #[test]
    fn every_syntax_of_the_same_rule_reads_the_same() {
        let wanted = rule(&[("class", "^(pavucontrol)$")], &[("float", "on")]);
        for written in [
            "float, ^(pavucontrol)$",
            "float, class:^(pavucontrol)$",
            "match:class ^(pavucontrol)$, float on",
        ] {
            assert_eq!(window(written), Ok(wanted.clone()), "{written}");
        }
        assert_eq!(
            window("noblur, title:^(Picture-in-Picture)$"),
            Ok(rule(
                &[("title", "^(Picture-in-Picture)$")],
                &[("no_blur", "on")]
            ))
        );
        assert_eq!(
            window("opacity 0.9 0.8, float, initialClass:^(org.gnome.Calculator)$"),
            Ok(rule(
                &[("initial_class", "^(org.gnome.Calculator)$")],
                &[("opacity", "0.9 0.8"), ("float", "on")]
            ))
        );
        assert!(window("float").is_err());
    }

    #[test]
    fn layer_rules_and_rule_blocks_read_in_todays_names() {
        assert_eq!(
            layer("blur, waybar"),
            Ok(rule(&[("namespace", "waybar")], &[("blur", "on")]))
        );
        assert_eq!(
            layer("blur on, match:namespace waybar"),
            Ok(rule(&[("namespace", "waybar")], &[("blur", "on")]))
        );
        let line = |key: &str, value: &str, block| Line {
            key: key.into(),
            value: value.into(),
            at: format!("hyprland.conf:{block}"),
            block,
        };
        let lines = [
            line("windowrule:name", "kitty", 1),
            line("windowrule:match:class", "kitty", 1),
            line("windowrule:float", "yes", 1),
            line("windowrule:match:title", "Mail", 2),
            line("windowrule:opacity", "0.9", 2),
        ];
        assert_eq!(
            blocks(&lines, "windowrule"),
            [
                (
                    "hyprland.conf:1".into(),
                    rule(&[("class", "kitty")], &[("float", "yes")])
                ),
                (
                    "hyprland.conf:2".into(),
                    rule(&[("title", "Mail")], &[("opacity", "0.9")])
                ),
            ]
        );
        assert!(truthy("yes") && truthy("On") && truthy("1") && !truthy("0"));
    }
}

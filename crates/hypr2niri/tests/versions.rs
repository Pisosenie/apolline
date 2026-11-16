// One config per era of Hyprland's syntax, each read the way its Hyprland reads it. CI runs the
// configs these make through `niri validate` too.

use std::path::Path;

use hypr2niri::niri::{self, Screens};

fn convert(name: &str) -> niri::Converted {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/configs")
        .join(name);
    let lua = name.ends_with(".lua");
    let lines = if lua {
        hypr2niri::lua::read(&path).0
    } else {
        hypr2niri::conf::read(&path)
    };
    assert!(!lines.is_empty(), "{name} read nothing");
    niri::convert(&lines, lua, name, &Screens::default())
}

fn has(converted: &niri::Converted, wanted: &[&str]) {
    for line in wanted {
        assert!(
            converted.kdl.contains(line),
            "{line:?} missing from:\n{}",
            converted.kdl
        );
    }
}

#[test]
fn hyprland_0_30_v1_rules_old_shadows_and_submaps() {
    let c = convert("0.30.conf");
    assert_eq!(c.era.said(), "Hyprland 0.35 or older");
    has(
        &c,
        &[
            "layout \"us,ru\"",
            "options \"grp:alt_shift_toggle\"",
            "output \"HDMI-A-1\" {",
            "transform \"90\"",
            "gaps 10",
            "active-gradient from=\"#33ccffee\" to=\"#00ff99ee\" angle=45",
            "softness 4",
            "match app-id=\"^(pavucontrol)$\"",
            "match title=\"^(Mail)$\"",
            "opacity 0.9",
            "Mod+Shift+2 { move-column-to-workspace 2 focus=false; }",
            "spawn-sh-at-startup \"waybar & nm-applet\"",
        ],
    );
    // What the submap bound is not bound everywhere.
    assert!(!c.kdl.contains("Right { set-column-width"), "{}", c.kdl);
    assert!(c.kdl.contains("niri has no key chords (submaps)"));
}

#[test]
fn hyprland_0_45_shadow_blocks_windowrulev2_and_descriptions() {
    let c = convert("0.45.conf");
    assert_eq!(c.era.said(), "Hyprland 0.45–0.52");
    has(
        &c,
        &[
            "output \"Dell Inc. DELL U2720Q 123ABC\" {",
            "scale 1.5",
            "softness 12",
            "color \"#1a1a1aee\"",
            "opacity 0.95",
            "match is-active=false",
            "xcursor-size 24",
            "QT_QPA_PLATFORMTHEME \"qt6ct\"",
            "match app-id=\"^(org\\\\.gnome\\\\.Calculator)$\"",
            "fixed 800",
            "workspace \"chat\"",
            "open-on-workspace \"chat\"",
            "match namespace=\"^(?:waybar)$\"",
            "match namespace=\"^(?:rofi)$\"",
            "Mod+Return hotkey-overlay-title=\"Open a terminal\" { spawn-sh \"foot\"; }",
            "XF86AudioMute allow-when-locked=true",
            "Mod+F { maximize-column; }",
            "spawn-sh-at-startup \"firefox\"",
        ],
    );
    assert!(!c.kdl.contains("hyprctl dispatch exit\";"));
}

#[test]
fn hyprland_0_51_monitor_blocks_gestures_and_a_bind_written_twice() {
    let c = convert("0.51.conf");
    assert_eq!(c.era.said(), "Hyprland 0.51–0.52");
    has(
        &c,
        &[
            "output \"eDP-1\" {",
            "mode \"2880x1800\"",
            "scale 1.75",
            "variable-refresh-rate",
            "output \"DP-2\" {\n    off",
            "match app-id=\"^(blueman-manager)$\"",
            "opacity 0.9",
            "Mod+Q { spawn-sh \"foot\"; }",
        ],
    );
    assert_eq!(c.kdl.matches("Mod+Q {").count(), 1, "the later bind wins");
}

#[test]
fn hyprland_0_54_match_rules_rule_blocks_and_layer_rules() {
    let c = convert("0.54.conf");
    assert_eq!(c.era.said(), "Hyprland 0.53 or newer");
    has(
        &c,
        &[
            "match app-id=\"^(pavucontrol)$\"",
            "fixed 900",
            "match title=\"^(Picture-in-Picture)$\"",
            "exclude app-id=\"^(kitty)$\"",
            "match is-floating=false",
            "shadow {\n        off",
            "match app-id=\"^(?:thunderbird)$\"",
            "open-on-output \"DP-1\"",
            "open-focused false",
            "match namespace=\"^(?:waybar)$\"",
            "match namespace=\"^(?:rofi)$\"",
            "hide-after-inactive-ms 3000",
            "hide-when-typing",
            "animations {\n    off",
        ],
    );
}

#[test]
fn hyprland_0_57_lua() {
    let c = convert("0.57.lua");
    assert_eq!(c.era.said(), "Hyprland 0.55 or newer");
    has(
        &c,
        &[
            "output \"DP-1\" {",
            "gaps 8",
            "width 1",
            "geometry-corner-radius 6",
            "color \"#1a1a1aee\"",
            "repeat-rate 40",
            "xcursor-theme \"Bibata-Modern-Ice\"",
            "match title=\"^(Picture-in-Picture)$\"",
            "match namespace=\"^(?:waybar)$\"",
            "spawn-sh-at-startup \"waybar\"",
            "Mod+Return { spawn-sh \"kitty\"; }",
            "Mod+Shift+3 { move-column-to-workspace 3 focus=false; }",
            "Mod+V { toggle-window-floating; }",
        ],
    );
}

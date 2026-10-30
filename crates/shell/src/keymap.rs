use apolline_config::{Config, Shortcut};

use crate::{fragment, keys};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Apolline,
    Mode,
    Windows,
    Workspaces,
    Screens,
    System,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Apolline,
        Group::Mode,
        Group::Windows,
        Group::Workspaces,
        Group::Screens,
        Group::System,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::Apolline => "Apolline",
            Group::Mode => "This mode",
            Group::Windows => "Windows",
            Group::Workspaces => "Workspaces",
            Group::Screens => "Screens",
            Group::System => "Apps and system",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub keys: String,
    pub what: String,
    pub group: Group,
}

pub fn entries(config: &Config, mode: Option<usize>, niri_config: &str) -> Vec<Entry> {
    let mut list: Vec<Entry> = Shortcut::ALL
        .into_iter()
        .map(|shortcut| Entry {
            keys: config.shortcuts.get(shortcut).to_owned(),
            what: fragment::action(shortcut).0.to_owned(),
            group: Group::Apolline,
        })
        .collect();
    if let Some(mode) = mode.and_then(|i| config.modes.get(i)) {
        let taken = |combo: &String| list_taken(config, combo);
        list.extend(
            mode.keys
                .iter()
                .filter(|(combo, _)| !taken(combo))
                .map(|(combo, command)| Entry {
                    keys: combo.clone(),
                    what: format!("Run {command}"),
                    group: Group::Mode,
                }),
        );
    }
    if config.windows.switcher {
        for (keys, what) in [
            ("Alt+Tab", "Recent windows, with previews"),
            ("Alt+Shift+Tab", "Recent windows, backwards"),
            ("Alt+Grave", "Recent windows of this app"),
        ] {
            list.push(Entry {
                keys: keys.into(),
                what: what.into(),
                group: Group::Windows,
            });
        }
    }
    for (combo, action) in keys::binds(niri_config) {
        if list.iter().any(|e| keys::same(&e.keys, &combo)) {
            continue;
        }
        let (group, what) = describe(&action);
        list.push(Entry {
            keys: combo,
            what,
            group,
        });
    }
    list
}

fn list_taken(config: &Config, combo: &str) -> bool {
    Shortcut::ALL
        .into_iter()
        .any(|s| keys::same(config.shortcuts.get(s), combo))
}

pub fn matches(entry: &Entry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || entry.what.to_lowercase().contains(&query)
        || entry.keys.to_lowercase().contains(&query)
}

pub fn describe(action: &str) -> (Group, String) {
    let mut words = action.split_whitespace();
    let name = words.next().unwrap_or_default();
    let rest: Vec<&str> = words.collect();
    let argument = rest.join(" ").replace('"', "");
    let direction = |name: &str| {
        ["left", "right", "up", "down", "first", "last", "previous"]
            .into_iter()
            .find(|d| name.ends_with(d))
            .unwrap_or("")
    };
    let side = direction(name);
    let text = match name {
        "spawn" => {
            return (
                Group::System,
                format!("Open {}", argument.split(' ').next().unwrap_or("")),
            );
        }
        "spawn-sh" => return (Group::System, format!("Run {argument}")),
        "close-window" => "Close the window".into(),
        "fullscreen-window" => "Fullscreen".into(),
        "maximize-column" => "Make the column full width".into(),
        "maximize-window-to-edges" => "Fill the screen, keep the bar".into(),
        "center-column" | "center-window" => "Center the column".into(),
        "toggle-window-floating" => "Float or tile the window".into(),
        "switch-focus-between-floating-and-tiling" => "Jump between floating and tiled".into(),
        "toggle-column-tabbed-display" => "Show the column as tabs".into(),
        "switch-preset-column-width" => "Cycle column widths".into(),
        "switch-preset-window-height" => "Cycle window heights".into(),
        "reset-window-height" => "Reset the window height".into(),
        "expand-column-to-available-width" => "Widen to the free space".into(),
        "set-column-width" | "set-window-height" => {
            let what = if name == "set-column-width" {
                "column"
            } else {
                "window"
            };
            let verb = if argument.starts_with('-') {
                "Narrow"
            } else {
                "Widen"
            };
            format!(
                "{verb} the {what} by {}",
                argument.trim_start_matches(['+', '-'])
            )
        }
        "toggle-overview" => return (Group::Workspaces, "Overview of everything".into()),
        "screenshot" | "screenshot-screen" | "screenshot-window" => {
            let what = match name {
                "screenshot-screen" => "Screenshot the screen",
                "screenshot-window" => "Screenshot the window",
                _ => "Screenshot an area",
            };
            return (Group::System, what.into());
        }
        "quit" => return (Group::System, "Log out".into()),
        "power-off-monitors" => return (Group::Screens, "Turn the screens off".into()),
        "show-hotkey-overlay" => return (Group::System, "niri's own key list".into()),
        "toggle-keyboard-shortcuts-inhibit" => {
            return (Group::System, "Let the app have every key".into());
        }
        "switch-layout" => return (Group::System, "Next keyboard layout".into()),
        "focus-workspace" => return (Group::Workspaces, format!("Go to workspace {argument}")),
        "move-column-to-workspace" | "move-window-to-workspace" => {
            return (
                Group::Workspaces,
                format!("Move the window to workspace {argument}"),
            );
        }
        _ if name.starts_with("focus-workspace") => {
            return (
                Group::Workspaces,
                format!("Go to the workspace {}", word(side)),
            );
        }
        _ if name.starts_with("move-workspace") => {
            return (
                Group::Workspaces,
                format!("Move this workspace {}", word(side)),
            );
        }
        _ if name.contains("to-workspace") => {
            return (
                Group::Workspaces,
                format!("Move the window to the workspace {}", word(side)),
            );
        }
        _ if name.contains("monitor") => {
            let verb = if name.starts_with("focus") {
                "Focus the screen"
            } else {
                "Move it to the screen"
            };
            return (
                Group::Screens,
                format!("{verb} {}", word(side)).trim().to_owned(),
            );
        }
        _ if name.starts_with("focus-column") || name.starts_with("focus-window") => {
            format!("Focus the window {}", word(side)).trim().to_owned()
        }
        _ if name.starts_with("move-column") || name.starts_with("move-window") => {
            format!("Move the window {side}").trim().to_owned()
        }
        _ if name.starts_with("consume") || name.starts_with("expel") => {
            "Pull the window into or out of the column".into()
        }
        _ => {
            let readable = name.replace('-', " ");
            return (Group::System, capitalize(&readable));
        }
    };
    (Group::Windows, text)
}

fn word(side: &str) -> &'static str {
    match side {
        "left" => "on the left",
        "right" => "on the right",
        "up" => "above",
        "down" => "below",
        "first" => "first",
        "last" => "last",
        "previous" => "you were on",
        _ => "",
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub const SEARCH: &str = "keys-search";

pub fn view<'a>(
    entries: &'a [Entry],
    query: &'a str,
    palette: apolline_theme::Palette,
) -> iced::Element<'a, crate::shell::Message> {
    use apolline_theme::neutral;
    use iced::widget::{
        Column, Row, Space, column, container, mouse_area, opaque, row, scrollable, text,
        text_input,
    };
    use iced::{Alignment, Element, Fill, Padding};

    use crate::icon::{Icon, icon};
    use crate::shell::Message;
    use crate::style;

    let caps = |combo: &str| -> Element<'a, Message> {
        Row::with_children(keys::caps(combo).into_iter().map(|cap| {
            container(text(cap).size(11).font(style::mono()))
                .padding([2, 7])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(style::color(style::FILL))),
                    border: iced::Border {
                        color: style::color(neutral::HAIRLINE),
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    ..container::Style::default()
                })
                .into()
        }))
        .spacing(3)
        .into()
    };
    let groups: Vec<Element<'a, Message>> = Group::ALL
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<Element<'a, Message>> = entries
                .iter()
                .filter(|e| e.group == group && matches(e, query))
                .map(|e| {
                    row![
                        container(caps(&e.keys)).width(170),
                        text(e.what.as_str())
                            .size(13)
                            .color(style::color(style::SOFT_TEXT))
                            .width(Fill),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .into()
                })
                .collect();
            (!rows.is_empty()).then(|| {
                column![
                    text(group.title().to_uppercase())
                        .size(10)
                        .font(style::semibold())
                        .color(style::color(palette.accent)),
                    Column::with_children(rows).spacing(7),
                ]
                .spacing(10)
                .into()
            })
        })
        .collect();
    let empty = groups.is_empty();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (index, group) in groups.into_iter().enumerate() {
        if index % 2 == 0 {
            left.push(group);
        } else {
            right.push(group);
        }
    }
    let left = Column::with_children(left).spacing(22);
    let right = Column::with_children(right).spacing(22);
    let body: Element<'a, Message> = if empty {
        container(
            text(format!("No shortcut does “{}”", query.trim()))
                .size(14)
                .color(style::color(style::SOFT_TEXT)),
        )
        .center_x(Fill)
        .padding([40, 0])
        .into()
    } else {
        scrollable(
            row![left.width(Fill), right.width(Fill)]
                .spacing(28)
                .padding([4, 4]),
        )
        .direction(style::thin())
        .style(style::scroller)
        .height(540)
        .into()
    };
    let search = row![
        icon(Icon::Search, 18.0, style::FAINT),
        text_input("What do you want to do?", query)
            .id(SEARCH)
            .on_input(Message::KeysQuery)
            .size(16)
            .padding(0)
            .style(style::bare_field(palette)),
        text("Change Apolline's in Settings → Shortcuts")
            .size(12)
            .color(style::color(style::FAINT)),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    let panel = container(
        column![
            container(search).padding([16, 18]),
            container(Space::new().height(1))
                .width(Fill)
                .style(|_| container::background(style::color(neutral::HAIRLINE))),
            container(body).padding([16, 20]),
        ]
        .width(920),
    )
    .style(style::surface);
    mouse_area(
        container(opaque(panel))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .padding(Padding::new(0.0).top(110.0))
            .style(style::scrim),
    )
    .on_press(Message::Overlay(None))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niri_actions_read_like_sentences() {
        assert_eq!(
            describe("focus-column-left").1,
            "Focus the window on the left"
        );
        assert_eq!(describe("move-window-down").1, "Move the window down");
        assert_eq!(
            describe("focus-workspace 3"),
            (Group::Workspaces, "Go to workspace 3".into())
        );
        assert_eq!(
            describe(r#"set-column-width "-10%""#).1,
            "Narrow the column by 10%"
        );
        assert_eq!(
            describe(r#"spawn "alacritty""#),
            (Group::System, "Open alacritty".into())
        );
        assert_eq!(describe("focus-monitor-right").0, Group::Screens);
        assert_eq!(describe("something-new").1, "Something new");
    }

    #[test]
    fn the_list_puts_apolline_first_and_skips_what_it_replaces() {
        let (config, _) =
            Config::parse("[[mode]]\nname = \"Play\"\nkeys = { \"Mod+G\" = \"steam\" }\n");
        let niri = "binds {\n    Mod+Space { spawn \"fuzzel\"; }\n    Mod+Left { focus-column-left; }\n}\n";
        let list = entries(&config, Some(0), niri);
        assert_eq!(list[0].group, Group::Apolline);
        assert!(
            list.iter()
                .any(|e| e.group == Group::Mode && e.what == "Run steam")
        );
        assert_eq!(
            list.iter()
                .filter(|e| keys::same(&e.keys, "Mod+Space"))
                .count(),
            1
        );
        let left = list.iter().find(|e| e.keys == "Mod+Left").unwrap();
        assert!(matches(left, "left"));
        assert!(!matches(left, "screenshot"));
    }
}

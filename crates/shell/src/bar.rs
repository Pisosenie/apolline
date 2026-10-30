use apolline_config::{ModeSettings, Workspaces};
use apolline_theme::{Palette, metrics, neutral};
use chrono::{DateTime, Local};
use iced::widget::{Row, button, column, container, mouse_area, row, space, text};
use iced::{Alignment, Background, Element, Fill, Theme};

use crate::apps::{self, App};
use crate::controls::Controls;
use crate::icon::{Icon, icon};
use crate::media::Track;
use crate::power::Battery;
use crate::shell::{Message, Overlay};
use crate::status::Status;
use crate::style;

pub struct Bar<'a> {
    pub mode: Option<&'a ModeSettings>,
    pub has_modes: bool,
    pub palette: Palette,
    pub active: usize,
    pub status: Status,
    pub battery: Option<Battery>,
    pub track: Option<&'a Track>,
    pub controls: &'a Controls,
    pub held: usize,
    pub do_not_disturb: bool,
    pub awake: bool,
    pub now: DateTime<Local>,
    pub look: apolline_config::Bar,
    pub panel_open: bool,
    pub apps: &'a [App],
    pub pinned: Vec<Pinned>,
}

pub struct Pinned {
    pub id: String,
    pub running: bool,
    pub focused: bool,
}

pub fn view(bar: Bar<'_>) -> Element<'_, Message> {
    let palette = bar.palette;

    let chip = bar.has_modes.then(|| {
        let (glyph, name, tint) = match bar.mode {
            Some(mode) => (Icon::named(&mode.icon), mode.name.as_str(), palette.accent),
            None => (Icon::Circle, "No mode", neutral::MUTED),
        };
        let soft = if bar.mode.is_some() {
            palette.accent_soft
        } else {
            style::FILL
        };
        button(
            row![
                icon(glyph, 14.0, tint),
                text(name).size(13).font(style::medium()),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([4, 11])
        .style(move |_: &Theme, status| button::Style {
            background: Some(Background::Color(style::color(match status {
                button::Status::Hovered | button::Status::Pressed => tint.alpha(0.28),
                _ => soft,
            }))),
            text_color: style::color(tint),
            border: iced::Border {
                radius: style::PILL.into(),
                ..iced::Border::default()
            },
            ..button::Style::default()
        })
        .on_press(Message::Overlay(Some(Overlay::Switcher)))
    });

    let left = row![chip, workspaces(&bar), tasks(&bar)]
        .spacing(10)
        .align_y(Alignment::Center);

    let mut clock = row![].spacing(10).align_y(Alignment::Center);
    if bar.look.date {
        clock = clock.push(
            text(bar.now.format("%a %-d %b").to_string())
                .size(13)
                .color(style::color(neutral::MUTED)),
        );
    }
    let time = if bar.look.seconds {
        "%H:%M:%S"
    } else {
        "%H:%M"
    };
    clock = clock.push(
        text(bar.now.format(time).to_string())
            .size(13)
            .font(style::semibold()),
    );

    let mut right = Row::new().spacing(6).align_y(Alignment::Center);
    if let Some(track) = bar.track.filter(|t| t.playing) {
        right = right.push(
            button(
                row![
                    icon(Icon::Music, 13.0, palette.accent),
                    text(crate::media::shorten(&track.line(), 34))
                        .size(12)
                        .color(style::color(neutral::MUTED)),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .padding([4, 10])
            .style(style::ghost(style::PILL))
            .on_press(Message::Overlay(Some(Overlay::Panel))),
        );
    }
    if bar.held > 0 || bar.do_not_disturb {
        let mut bell = row![icon(
            if bar.do_not_disturb {
                Icon::BellOff
            } else {
                Icon::Bell
            },
            15.0,
            neutral::TEXT
        )]
        .spacing(5)
        .align_y(Alignment::Center);
        if bar.held > 0 {
            bell = bell.push(
                container(
                    text(bar.held.to_string())
                        .size(10)
                        .font(style::semibold())
                        .color(style::color(palette.on_accent)),
                )
                .padding([0, 5])
                .style(style::pill(palette.accent)),
            );
        }
        right = right.push(
            button(bell)
                .padding([5, 8])
                .style(style::ghost(style::PILL))
                .on_press(Message::Overlay(Some(Overlay::Panel))),
        );
    }
    right = right.push(tray(&bar));

    container(
        row![
            container(left).width(Fill),
            clock,
            container(right).width(Fill).align_x(Alignment::End),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(metrics::BAR_HEIGHT)
    .padding([0, 5])
    .align_y(Alignment::Center)
    .style(style::glass(bar.look.opacity, bar.look.floating))
    .into()
}

fn workspaces<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let palette = bar.palette;
    Row::with_children((0..bar.status.workspaces).map(|i| {
        let active = i == bar.active;
        let busy = bar.status.busy.contains(&i);
        let mark: Element<'a, Message> = match bar.look.workspaces {
            Workspaces::Dots => {
                let (width, fill) = match (active, busy) {
                    (true, _) => (18.0, palette.accent),
                    (false, true) => (6.0, neutral::TEXT.alpha(0.72)),
                    (false, false) => (6.0, neutral::TEXT.alpha(0.26)),
                };
                container(space())
                    .width(width)
                    .height(6)
                    .style(style::pill(fill))
                    .into()
            }
            Workspaces::Numbers => {
                let ink = match (active, busy) {
                    (true, _) => palette.accent,
                    (false, true) => neutral::TEXT,
                    (false, false) => neutral::TEXT.alpha(0.45),
                };
                text((i + 1).to_string())
                    .size(12)
                    .font(style::mono())
                    .color(style::color(ink))
                    .into()
            }
        };
        let width = match (bar.look.workspaces, active) {
            (Workspaces::Dots, true) => 30.0,
            (Workspaces::Dots, false) => 18.0,
            (Workspaces::Numbers, _) => 24.0,
        };
        let highlight = active && bar.look.workspaces == Workspaces::Numbers;
        button(container(mark).center_x(width).center_y(24))
            .padding(0)
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if highlight {
                    style.background = Some(Background::Color(style::color(palette.accent_soft)));
                }
                style
            })
            .on_press(Message::Workspace(i, bar.status.ids.get(i).copied()))
            .into()
    }))
    .spacing(0)
    .align_y(Alignment::Center)
    .into()
}

fn tasks<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let palette = bar.palette;
    let pinned = bar.pinned.iter().map(|pin| {
        let glyph = crate::windows::picture(bar.apps, &pin.id, 16.0, palette);
        let (width, tint) = match (pin.focused, pin.running) {
            (true, _) => (12, palette.accent),
            (false, true) => (4, neutral::MUTED),
            (false, false) => (0, neutral::MUTED),
        };
        let marker = container(space())
            .width(width)
            .height(2)
            .style(style::pill(tint));
        let focused = pin.focused;
        let button = button(column![glyph, marker].spacing(2).align_x(Alignment::Center))
            .padding(iced::Padding::new(4.0).bottom(1.0))
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if focused {
                    style.background = Some(Background::Color(style::color(style::FILL)));
                }
                style
            })
            .on_press(Message::OpenPinned(pin.id.clone()));
        mouse_area(button)
            .on_right_press(Message::Pin(pin.id.clone()))
            .into()
    });
    let pinned_ids: Vec<&str> = bar.pinned.iter().map(|pin| pin.id.as_str()).collect();
    let loose: Vec<&(u64, String, bool)> = bar
        .status
        .windows
        .iter()
        .filter(|(_, app, _)| !pinned_ids.contains(&app_key(bar.apps, app)))
        .collect();
    let gap = (!bar.pinned.is_empty() && !loose.is_empty()).then(|| {
        container(space())
            .width(1)
            .height(14)
            .style(|_| container::background(style::color(neutral::HAIRLINE)))
            .into()
    });
    let windows = loose.into_iter().map(|(id, app, focused)| {
        let focused = *focused;
        let glyph = crate::windows::picture(bar.apps, app, 16.0, palette);
        let marker = container(space())
            .width(if focused { 12 } else { 0 })
            .height(2)
            .style(style::pill(palette.accent));
        let task = button(column![glyph, marker].spacing(2).align_x(Alignment::Center))
            .padding(iced::Padding::new(4.0).bottom(1.0))
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if focused {
                    style.background = Some(Background::Color(style::color(style::FILL)));
                }
                style
            })
            .on_press(Message::FocusWindow(*id));
        mouse_area(task)
            .on_middle_press(Message::CloseWindow(*id))
            .on_right_press(Message::Pin(app_key(bar.apps, app).to_owned()))
            .into()
    });
    Row::with_children(pinned.chain(gap).chain(windows))
        .spacing(2)
        .align_y(Alignment::Center)
        .into()
}

pub fn app_key<'a>(apps: &'a [App], app_id: &'a str) -> &'a str {
    apps::find(apps, app_id).map_or(app_id, |app| app.id.as_str())
}

fn tray<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let tint = neutral::TEXT;
    let mut items = Row::new().spacing(10).align_y(Alignment::Center);
    if !bar.status.keyboard.is_empty() {
        items = items.push(
            text(bar.status.keyboard.clone())
                .size(11)
                .font(style::semibold())
                .color(style::color(neutral::MUTED)),
        );
    }
    if bar.awake {
        items = items.push(icon(Icon::Coffee, 15.0, bar.palette.accent));
    }
    if let Some(wifi) = &bar.controls.wifi {
        let glyph = if wifi.on && wifi.connected.is_some() {
            Icon::Wifi
        } else {
            Icon::WifiOff
        };
        items = items.push(icon(glyph, 15.0, tint));
    }
    if let Some(bluetooth) = bar.controls.bluetooth.as_ref().filter(|b| b.on) {
        let shade = if bluetooth.connected.is_some() {
            tint
        } else {
            neutral::MUTED
        };
        items = items.push(icon(Icon::Bluetooth, 15.0, shade));
    }
    if let Some(volume) = bar.controls.volume {
        items = items.push(icon(Icon::volume(volume.level, volume.muted), 15.0, tint));
    }
    if let Some(battery) = bar.battery {
        let shade = match battery {
            Battery { charging: true, .. } => bar.palette.accent,
            Battery {
                percent: 0..=10, ..
            } => style::DANGER,
            _ => tint,
        };
        let mut level = row![icon(
            Icon::battery(battery.percent, battery.charging),
            17.0,
            shade
        )]
        .spacing(5)
        .align_y(Alignment::Center);
        if bar.look.battery_percent {
            level = level.push(
                text(format!("{}%", battery.percent))
                    .size(12)
                    .font(style::medium())
                    .color(style::color(shade)),
            );
        }
        items = items.push(level);
    }
    let open = bar.panel_open;
    button(items)
        .padding([5, 11])
        .style(move |theme, status| {
            let mut style = style::ghost(style::PILL)(theme, status);
            if open {
                style.background = Some(Background::Color(style::color(style::HOVER)));
            }
            style
        })
        .on_press(Message::Overlay(if open {
            None
        } else {
            Some(Overlay::Panel)
        }))
        .into()
}

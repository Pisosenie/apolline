use apolline_config::Workspaces;
use apolline_theme::{Mode, Palette, metrics, neutral};
use chrono::{DateTime, Local};
use iced::widget::{Row, button, container, row, space, text};
use iced::{Alignment, Background, Element, Fill, Theme};

use crate::controls::Controls;
use crate::icon::{Icon, icon};
use crate::media::Track;
use crate::power::Battery;
use crate::shell::{Message, Overlay};
use crate::status::Status;
use crate::style;

pub struct Bar<'a> {
    pub mode: Mode,
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
}

pub fn view(bar: Bar<'_>) -> Element<'_, Message> {
    let palette = bar.palette;

    let chip = button(
        row![
            icon(Icon::of(bar.mode), 14.0, palette.accent),
            text(bar.mode.name()).size(13).font(style::medium()),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([4, 11])
    .style(move |_: &Theme, status| button::Style {
        background: Some(Background::Color(style::color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.accent.alpha(0.28),
            _ => palette.accent_soft,
        }))),
        text_color: style::color(palette.accent),
        border: iced::Border {
            radius: style::PILL.into(),
            ..iced::Border::default()
        },
        ..button::Style::default()
    })
    .on_press(Message::Overlay(Some(Overlay::Switcher)));

    let left = row![chip, workspaces(&bar)]
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
    .style(style::bar)
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

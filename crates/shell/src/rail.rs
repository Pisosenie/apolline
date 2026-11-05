use crate::style::text;
use apolline_config::{Piece, Shape, TrayItem, Workspaces};
use apolline_theme::neutral;
use iced::widget::{Column, button, column, container, mouse_area, space, tooltip};
use iced::{Alignment, Background, Element, Fill, Theme};

use crate::bar::{Bar, app_key, module, tray_item};
use crate::icon::{Icon, icon};
use crate::power::Battery;
use crate::shell::{Message, Overlay};
use crate::style;

// The bar down a side of the screen: the same pieces, stacked, with words cut down to what fits.
pub fn view(bar: Bar<'_>) -> Element<'_, Message> {
    let look = bar.look;
    if look.shape == Shape::Pill {
        return pill(&bar);
    }
    let islands = look.shape == Shape::Islands;
    let zone = |pieces: &[Piece]| -> Element<'_, Message> {
        let parts: Vec<Element<'_, Message>> =
            pieces.iter().filter_map(|p| piece(&bar, p)).collect();
        let empty = parts.is_empty();
        let stack = Column::with_children(parts)
            .spacing(8)
            .align_x(Alignment::Center);
        if !islands {
            return stack.into();
        }
        if empty {
            return space().into();
        }
        container(stack)
            .width(Fill)
            .padding([6, 0])
            .align_x(Alignment::Center)
            .style(style::glass(look.opacity, true, bar.radius))
            .into()
    };
    let sides = if look.center.is_empty() {
        column![container(zone(&look.left)).height(Fill), zone(&look.right),]
    } else {
        column![
            container(zone(&look.left)).height(Fill),
            zone(&look.center),
            container(zone(&look.right))
                .height(Fill)
                .align_y(Alignment::End),
        ]
    };
    let strip = container(sides.align_x(Alignment::Center))
        .width(style::RAIL)
        .height(Fill)
        .align_x(Alignment::Center);
    if islands {
        strip.into()
    } else {
        strip
            .padding([6, 0])
            .style(style::glass(look.opacity, look.floating, bar.radius))
            .into()
    }
}

// One pill down the middle of the side, with everything in it.
fn pill<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let look = bar.look;
    let parts: Vec<Element<'a, Message>> = [&look.left, &look.center, &look.right]
        .into_iter()
        .flat_map(|pieces| pieces.iter().filter_map(|p| piece(bar, p)))
        .collect();
    container(
        container(
            Column::with_children(parts)
                .spacing(10)
                .align_x(Alignment::Center),
        )
        .width(Fill)
        .padding([10, 0])
        .align_x(Alignment::Center)
        .style(style::glass(look.opacity, true, bar.radius)),
    )
    .width(style::RAIL)
    .height(Fill)
    .center_y(Fill)
    .into()
}

fn piece<'a>(bar: &Bar<'a>, piece: &Piece) -> Option<Element<'a, Message>> {
    match piece {
        Piece::Mode => mode(bar),
        Piece::Workspaces => Some(workspaces(bar)),
        Piece::Windows => Some(windows(bar)),
        Piece::Clock => Some(clock(bar)),
        Piece::Media => bar.track.filter(|t| t.playing).map(|track| {
            hint(
                button(icon(Icon::Music, 15.0, bar.palette.accent))
                    .padding(6)
                    .style(style::ghost(style::PILL))
                    .on_press(Message::Overlay(Some(Overlay::Panel))),
                track.line(),
            )
        }),
        Piece::Notifications => bell(bar),
        Piece::Recording => {
            let recorded = bar.recording.map(|elapsed| {
                let minutes = elapsed.as_secs() / 60;
                button(
                    column![
                        container(space().width(8).height(8)).style(style::pill(style::DANGER)),
                        text(crate::i18n::fill("{minutes}m", &[("minutes", &minutes)]))
                            .size(10)
                            .font(style::mono()),
                    ]
                    .spacing(3)
                    .align_x(Alignment::Center),
                )
                .padding(4)
                .style(style::ghost(8.0))
                .on_press(Message::StopRecording)
                .into()
            });
            let going: Vec<Element<'a, Message>> = crate::bar::watching(bar, false)
                .into_iter()
                .chain(crate::bar::timers(bar, false))
                .chain(recorded)
                .collect();
            (!going.is_empty()).then(|| {
                Column::with_children(going)
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .into()
            })
        }
        Piece::Updates => crate::bar::updates(bar, false),
        Piece::Stats => stats(bar),
        Piece::Tray => Some(tray(bar)),
        Piece::Apps => {
            let icons = crate::bar::tray_icons(bar);
            (!icons.is_empty()).then(|| {
                Column::with_children(icons)
                    .spacing(2)
                    .align_x(Alignment::Center)
                    .into()
            })
        }
        Piece::Modules => {
            let loose: Vec<Element<'a, Message>> = bar
                .modules
                .iter()
                .filter(|m| m.name.is_none())
                .map(|m| module(m, bar.palette, false))
                .collect();
            (!loose.is_empty()).then(|| Column::with_children(loose).spacing(4).into())
        }
        Piece::Module(name) => bar
            .modules
            .iter()
            .find(|m| m.name == Some(name.as_str()))
            .map(|m| module(m, bar.palette, false)),
    }
}

// What a thing is, said on hover since there's no room to write it.
fn hint<'a>(content: impl Into<Element<'a, Message>>, words: String) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(words).size(12))
            .padding([4, 8])
            .style(style::surface),
        tooltip::Position::Right,
    )
    .into()
}

fn mode<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    if !bar.has_modes {
        return None;
    }
    let (glyph, name, tint) = match bar.mode {
        Some(mode) => (
            Icon::named(&mode.icon),
            mode.name.clone(),
            bar.palette.accent,
        ),
        None => (Icon::Circle, "No mode".to_owned(), neutral::muted()),
    };
    let soft = if bar.mode.is_some() {
        bar.palette.accent_soft
    } else {
        style::fill_color()
    };
    Some(hint(
        button(icon(glyph, 16.0, tint))
            .padding(8)
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(style::color(match status {
                    button::Status::Hovered | button::Status::Pressed => tint.alpha(0.28),
                    _ => soft,
                }))),
                border: iced::Border {
                    radius: style::PILL.into(),
                    ..iced::Border::default()
                },
                ..button::Style::default()
            })
            .on_press(Message::Overlay(Some(Overlay::Switcher))),
        name,
    ))
}

fn workspaces<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let palette = bar.palette;
    Column::with_children((0..bar.status.workspaces).map(|i| {
        let active = i == bar.active;
        let busy = bar.status.busy.contains(&i);
        let mark: Element<'a, Message> = match bar.look.workspaces {
            Workspaces::Dots => {
                let (height, fill) = match (active, busy) {
                    (true, _) => (18.0, palette.accent),
                    (false, true) => (6.0, neutral::text().alpha(0.72)),
                    (false, false) => (6.0, neutral::text().alpha(0.26)),
                };
                container(space())
                    .width(6)
                    .height(height)
                    .style(style::pill(fill))
                    .into()
            }
            Workspaces::Numbers => {
                let ink = match (active, busy) {
                    (true, _) => palette.accent,
                    (false, true) => neutral::text(),
                    (false, false) => neutral::text().alpha(0.45),
                };
                text((i + 1).to_string())
                    .size(12)
                    .font(style::mono())
                    .color(style::color(ink))
                    .into()
            }
        };
        let tall = match (bar.look.workspaces, active) {
            (Workspaces::Dots, true) => 30.0,
            (Workspaces::Dots, false) => 18.0,
            (Workspaces::Numbers, _) => 24.0,
        };
        let highlight = active && bar.look.workspaces == Workspaces::Numbers;
        button(container(mark).center_x(28).center_y(tall))
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
    .align_x(Alignment::Center)
    .into()
}

// Pinned apps, then the open windows, one under another.
fn windows<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let palette = bar.palette;
    let pinned_ids: Vec<&str> = bar.pinned.iter().map(|pin| pin.id.as_str()).collect();
    let face = |id: &str, focused: bool, running: bool, press: Message| -> Element<'a, Message> {
        let glyph = crate::windows::picture(bar.apps, id, 18.0, palette);
        let (height, tint) = match (focused, running) {
            (true, _) => (12, palette.accent),
            (false, true) => (4, neutral::muted()),
            (false, false) => (0, neutral::muted()),
        };
        let marker = container(space())
            .width(2)
            .height(height)
            .style(style::pill(tint));
        let row = iced::widget::row![marker, glyph]
            .spacing(3)
            .align_y(Alignment::Center);
        button(row)
            .padding(iced::Padding::new(4.0).left(1.0))
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if focused {
                    style.background = Some(Background::Color(style::color(style::fill_color())));
                }
                style
            })
            .on_press(press)
            .into()
    };
    let pinned = bar.pinned.iter().map(|pin| {
        mouse_area(face(
            &pin.id,
            pin.focused,
            pin.running,
            Message::OpenPinned(pin.id.clone()),
        ))
        .on_right_press(Message::Pin(pin.id.clone()))
        .into()
    });
    let loose = bar
        .status
        .windows
        .iter()
        .filter(|(_, app, _)| !pinned_ids.contains(&app_key(bar.apps, app)))
        .map(|(id, app, focused)| {
            mouse_area(face(app, *focused, true, Message::FocusWindow(*id)))
                .on_middle_press(Message::CloseWindow(*id))
                .on_right_press(Message::Pin(app_key(bar.apps, app).to_owned()))
                .into()
        });
    Column::with_children(pinned.chain(loose))
        .spacing(2)
        .align_x(Alignment::Center)
        .into()
}

// Hours over minutes; the date is the calendar's.
fn clock<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let mut lines = column![
        text(bar.now.format(bar.clock.hours()).to_string())
            .size(15)
            .font(style::semibold()),
        text(bar.now.format("%M").to_string())
            .size(15)
            .font(style::semibold()),
    ]
    .spacing(0)
    .align_x(Alignment::Center);
    if bar.look.seconds {
        lines = lines.push(
            text(bar.now.format("%S").to_string())
                .size(11)
                .color(style::color(neutral::muted())),
        );
    }
    if bar.next.is_some() {
        lines = lines
            .push(container(space().width(6).height(6)).style(style::pill(bar.palette.accent)));
    }
    let words = match bar.next {
        Some((what, wait)) => crate::events::when(what, wait),
        None => bar
            .now
            .format(&crate::i18n::names("%A, %-d %B", &bar.now))
            .to_string(),
    };
    hint(
        button(lines)
            .padding([6, 4])
            .style(style::ghost(8.0))
            .on_press(Message::Overlay(Some(Overlay::Calendar))),
        words,
    )
}

fn bell<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    if bar.held == 0 && !bar.do_not_disturb {
        return None;
    }
    let glyph = if bar.do_not_disturb {
        Icon::BellOff
    } else {
        Icon::Bell
    };
    let mut bell = column![icon(glyph, 15.0, neutral::text())]
        .spacing(3)
        .align_x(Alignment::Center);
    if bar.held > 0 {
        bell = bell.push(
            container(
                text(bar.held.to_string())
                    .size(10)
                    .font(style::semibold())
                    .color(style::color(bar.palette.on_accent)),
            )
            .padding([0, 5])
            .style(style::pill(bar.palette.accent)),
        );
    }
    Some(
        button(bell)
            .padding(6)
            .style(style::ghost(style::PILL))
            .on_press(Message::Overlay(Some(Overlay::Panel)))
            .into(),
    )
}

fn stats<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    let shown = bar.look.stats;
    let figures: Vec<Element<'a, Message>> = [
        (shown.cpu, Icon::Cpu, bar.stats.cpu.map(|v| format!("{v}"))),
        (
            shown.memory,
            Icon::MemoryStick,
            bar.stats.memory.map(|v| format!("{v}")),
        ),
        (
            shown.temperature,
            Icon::Thermometer,
            bar.stats.temperature.map(|v| format!("{v}°")),
        ),
    ]
    .into_iter()
    .filter_map(|(on, glyph, value)| {
        let value = value.filter(|_| on)?;
        Some(
            column![
                icon(glyph, 13.0, neutral::muted()),
                text(value)
                    .size(10)
                    .font(style::mono())
                    .color(style::color(neutral::muted())),
            ]
            .spacing(2)
            .align_x(Alignment::Center)
            .into(),
        )
    })
    .collect();
    (!figures.is_empty()).then(|| {
        button(
            Column::with_children(figures)
                .spacing(8)
                .align_x(Alignment::Center),
        )
        .padding([6, 4])
        .style(style::ghost(8.0))
        .on_press(Message::Overlay(Some(Overlay::Machine)))
        .into()
    })
}

// The quick settings button, its icons one under another and the battery's figure under it.
fn tray<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let items = bar.look.tray.iter().filter_map(|item| match item {
        TrayItem::Battery => bar.battery.map(|battery| {
            let shade = match battery {
                Battery { charging: true, .. } => bar.palette.accent,
                Battery {
                    percent: 0..=10, ..
                } => style::DANGER,
                _ => neutral::text(),
            };
            let mut level = column![icon(
                Icon::battery(battery.percent, battery.charging),
                17.0,
                shade
            )]
            .spacing(1)
            .align_x(Alignment::Center);
            if bar.look.battery_percent {
                level = level.push(
                    text(battery.percent.to_string())
                        .size(10)
                        .font(style::medium())
                        .color(style::color(shade)),
                );
            }
            level.into()
        }),
        item => tray_item(bar, *item),
    });
    let open = bar.panel_open;
    button(
        Column::with_children(items)
            .spacing(10)
            .align_x(Alignment::Center),
    )
    .padding([10, 5])
    .style(move |theme, status| {
        let mut style = style::ghost(style::PILL)(theme, status);
        if open {
            style.background = Some(Background::Color(style::color(style::hover_color())));
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

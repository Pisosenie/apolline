use crate::style::text;
use apolline_config::{ModeSettings, Piece, TrayItem, Workspaces};
use apolline_theme::{Palette, metrics, neutral};
use chrono::{DateTime, Local};
use iced::widget::{Row, button, column, container, mouse_area, row, space};
use iced::{Alignment, Background, Element, Fill, Theme};

use crate::apps::{self, App};
use crate::controls::Controls;
use crate::drag::drag;
use crate::icon::{Icon, icon};
use crate::media::Track;
use crate::power::Battery;
use crate::shell::{Message, Overlay};
use crate::status::Status;
use crate::strip;
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
    pub look: &'a apolline_config::Bar,
    // The island's corners, when it floats.
    pub radius: f32,
    pub panel_open: bool,
    pub apps: &'a [App],
    pub pinned: Vec<Pinned>,
    // An app being dragged on the row of pinned apps: whether it was pinned, and where it is.
    pub dragging: Option<(&'a str, bool, f32)>,
    // What apps keep in the tray, and the icon theme to draw them from.
    pub tray: &'a [crate::tray::Item],
    pub icons: Option<&'a str>,
    pub modules: Vec<Module<'a>>,
    pub recording: Option<std::time::Duration>,
    pub stats: crate::stats::Stats,
    // The calendar's next thing, and in how many minutes (None: it's on now).
    pub next: Option<(&'a str, Option<i64>)>,
}

pub struct Pinned {
    pub id: String,
    pub running: bool,
    pub focused: bool,
}

// A [[module]]'s latest line.
pub struct Module<'a> {
    pub index: usize,
    pub name: Option<&'a str>,
    pub text: &'a str,
    pub clickable: bool,
}

pub fn view(bar: Bar<'_>) -> Element<'_, Message> {
    if bar.look.position.vertical() {
        return crate::rail::view(bar);
    }
    let look = bar.look;
    let islands = look.shape == apolline_config::Shape::Islands;
    let zone = |pieces: &[Piece], spacing: f32| -> Element<'_, Message> {
        let parts: Vec<Element<'_, Message>> =
            pieces.iter().filter_map(|p| piece(&bar, p)).collect();
        let empty = parts.is_empty();
        let row = Row::with_children(parts)
            .spacing(spacing)
            .align_y(Alignment::Center);
        if !islands {
            return row.into();
        }
        if empty {
            return space().into();
        }
        // Each side its own pill of glass, as tall as the bar.
        container(row)
            .height(Fill)
            .padding([0, 5])
            .align_y(Alignment::Center)
            .style(style::glass(look.opacity, true, bar.radius))
            .into()
    };
    // With nothing in the middle, the right side keeps what it needs and the left has the rest,
    // instead of half each.
    let sides = if look.center.is_empty() {
        row![
            container(zone(&look.left, 10.0)).width(Fill),
            zone(&look.right, 6.0),
        ]
    } else {
        row![
            container(zone(&look.left, 10.0)).width(Fill),
            zone(&look.center, 10.0),
            container(zone(&look.right, 6.0))
                .width(Fill)
                .align_x(Alignment::End),
        ]
    };
    let strip = container(sides.align_y(Alignment::Center))
        .width(Fill)
        .height(metrics::BAR_HEIGHT)
        .align_y(Alignment::Center);
    if islands {
        strip.into()
    } else {
        strip
            .padding([0, 5])
            .style(style::glass(look.opacity, look.floating, bar.radius))
            .into()
    }
}

fn piece<'a>(bar: &Bar<'a>, piece: &Piece) -> Option<Element<'a, Message>> {
    match piece {
        Piece::Mode => mode(bar),
        Piece::Workspaces => Some(workspaces(bar)),
        Piece::Windows => Some(tasks(bar)),
        Piece::Clock => Some(clock(bar)),
        Piece::Media => media(bar),
        Piece::Notifications => bell(bar),
        Piece::Recording => recording(bar),
        Piece::Stats => stats(bar),
        Piece::Tray => Some(tray(bar)),
        Piece::Apps => apps_tray(bar),
        // The ones placed by name elsewhere stay there.
        Piece::Modules => {
            // A named one stands only where it's placed; left off, it's off.
            let loose: Vec<Element<'a, Message>> = bar
                .modules
                .iter()
                .filter(|m| m.name.is_none())
                .map(module)
                .collect();
            (!loose.is_empty()).then(|| Row::with_children(loose).spacing(6).into())
        }
        Piece::Module(name) => bar
            .modules
            .iter()
            .find(|m| m.name == Some(name.as_str()))
            .map(module),
    }
}

fn mode<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    if !bar.has_modes {
        return None;
    }
    let palette = bar.palette;
    let (glyph, name, tint) = match bar.mode {
        Some(mode) => (Icon::named(&mode.icon), mode.name.as_str(), palette.accent),
        None => (Icon::Circle, "No mode", neutral::muted()),
    };
    let soft = if bar.mode.is_some() {
        palette.accent_soft
    } else {
        style::fill_color()
    };
    Some(
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
        .into(),
    )
}

fn clock<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let mut clock = row![].spacing(10).align_y(Alignment::Center);
    if bar.look.date {
        clock = clock.push(
            text(bar.now.format("%a %-d %b").to_string())
                .size(13)
                .color(style::color(neutral::muted())),
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
    if let Some((what, wait)) = bar.next {
        clock = clock.push(
            row![
                container(space().width(6).height(6)).style(style::pill(bar.palette.accent)),
                text(crate::events::when(&crate::media::shorten(what, 22), wait))
                    .size(12)
                    .color(style::color(neutral::muted())),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    button(clock)
        .padding([4, 10])
        .style(style::ghost(8.0))
        .on_press(Message::Overlay(Some(Overlay::Calendar)))
        .into()
}

fn media<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    let track = bar.track.filter(|t| t.playing)?;
    Some(
        button(
            row![
                icon(Icon::Music, 13.0, bar.palette.accent),
                text(crate::media::shorten(&track.line(), 34))
                    .size(12)
                    .color(style::color(neutral::muted())),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([4, 10])
        .style(style::ghost(style::PILL))
        .on_press(Message::Overlay(Some(Overlay::Panel)))
        .into(),
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
    let mut bell = row![icon(glyph, 15.0, neutral::text())]
        .spacing(5)
        .align_y(Alignment::Center);
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
            .padding([5, 8])
            .style(style::ghost(style::PILL))
            .on_press(Message::Overlay(Some(Overlay::Panel)))
            .into(),
    )
}

fn recording<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    let secs = bar.recording?.as_secs();
    Some(
        button(
            row![
                container(space())
                    .width(8)
                    .height(8)
                    .style(style::pill(style::DANGER)),
                text(format!("{}:{:02}", secs / 60, secs % 60))
                    .size(12)
                    .font(style::mono()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .style(style::ghost(8.0))
        .on_press(Message::StopRecording)
        .into(),
    )
}

pub fn module<'a>(module: &Module<'a>) -> Element<'a, Message> {
    let label = text(module.text.to_owned())
        .size(12)
        .color(style::color(neutral::muted()));
    button(label)
        .padding([4, 8])
        .style(style::ghost(8.0))
        .on_press_maybe(
            module
                .clickable
                .then_some(Message::ModuleClick(module.index)),
        )
        .into()
}

fn stats<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    let shown = bar.look.stats;
    let figures: Vec<Element<'a, Message>> = [
        (shown.cpu, Icon::Cpu, bar.stats.cpu.map(|v| format!("{v}%"))),
        (
            shown.memory,
            Icon::MemoryStick,
            bar.stats.memory.map(|v| format!("{v}%")),
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
            row![
                icon(glyph, 13.0, neutral::muted()),
                text(value)
                    .size(12)
                    .font(style::mono())
                    .color(style::color(neutral::muted())),
            ]
            .spacing(5)
            .align_y(Alignment::Center)
            .padding([0, 4])
            .into(),
        )
    })
    .collect();
    (!figures.is_empty()).then(|| {
        Row::with_children(figures)
            .spacing(6)
            .align_y(Alignment::Center)
            .into()
    })
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
                    (false, true) => (6.0, neutral::text().alpha(0.72)),
                    (false, false) => (6.0, neutral::text().alpha(0.26)),
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

// Pinned apps, then the open windows; any of them can be dragged among the pinned ones to pin it
// there, and a pinned one dragged well past them is unpinned.
fn tasks<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let palette = bar.palette;
    let ids: Vec<String> = bar.pinned.iter().map(|pin| pin.id.clone()).collect();
    let shown = match bar.dragging {
        Some((app, was_pinned, x)) => strip::landing(&ids, app, was_pinned, x),
        None => ids.clone(),
    };
    let lifted = bar.dragging.map(|(app, ..)| app);
    let loose_of = |pinned: &[String]| -> Vec<(u64, String, bool)> {
        bar.status
            .windows
            .iter()
            .filter(|(_, app, _)| !pinned.iter().any(|id| id == app_key(bar.apps, app)))
            .cloned()
            .collect()
    };
    let loose = loose_of(&shown);
    let pinned = shown.iter().map(|id| {
        let pin = bar.pinned.iter().find(|pin| pin.id == *id);
        let (running, focused) = pin.map_or((true, false), |pin| (pin.running, pin.focused));
        let glyph = crate::windows::picture(bar.apps, id, 16.0, palette);
        let (width, tint) = match (focused, running) {
            (true, _) => (12, palette.accent),
            (false, true) => (4, neutral::muted()),
            (false, false) => (0, neutral::muted()),
        };
        let marker = container(space())
            .width(width)
            .height(2)
            .style(style::pill(tint));
        let held = lifted == Some(id.as_str());
        let button = button(column![glyph, marker].spacing(2).align_x(Alignment::Center))
            .padding(iced::Padding::new(4.0).bottom(1.0))
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if held {
                    style.background = Some(Background::Color(style::color(palette.accent_soft)));
                } else if focused {
                    style.background = Some(Background::Color(style::color(style::fill_color())));
                }
                style
            })
            .on_press(Message::OpenPinned(id.clone()));
        mouse_area(button)
            .on_right_press(Message::Pin(id.clone()))
            .into()
    });
    let gap = (!shown.is_empty() && !loose.is_empty()).then(|| {
        container(space())
            .width(1)
            .height(14)
            .style(|_| container::background(style::color(neutral::hairline())))
            .into()
    });
    let windows = loose.into_iter().map(|(id, app, focused)| {
        let glyph = crate::windows::picture(bar.apps, &app, 16.0, palette);
        let marker = container(space())
            .width(if focused { 12 } else { 0 })
            .height(2)
            .style(style::pill(palette.accent));
        let task = button(column![glyph, marker].spacing(2).align_x(Alignment::Center))
            .padding(iced::Padding::new(4.0).bottom(1.0))
            .style(move |theme, status| {
                let mut style = style::ghost(7.0)(theme, status);
                if focused {
                    style.background = Some(Background::Color(style::color(style::fill_color())));
                }
                style
            })
            .on_press(Message::FocusWindow(id));
        mouse_area(task)
            .on_middle_press(Message::CloseWindow(id))
            .on_right_press(Message::Pin(app_key(bar.apps, &app).to_owned()))
            .into()
    });
    let row = Row::with_children(pinned.chain(gap).chain(windows))
        .spacing(2)
        .align_y(Alignment::Center);
    // What a press lands on, as the row stands before any drag.
    let loose: Vec<String> = loose_of(&ids)
        .into_iter()
        .map(|(_, app, _)| app_key(bar.apps, &app).to_owned())
        .collect();
    drag(
        row,
        move |at| {
            let picked = strip::hit(at.x, ids.len(), loose.len()).map(|hit| match hit {
                strip::Hit::Pinned(i) => (ids[i].clone(), true),
                strip::Hit::Window(i) => (loose[i].clone(), false),
            });
            Message::StripStart(picked, at.x)
        },
        |at| Message::StripMove(at.x),
        |at| Message::StripDrop(at.x),
    )
    .into()
}

pub fn app_key<'a>(apps: &'a [App], app_id: &'a str) -> &'a str {
    apps::find(apps, app_id).map_or(app_id, |app| app.id.as_str())
}

fn tray<'a>(bar: &Bar<'a>) -> Element<'a, Message> {
    let items = bar
        .look
        .tray
        .iter()
        .filter_map(|item| tray_item(bar, *item));
    let items = Row::with_children(items)
        .spacing(10)
        .align_y(Alignment::Center);
    let open = bar.panel_open;
    button(items)
        .padding([5, 11])
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

pub fn tray_item<'a>(bar: &Bar<'a>, item: TrayItem) -> Option<Element<'a, Message>> {
    let tint = neutral::text();
    match item {
        TrayItem::Keyboard => (!bar.status.keyboard.is_empty()).then(|| {
            text(bar.status.keyboard.clone())
                .size(11)
                .font(style::semibold())
                .color(style::color(neutral::muted()))
                .into()
        }),
        TrayItem::Awake => bar
            .awake
            .then(|| icon(Icon::Coffee, 15.0, bar.palette.accent).into()),
        TrayItem::Wifi => bar.controls.wifi.as_ref().map(|wifi| {
            let glyph = if wifi.on && wifi.connected.is_some() {
                Icon::Wifi
            } else {
                Icon::WifiOff
            };
            icon(glyph, 15.0, tint).into()
        }),
        TrayItem::Bluetooth => bar
            .controls
            .bluetooth
            .as_ref()
            .filter(|b| b.on)
            .map(|bluetooth| {
                let shade = if bluetooth.connected.is_some() {
                    tint
                } else {
                    neutral::muted()
                };
                icon(Icon::Bluetooth, 15.0, shade).into()
            }),
        TrayItem::Volume => bar
            .controls
            .volume
            .map(|volume| icon(Icon::volume(volume.level, volume.muted), 15.0, tint).into()),
        TrayItem::Battery => bar.battery.map(|battery| {
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
            level.into()
        }),
    }
}

// Each app's tray icon: a click is the app's own, a right click its menu.
fn apps_tray<'a>(bar: &Bar<'a>) -> Option<Element<'a, Message>> {
    let icons = tray_icons(bar);
    (!icons.is_empty()).then(|| {
        Row::with_children(icons)
            .spacing(0)
            .align_y(Alignment::Center)
            .into()
    })
}

pub fn tray_icons<'a>(bar: &Bar<'a>) -> Vec<Element<'a, Message>> {
    let palette = bar.palette;
    bar.tray
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let glyph: Element<'a, Message> = match &item.icon {
                crate::tray::Icon::Pixels(w, h, rgba) => iced::widget::image(
                    iced::widget::image::Handle::from_rgba(*w, *h, rgba.clone()),
                )
                .width(16)
                .height(16)
                .into(),
                crate::tray::Icon::Named(name, folder) => {
                    match crate::apps::icon_named(name, bar.icons, folder.as_deref()) {
                        Some(apps::Picture::Vector(path)) => {
                            iced::widget::svg(iced::widget::svg::Handle::from_path(path))
                                .width(16)
                                .height(16)
                                .into()
                        }
                        Some(apps::Picture::Raster(path)) => {
                            iced::widget::image(iced::widget::image::Handle::from_path(path))
                                .width(16)
                                .height(16)
                                .into()
                        }
                        None => crate::windows::picture(bar.apps, name, 16.0, palette),
                    }
                }
                crate::tray::Icon::None => {
                    crate::windows::picture(bar.apps, &item.title, 16.0, palette)
                }
            };
            let attention = item.attention;
            let glyph = container(glyph).padding(1).style(move |_| {
                if attention {
                    container::Style {
                        border: iced::Border {
                            color: style::color(palette.accent),
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                        ..container::Style::default()
                    }
                } else {
                    container::Style::default()
                }
            });
            let press = if item.menu_only {
                Message::TrayMenu(index)
            } else {
                Message::TrayPress(index, crate::tray::Press::Activate)
            };
            let button = button(glyph)
                .padding(4)
                .style(style::ghost(7.0))
                .on_press(press);
            iced::widget::tooltip(
                mouse_area(button)
                    .on_right_press(Message::TrayMenu(index))
                    .on_middle_press(Message::TrayPress(index, crate::tray::Press::Secondary)),
                container(text(item.title.clone()).size(12))
                    .padding([4, 8])
                    .style(style::surface),
                iced::widget::tooltip::Position::Bottom,
            )
            .into()
        })
        .collect()
}

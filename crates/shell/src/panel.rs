use apolline_config::ModeSettings;
use apolline_theme::{Palette, metrics, neutral};
use iced::widget::{
    Column, Row, button, column, container, mouse_area, opaque, row, scrollable, slider, space,
    text,
};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::controls::{Change, Controls, Network, Paired, Radio};
use crate::icon::{Icon, icon};
use crate::media::{Action, Track, shorten};
use crate::notify::Notification;
use crate::popup;
use crate::power::Battery;
use crate::shell::Message;
use crate::style;

pub const WIDTH: f32 = 384.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Lock,
    LogOut,
    Restart,
    Shutdown,
}

impl Power {
    pub fn command(self) -> String {
        match self {
            Power::Lock => format!("{} lock", crate::control::me()),
            Power::LogOut => "niri msg action quit".into(),
            Power::Restart => "systemctl reboot".into(),
            Power::Shutdown => "systemctl poweroff".into(),
        }
    }

    pub fn needs_second_click(self) -> bool {
        matches!(self, Power::Restart | Power::Shutdown)
    }

    fn label(self) -> &'static str {
        match self {
            Power::Lock => "Lock",
            Power::LogOut => "Log out",
            Power::Restart => "Restart",
            Power::Shutdown => "Power off",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Power::Lock => Icon::Lock,
            Power::LogOut => Icon::LogOut,
            Power::Restart => Icon::Restart,
            Power::Shutdown => Icon::Power,
        }
    }
}

pub struct Panel<'a> {
    pub at_bottom: bool,
    pub modes: &'a [ModeSettings],
    pub mode: Option<usize>,
    pub palette: Palette,
    pub controls: &'a Controls,
    pub battery: Option<Battery>,
    pub track: Option<&'a Track>,
    pub do_not_disturb: bool,
    pub power_menu: bool,
    pub armed: Option<Power>,
    pub awake: bool,
    pub history: &'a [Notification],
    pub detail: Option<Detail>,
    pub networks: &'a [Network],
    pub paired: &'a [Paired],
    pub joining: Option<&'a (Network, String)>,
    pub note: Option<&'a str>,
    pub busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Wifi,
    Bluetooth,
}

pub const PASSWORD: &str = "wifi-password";

pub fn view(panel: Panel<'_>) -> Element<'_, Message> {
    let palette = panel.palette;
    let mut body = column![header(&panel)].spacing(14);
    if panel.power_menu {
        body = body.push(power_menu(panel.armed));
    }

    if let Some(detail) = panel.detail {
        body = body.push(match detail {
            Detail::Wifi => wifi_list(&panel),
            Detail::Bluetooth => paired_list(&panel),
        });
        return frame(body, panel.at_bottom);
    }

    let wifi = radio_tile(
        "Wi-Fi",
        panel.controls.wifi.as_ref(),
        (Icon::Wifi, Icon::WifiOff),
        palette,
        Change::Wifi,
        Detail::Wifi,
    );
    let bluetooth = radio_tile(
        "Bluetooth",
        panel.controls.bluetooth.as_ref(),
        (Icon::Bluetooth, Icon::BluetoothOff),
        palette,
        Change::Bluetooth,
        Detail::Bluetooth,
    );
    let quiet = tile(
        if panel.do_not_disturb {
            Icon::BellOff
        } else {
            Icon::Bell
        },
        "Do not disturb",
        if panel.do_not_disturb {
            "Only urgent"
        } else {
            "Off"
        }
        .to_owned(),
        panel.do_not_disturb,
        palette,
        Some(Message::DoNotDisturb(!panel.do_not_disturb)),
    );
    let awake = tile(
        Icon::Coffee,
        "Keep awake",
        if panel.awake {
            "No lock, no sleep"
        } else {
            "Off"
        }
        .to_owned(),
        panel.awake,
        palette,
        Some(Message::KeepAwake(!panel.awake)),
    );
    body = body.push(
        column![
            row![wifi, bluetooth].spacing(10),
            row![quiet, awake].spacing(10),
        ]
        .spacing(10),
    );

    let mut sliders = column![].spacing(6);
    if let Some(volume) = panel.controls.volume {
        let level = if volume.muted { 0.0 } else { volume.level };
        sliders = sliders.push(slider_row(
            icon(
                Icon::volume(volume.level, volume.muted),
                18.0,
                neutral::TEXT,
            ),
            Some(Message::Change(Change::Mute(!volume.muted))),
            level,
            palette,
            |v| Message::Change(Change::Volume(v)),
        ));
    }
    for (devices, glyph, pick) in [
        (
            &panel.controls.outputs,
            Icon::Headphones,
            Change::Output as fn(u32) -> Change,
        ),
        (
            &panel.controls.inputs,
            Icon::Mic,
            Change::Input as fn(u32) -> Change,
        ),
    ] {
        if devices.len() > 1 {
            sliders = sliders.push(devices_row(devices, glyph, pick));
        }
    }
    if let Some(brightness) = panel.controls.brightness {
        sliders = sliders.push(slider_row(
            icon(Icon::Sun, 18.0, neutral::TEXT),
            None,
            f32::from(brightness) / 100.0,
            palette,
            |v| Message::Change(Change::Brightness((v * 100.0).round().max(1.0) as u8)),
        ));
    }
    body = body.push(sliders);

    if !panel.modes.is_empty() {
        body = body.push(modes(panel.modes, panel.mode, palette));
    }

    if let Some(track) = panel.track {
        body = body.push(player(track, palette));
    }
    body = body.push(history(panel.history));

    frame(body, panel.at_bottom)
}

fn header<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let battery: Element<'a, Message> = match panel.battery {
        Some(battery) => {
            let tint = if battery.charging {
                panel.palette.accent
            } else {
                neutral::TEXT
            };
            let mut line = row![
                icon(Icon::battery(battery.percent, battery.charging), 18.0, tint),
                text(format!("{}%", battery.percent))
                    .size(14)
                    .font(style::medium()),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            if battery.charging {
                line = line.push(text("charging").size(12).color(style::color(style::FAINT)));
            }
            line.into()
        }
        None => text("Apolline").size(14).font(style::medium()).into(),
    };
    let round = |glyph: Icon, message: Message, on: bool| {
        button(container(icon(glyph, 16.0, neutral::TEXT)).center(34))
            .padding(0)
            .style(move |theme, status| {
                let mut style = style::secondary(theme, status);
                style.border.radius = style::PILL.into();
                if on {
                    style.background = Some(iced::Background::Color(style::color(style::HOVER)));
                }
                style
            })
            .on_press(message)
    };
    row![
        battery,
        space::horizontal(),
        row![
            round(Icon::Camera, Message::Screenshot, false),
            round(Icon::Settings, Message::OpenSettings, false),
            round(Icon::Lock, Message::Power(Power::Lock), false),
            round(
                Icon::Power,
                Message::PowerMenu(!panel.power_menu),
                panel.power_menu
            ),
        ]
        .spacing(8),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn power_menu<'a>(armed: Option<Power>) -> Element<'a, Message> {
    let entries = [Power::Lock, Power::LogOut, Power::Restart, Power::Shutdown];
    let buttons = Row::with_children(entries.map(|power| {
        let is_armed = armed == Some(power);
        let label = if is_armed { "Sure?" } else { power.label() };
        let tint = if is_armed {
            neutral::INK
        } else {
            neutral::TEXT
        };
        button(
            column![icon(power.icon(), 16.0, tint), text(label).size(11)]
                .spacing(6)
                .align_x(Alignment::Center),
        )
        .width(Fill)
        .padding([10, 4])
        .style(move |theme, status| {
            let mut style = style::danger(is_armed)(theme, status);
            style.border.radius = style::RADIUS_TILE.into();
            style
        })
        .on_press(Message::Power(power))
        .into()
    }))
    .spacing(8);
    buttons.into()
}

fn radio_tile<'a>(
    title: &'static str,
    radio: Option<&Radio>,
    glyphs: (Icon, Icon),
    palette: Palette,
    change: fn(bool) -> Change,
    detail: Detail,
) -> Element<'a, Message> {
    match radio {
        Some(radio) => {
            let subtitle = match (&radio.connected, radio.on) {
                (Some(name), true) => name.clone(),
                (None, true) => "On".to_owned(),
                (_, false) => "Off".to_owned(),
            };
            let main = tile(
                if radio.on { glyphs.0 } else { glyphs.1 },
                title,
                subtitle,
                radio.on,
                palette,
                Some(Message::Change(change(!radio.on))),
            );
            if !radio.on {
                return main;
            }
            let ink = palette.on_accent;
            let more = button(icon(Icon::ChevronRight, 16.0, ink))
                .padding(8)
                .style(style::ghost(10.0))
                .on_press(Message::Detail(Some(detail)));
            iced::widget::stack![
                main,
                container(more)
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::End)
                    .align_y(Alignment::Center)
                    .padding(Padding::new(0.0).right(6.0)),
            ]
            .into()
        }
        None => tile(
            glyphs.1,
            title,
            "Unavailable".to_owned(),
            false,
            palette,
            None,
        ),
    }
}

fn tile<'a>(
    glyph: Icon,
    title: &'static str,
    subtitle: String,
    on: bool,
    palette: Palette,
    message: Option<Message>,
) -> Element<'a, Message> {
    let ink = if on { palette.on_accent } else { neutral::TEXT };
    let faint = if on {
        palette.on_accent.alpha(0.72)
    } else {
        style::FAINT
    };
    let content = row![
        icon(glyph, 18.0, ink),
        column![
            text(title)
                .size(13)
                .font(style::semibold())
                .color(style::color(ink)),
            text(crate::media::shorten(&subtitle, 18))
                .size(12)
                .color(style::color(faint)),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    button(container(content).height(46).align_y(Alignment::Center))
        .width(Fill)
        .padding([6, 14])
        .style(style::tile(on, palette))
        .on_press_maybe(message)
        .into()
}

fn slider_row<'a>(
    glyph: iced::widget::Svg<'a>,
    on_icon: Option<Message>,
    value: f32,
    palette: Palette,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        button(container(glyph).center(32))
            .padding(0)
            .style(style::ghost(style::PILL))
            .on_press_maybe(on_icon),
        slider(0.0..=1.0, value, on_change)
            .step(0.01_f32)
            .height(22)
            .style(style::rail(palette, 6.0)),
        text(format!("{:.0}", value * 100.0))
            .size(12)
            .font(style::mono())
            .color(style::color(style::FAINT))
            .width(Length::Fixed(28.0))
            .align_x(Alignment::End),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn modes<'a>(
    modes: &'a [ModeSettings],
    current: Option<usize>,
    palette: Palette,
) -> Element<'a, Message> {
    let chip = |label: &'a str, glyph: Icon, target: Option<usize>| {
        let on = target == current;
        let ink = if on { palette.on_accent } else { neutral::TEXT };
        button(
            row![
                icon(glyph, 15.0, ink),
                text(label).size(13).font(style::medium())
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([9, 12])
        .style(move |theme, status| {
            let mut style = style::tile(on, palette)(theme, status);
            style.border.radius = 10.0.into();
            style
        })
        .on_press(Message::Mode(target))
        .into()
    };
    let mut chips: Vec<Element<'a, Message>> = vec![chip("None", Icon::Circle, None)];
    chips.extend(
        modes
            .iter()
            .enumerate()
            .map(|(index, mode)| chip(&mode.name, Icon::named(&mode.icon), Some(index))),
    );
    column![
        text("MODE")
            .size(10)
            .font(style::semibold())
            .color(style::color(style::FAINT)),
        Row::with_children(chips)
            .spacing(6)
            .wrap()
            .vertical_spacing(6),
    ]
    .spacing(8)
    .into()
}

fn player<'a>(track: &'a Track, palette: Palette) -> Element<'a, Message> {
    let control = |glyph: Icon, action: Action| {
        button(container(icon(glyph, 16.0, neutral::TEXT)).center(32))
            .padding(0)
            .style(style::ghost(style::PILL))
            .on_press(Message::Player(action))
    };
    let toggle = button(
        container(icon(
            if track.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            16.0,
            palette.on_accent,
        ))
        .center(36),
    )
    .padding(0)
    .style(move |theme, status| {
        let mut style = style::primary(palette)(theme, status);
        style.border.radius = style::PILL.into();
        style
    })
    .on_press(Message::Player(Action::Toggle));

    let mut words = column![
        text(crate::media::shorten(&track.title, 26))
            .size(13)
            .font(style::semibold())
    ]
    .spacing(2);
    if let Some(artist) = &track.artist {
        words = words.push(
            text(crate::media::shorten(artist, 30))
                .size(12)
                .color(style::color(style::FAINT)),
        );
    }
    container(
        row![
            container(icon(Icon::Music, 18.0, palette.accent))
                .center(40)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(style::color(palette.accent_soft))),
                    border: iced::Border {
                        radius: 10.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
            words,
            space::horizontal(),
            row![
                control(Icon::SkipBack, Action::Previous),
                toggle,
                control(Icon::SkipForward, Action::Next)
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .style(style::card)
    .into()
}

fn history<'a>(items: &'a [Notification]) -> Element<'a, Message> {
    let mut header = row![
        text("NOTIFICATIONS")
            .size(10)
            .font(style::semibold())
            .color(style::color(style::FAINT)),
        space::horizontal(),
    ]
    .align_y(Alignment::Center);
    if !items.is_empty() {
        header = header.push(
            button(text("Clear").size(11).color(style::color(style::FAINT)))
                .padding([2, 8])
                .style(style::ghost(6.0))
                .on_press(Message::ClearAll),
        );
    }
    let list: Element<'a, Message> = if items.is_empty() {
        text("Nothing new")
            .size(12)
            .color(style::color(style::FAINT))
            .into()
    } else {
        let entries = Column::with_children(items.iter().map(entry)).spacing(6);
        container(
            scrollable(entries)
                .direction(style::thin())
                .style(style::scroller),
        )
        .max_height(270)
        .into()
    };
    column![header, list].spacing(8).into()
}

fn entry(notification: &Notification) -> Element<'_, Message> {
    let mut words = column![
        row![
            text(shorten(&notification.app, 28))
                .size(11)
                .color(style::color(style::FAINT)),
            space::horizontal(),
            text(popup::ago(notification.received.elapsed()))
                .size(11)
                .color(style::color(style::FAINT)),
        ],
        text(shorten(&notification.summary, 40))
            .size(13)
            .font(style::semibold()),
    ]
    .spacing(2)
    .width(Fill);
    if !notification.body.is_empty() {
        words = words.push(
            text(shorten(&notification.body, 48))
                .size(12)
                .color(style::color(style::FAINT)),
        );
    }
    container(
        row![
            words,
            button(icon(Icon::X, 12.0, style::FAINT))
                .padding(5)
                .style(style::ghost(6.0))
                .on_press(Message::Forget(notification.id)),
        ]
        .spacing(8)
        .align_y(Alignment::Start),
    )
    .padding([10, 12])
    .style(style::card)
    .into()
}

fn devices_row<'a>(
    devices: &'a [crate::controls::Device],
    glyph: Icon,
    pick: fn(u32) -> Change,
) -> Element<'a, Message> {
    let chips = Row::with_children(devices.iter().map(|device| {
        let label = text(device.name.as_str())
            .size(12)
            .wrapping(text::Wrapping::None);
        button(label)
            .padding([5, 10])
            .style(style::choice(device.default))
            .on_press_maybe((!device.default).then(|| Message::Change(pick(device.id))))
            .into()
    }))
    .spacing(4)
    .wrap();
    row![container(icon(glyph, 16.0, style::FAINT)).width(26), chips,]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([2, 0])
        .into()
}

fn frame<'a>(body: iced::widget::Column<'a, Message>, at_bottom: bool) -> Element<'a, Message> {
    let card = container(body.width(WIDTH))
        .padding(16)
        .style(style::surface);
    mouse_area(
        container(opaque(card))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::End)
            .align_y(if at_bottom {
                Alignment::End
            } else {
                Alignment::Start
            })
            .padding(if at_bottom {
                Padding::new(metrics::BAR_INSET)
                    .bottom(metrics::BAR_INSET + metrics::BAR_HEIGHT + 8.0)
            } else {
                Padding::new(metrics::BAR_INSET).top(metrics::BAR_INSET + metrics::BAR_HEIGHT + 8.0)
            }),
    )
    .on_press(Message::Overlay(None))
    .into()
}

fn detail_header<'a>(title: &'static str, busy: bool) -> Element<'a, Message> {
    row![
        button(icon(Icon::ChevronLeft, 16.0, neutral::TEXT))
            .padding(6)
            .style(style::ghost(8.0))
            .on_press(Message::Detail(None)),
        text(title).size(15).font(style::semibold()).width(Fill),
        text(if busy { "Working…" } else { "" })
            .size(12)
            .color(style::color(style::FAINT)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn wifi_list<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let palette = panel.palette;
    let mut list = column![detail_header("Wi-Fi", panel.busy)].spacing(4);
    if panel.networks.is_empty() {
        list = list.push(
            text("Looking for networks…")
                .size(12)
                .color(style::color(style::FAINT)),
        );
    }
    for network in panel.networks.iter().take(9) {
        let level = match network.signal {
            0..=30 => 1,
            31..=55 => 2,
            56..=75 => 3,
            _ => 4,
        };
        let lit = if network.active {
            palette.accent
        } else {
            neutral::TEXT
        };
        let bars = row((1..=4).map(|bar| {
            let tint = if bar <= level {
                lit
            } else {
                neutral::TEXT.alpha(0.18)
            };
            container(space())
                .width(3)
                .height(3 * bar + 1)
                .style(style::pill(tint))
                .into()
        }))
        .spacing(2)
        .align_y(Alignment::End);
        let mut line = row![
            container(bars).width(22).height(14).align_y(Alignment::End),
            text(crate::media::shorten(&network.ssid, 28))
                .size(13)
                .width(Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if network.secure {
            line = line.push(icon(Icon::Lock, 13.0, style::FAINT));
        }
        if network.active {
            line = line.push(icon(Icon::Check, 14.0, palette.accent));
        }
        list = list.push(
            button(line)
                .width(Fill)
                .padding([8, 10])
                .style(style::ghost(8.0))
                .on_press_maybe(
                    (!network.active && !panel.busy).then(|| Message::PickNetwork(network.clone())),
                ),
        );
        if let Some((joining, password)) = panel.joining
            && joining.ssid == network.ssid
        {
            list = list.push(
                row![
                    iced::widget::text_input("Password", password)
                        .id(PASSWORD)
                        .secure(true)
                        .on_input(Message::NetPassword)
                        .on_submit(Message::JoinNetwork)
                        .padding([7, 10])
                        .size(13)
                        .style(style::field(palette)),
                    button(text("Join").size(13))
                        .padding([7, 14])
                        .style(style::primary(palette))
                        .on_press_maybe(
                            (!panel.busy && !password.is_empty()).then_some(Message::JoinNetwork)
                        ),
                ]
                .spacing(8)
                .padding([2, 10]),
            );
        }
    }
    if let Some(note) = panel.note {
        list = list.push(text(note).size(12).color(style::color(style::WARN)));
    }
    list.into()
}

fn paired_list<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let palette = panel.palette;
    let mut list = column![detail_header("Bluetooth", panel.busy)].spacing(4);
    if panel.paired.is_empty() {
        list = list.push(
            text("No paired devices yet. Pair one once with bluetoothctl or blueman; after that it lives here.")
                .size(12)
                .color(style::color(style::FAINT)),
        );
    }
    for device in panel.paired {
        let state = if device.connected {
            "Connected"
        } else {
            "Not connected"
        };
        let line = row![
            icon(
                Icon::Bluetooth,
                15.0,
                if device.connected {
                    palette.accent
                } else {
                    neutral::MUTED
                }
            ),
            column![
                text(device.name.as_str()).size(13),
                text(state).size(11).color(style::color(style::FAINT)),
            ]
            .width(Fill),
            text(if device.connected {
                "Disconnect"
            } else {
                "Connect"
            })
            .size(12)
            .color(style::color(palette.accent)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        list = list.push(
            button(line)
                .width(Fill)
                .padding([8, 10])
                .style(style::ghost(8.0))
                .on_press_maybe((!panel.busy).then(|| Message::PairDevice(device.clone()))),
        );
    }
    if let Some(note) = panel.note {
        list = list.push(text(note).size(12).color(style::color(style::WARN)));
    }
    list.into()
}

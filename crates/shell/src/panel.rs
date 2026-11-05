use crate::style::text;
use apolline_config::ModeSettings;
use apolline_theme::{Palette, metrics, neutral};
use iced::widget::{
    Column, Row, button, column, container, mouse_area, opaque, row, scrollable, slider, space,
};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::bluetooth::{Ask, Asking};
use crate::controls::{Change, Controls, Radio};
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
    pub edge: apolline_config::Edge,
    // How far it keeps clear of the bar.
    pub gap: f32,
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
    pub look: apolline_config::Surface,
    pub night: bool,
    pub profiles: Option<&'a crate::profile::Profiles>,
    // When do not disturb turns itself off, if it was set for a while.
    pub quiet_until: Option<chrono::DateTime<chrono::Local>>,
    pub history: &'a [Notification],
    // Apps whose notifications show all, not only the newest.
    pub opened: &'a [String],
    pub apps: &'a [crate::apps::App],
    pub detail: Option<Detail>,
    pub networks: &'a [crate::wifi::Network],
    pub gadgets: &'a [crate::bluetooth::Device],
    pub joining: Option<&'a (crate::wifi::Network, String)>,
    pub asking: Option<&'a Ask>,
    pub pin: &'a str,
    pub note: Option<&'a str>,
    pub busy: bool,
    pub tunnels: &'a [crate::vpn::Tunnel],
    // What it shows and in what order.
    pub layout: &'a apolline_config::Panel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Wifi,
    Bluetooth,
    Quiet,
    Vpn,
}

// How long do not disturb lasts, picked in quick settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quiet {
    For(u32),
    Morning,
    Always,
    Off,
}

pub const PASSWORD: &str = "wifi-password";
pub const PIN: &str = "bluetooth-pin";

pub fn view(panel: Panel<'_>) -> Element<'_, Message> {
    let palette = panel.palette;
    let mut body = column![header(&panel)].spacing(14);
    if panel.power_menu {
        body = body.push(power_menu(panel.armed));
    }

    if let Some(detail) = panel.detail {
        body = body.push(match detail {
            Detail::Wifi => wifi_list(&panel),
            Detail::Bluetooth => gadget_list(&panel),
            Detail::Quiet => quiet_list(&panel),
            Detail::Vpn => vpn_list(&panel),
        });
        return frame(body, panel.edge, panel.gap, panel.look);
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
        match (panel.do_not_disturb, panel.quiet_until) {
            (true, Some(until)) => {
                crate::i18n::fill("Until {time}", &[("time", &until.format("%H:%M"))])
            }
            (true, None) => "Only urgent".to_owned(),
            (false, _) => "Off".to_owned(),
        },
        panel.do_not_disturb,
        palette,
        Some(Message::DoNotDisturb(!panel.do_not_disturb)),
    );
    let quiet = more(quiet, Detail::Quiet, panel.do_not_disturb, palette);
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
    let night = tile(
        Icon::Moon,
        "Night light",
        if panel.night { "Warm" } else { "Off" }.to_owned(),
        panel.night,
        palette,
        Some(Message::NightLight(!panel.night)),
    );
    let mut wifi = Some(wifi);
    let mut bluetooth = Some(bluetooth);
    let mut quiet = Some(quiet);
    let mut awake = Some(awake);
    let mut night = Some(night);
    let mut power = Some(power_tile(panel.profiles, palette));
    let mut vpn = vpn_tile(&panel);
    let shown: Vec<Element<'_, Message>> = panel
        .layout
        .tiles
        .iter()
        .filter_map(|name| match name.as_str() {
            "wifi" => wifi.take(),
            "bluetooth" => bluetooth.take(),
            "quiet" => quiet.take(),
            "awake" => awake.take(),
            "night" => night.take(),
            "power" => power.take(),
            "vpn" => vpn.take(),
            _ => None,
        })
        .collect();
    // Two to a row, the last one alone beside an empty half.
    let mut tiles = column![].spacing(10);
    let mut shown = shown.into_iter();
    while let Some(first) = shown.next() {
        let second: Element<'_, Message> =
            shown.next().unwrap_or_else(|| space::horizontal().into());
        tiles = tiles.push(row![first, second].spacing(10));
    }

    let mut sliders = column![].spacing(6);
    for name in &panel.layout.sliders {
        match name.as_str() {
            "volume" => {
                if let Some(volume) = panel.controls.volume {
                    let level = if volume.muted { 0.0 } else { volume.level };
                    sliders = sliders.push(slider_row(
                        icon(
                            Icon::volume(volume.level, volume.muted),
                            18.0,
                            neutral::text(),
                        ),
                        Some(Message::Change(Change::Mute(!volume.muted))),
                        level,
                        palette,
                        |v| Message::Change(Change::Volume(v)),
                    ));
                }
            }
            "apps" => {
                // Each app playing, with its icon: a click on it mutes it.
                for stream in &panel.controls.streams {
                    let level = if stream.volume.muted {
                        0.0
                    } else {
                        stream.volume.level
                    };
                    let id = stream.id;
                    sliders = sliders.push(slider_row(
                        crate::windows::picture(panel.apps, &stream.name, 18.0, palette),
                        Some(Message::Change(Change::AppMute(id, !stream.volume.muted))),
                        level,
                        palette,
                        move |v| Message::Change(Change::AppVolume(id, v)),
                    ));
                }
            }
            "mic" => {
                if let Some(mic) = panel.controls.mic {
                    let glyph = if mic.muted { Icon::MicOff } else { Icon::Mic };
                    sliders = sliders.push(slider_row(
                        icon(glyph, 18.0, neutral::text()),
                        Some(Message::Change(Change::MicMute(!mic.muted))),
                        if mic.muted { 0.0 } else { mic.level },
                        palette,
                        |v| Message::Change(Change::MicVolume(v)),
                    ));
                }
            }
            "devices" => {
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
            }
            "brightness" => {
                if let Some(brightness) = panel.controls.brightness {
                    sliders = sliders.push(slider_row(
                        icon(Icon::Sun, 18.0, neutral::text()),
                        None,
                        f32::from(brightness) / 100.0,
                        palette,
                        |v| Message::Change(Change::Brightness((v * 100.0).round().max(1.0) as u8)),
                    ));
                }
            }
            _ => {}
        }
    }
    let mut tiles = Some(tiles);
    let mut sliders = Some(sliders);
    for part in &panel.layout.parts {
        match part.as_str() {
            "tiles" => body = body.push(tiles.take()),
            "sliders" => body = body.push(sliders.take()),
            "modes" if !panel.modes.is_empty() => {
                body = body.push(modes(panel.modes, panel.mode, palette));
            }
            "player" => {
                if let Some(track) = panel.track {
                    body = body.push(player(track, palette));
                }
            }
            "notifications" => {
                body = body.push(history(panel.history, panel.opened, panel.apps, palette));
            }
            _ => {}
        }
    }

    frame(body, panel.edge, panel.gap, panel.look)
}

fn header<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let battery: Element<'a, Message> = match panel.battery {
        Some(battery) => {
            let tint = if battery.charging {
                panel.palette.accent
            } else {
                neutral::text()
            };
            let mut line = row![
                icon(Icon::battery(battery.percent, battery.charging), 18.0, tint),
                text(format!("{}%", battery.percent))
                    .size(14)
                    .font(style::medium()),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            let said = match (battery.charging, battery.time_left()) {
                (true, Some(left)) => Some(crate::i18n::fill(
                    "charging, full in {left}",
                    &[("left", &left)],
                )),
                (true, None) => Some(crate::i18n::t("charging").to_owned()),
                (false, Some(left)) => Some(crate::i18n::fill("{left} left", &[("left", &left)])),
                (false, None) => None,
            };
            if let Some(said) = said {
                line = line.push(text(said).size(12).color(style::color(style::faint())));
            }
            line.into()
        }
        None => text("Apolline").size(14).font(style::medium()).into(),
    };
    let round = |glyph: Icon, message: Message, on: bool| {
        button(container(icon(glyph, 16.0, neutral::text())).center(34))
            .padding(0)
            .style(move |theme, status| {
                let mut style = style::secondary(theme, status);
                style.border.radius = style::PILL.into();
                if on {
                    style.background =
                        Some(iced::Background::Color(style::color(style::hover_color())));
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
            neutral::ink()
        } else {
            neutral::text()
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

// The power profile, one click to the next.
fn power_tile<'a>(
    profiles: Option<&crate::profile::Profiles>,
    palette: Palette,
) -> Element<'a, Message> {
    let Some(profiles) = profiles else {
        return space::horizontal().into();
    };
    let (glyph, name) = match profiles.active.as_str() {
        "power-saver" => (Icon::Leaf, "Power saver"),
        "performance" => (Icon::Zap, "Performance"),
        _ => (Icon::Gauge, "Balanced"),
    };
    tile(
        glyph,
        "Power",
        name.to_owned(),
        profiles.active != "balanced",
        palette,
        profiles.next().map(Message::PickProfile),
    )
}

// A tile with a way into more of it on its right edge.
fn more<'a>(
    main: Element<'a, Message>,
    detail: Detail,
    on: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let ink = if on {
        palette.on_accent
    } else {
        neutral::text()
    };
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

// With one tunnel the tile switches it; with more it also lists them.
fn vpn_tile<'a>(panel: &Panel<'a>) -> Option<Element<'a, Message>> {
    let first = panel.tunnels.first()?;
    let on: Vec<&crate::vpn::Tunnel> = panel
        .tunnels
        .iter()
        .filter(|t| t.active.is_some())
        .collect();
    let subtitle = match on.as_slice() {
        [] => "Off".to_owned(),
        [one] if one.busy => format!("{}…", one.name),
        [one] => one.name.clone(),
        many => format!("{} on", many.len()),
    };
    let target = on.first().copied().unwrap_or(first);
    let tile = tile(
        Icon::Shield,
        "VPN",
        subtitle,
        !on.is_empty(),
        panel.palette,
        Some(Message::Vpn(target.clone(), target.active.is_none())),
    );
    Some(if panel.tunnels.len() > 1 {
        more(tile, Detail::Vpn, !on.is_empty(), panel.palette)
    } else {
        tile
    })
}

fn vpn_list<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let palette = panel.palette;
    let mut list = column![detail_header("VPN", false)].spacing(4);
    for tunnel in panel.tunnels {
        let on = tunnel.active.is_some();
        let state = match (on, tunnel.busy) {
            (true, true) => "Connecting…",
            (true, false) => "On",
            (false, _) => "",
        };
        let line = row![
            icon(
                Icon::Shield,
                15.0,
                if on { palette.accent } else { neutral::muted() }
            ),
            text(tunnel.name.clone()).size(13).width(Fill),
            text(state).size(12).color(style::color(style::faint())),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        list = list.push(
            button(line)
                .width(Fill)
                .padding([9, 10])
                .style(style::ghost(8.0))
                .on_press(Message::Vpn(tunnel.clone(), !on)),
        );
    }
    if let Some(note) = panel.note {
        list = list.push(
            container(
                text(note.to_owned())
                    .size(12)
                    .color(style::color(style::WARN)),
            )
            .padding([4, 10]),
        );
    }
    list.into()
}

fn quiet_list<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let palette = panel.palette;
    let mut list = column![detail_header("Do not disturb", false)].spacing(4);
    list = list.push(
        container(
            text("Only urgent notifications come through, like calls and alarms.")
                .size(12)
                .color(style::color(style::faint())),
        )
        .padding([0, 10]),
    );
    for (choice, label) in [
        (Quiet::For(60), "For an hour"),
        (Quiet::For(180), "For three hours"),
        (Quiet::Morning, "Until tomorrow morning"),
        (Quiet::Always, "Until I turn it off"),
        (Quiet::Off, "Off"),
    ] {
        let picked = match choice {
            Quiet::Always => panel.do_not_disturb && panel.quiet_until.is_none(),
            Quiet::Off => !panel.do_not_disturb,
            _ => false,
        };
        let mut line = row![text(label).size(13).width(Fill)].align_y(Alignment::Center);
        if picked {
            line = line.push(icon(Icon::Check, 14.0, palette.accent));
        }
        list = list.push(
            button(line)
                .width(Fill)
                .padding([9, 10])
                .style(style::ghost(8.0))
                .on_press(Message::Quiet(choice)),
        );
    }
    if let (true, Some(until)) = (panel.do_not_disturb, panel.quiet_until) {
        list = list.push(
            container(
                text(crate::i18n::fill(
                    "On until {time}",
                    &[("time", &until.format("%H:%M"))],
                ))
                .size(12)
                .color(style::color(palette.accent)),
            )
            .padding([4, 10]),
        );
    }
    list.into()
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
    let ink = if on {
        palette.on_accent
    } else {
        neutral::text()
    };
    let faint = if on {
        palette.on_accent.alpha(0.72)
    } else {
        style::faint()
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
    glyph: impl Into<Element<'a, Message>>,
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
            .color(style::color(style::faint()))
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
        let ink = if on {
            palette.on_accent
        } else {
            neutral::text()
        };
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
        text(crate::i18n::t("Mode").to_uppercase())
            .size(10)
            .font(style::semibold())
            .color(style::color(style::faint())),
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
        button(container(icon(glyph, 16.0, neutral::text())).center(32))
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
                .color(style::color(style::faint())),
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

fn history<'a>(
    items: &'a [Notification],
    opened: &'a [String],
    apps: &'a [crate::apps::App],
    palette: Palette,
) -> Element<'a, Message> {
    let mut header = row![
        text(crate::i18n::t("Notifications").to_uppercase())
            .size(10)
            .font(style::semibold())
            .color(style::color(style::faint())),
        space::horizontal(),
    ]
    .align_y(Alignment::Center);
    if !items.is_empty() {
        header = header.push(
            button(text("Clear").size(11).color(style::color(style::faint())))
                .padding([2, 8])
                .style(style::ghost(6.0))
                .on_press(Message::ClearAll),
        );
    }
    let list: Element<'a, Message> = if items.is_empty() {
        text("Nothing new")
            .size(12)
            .color(style::color(style::faint()))
            .into()
    } else {
        // One app's newest on top of the rest of its own, which open with a click.
        let mut groups: Vec<(&str, Vec<&Notification>)> = Vec::new();
        for item in items {
            match groups.iter_mut().find(|(app, _)| *app == item.app) {
                Some((_, list)) => list.push(item),
                None => groups.push((&item.app, vec![item])),
            }
        }
        let mut entries = Column::new().spacing(6);
        for (app, list) in groups {
            let open = opened.iter().any(|o| o == app);
            let shown = if open { list.len() } else { 1 };
            for item in &list[..shown] {
                entries = entries.push(entry(item, apps, palette));
            }
            if list.len() > 1 {
                let said = if open {
                    "Show less".to_owned()
                } else {
                    format!("{} more from {}", list.len() - 1, shorten(app, 24))
                };
                entries = entries.push(
                    button(text(said).size(11).color(style::color(palette.accent)))
                        .padding([2, 10])
                        .style(style::ghost(6.0))
                        .on_press(Message::HistoryGroup(app.to_owned())),
                );
            }
        }
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

fn entry<'a>(
    notification: &'a Notification,
    apps: &'a [crate::apps::App],
    palette: Palette,
) -> Element<'a, Message> {
    let mut words = column![
        row![
            text(shorten(&notification.app, 28))
                .size(11)
                .color(style::color(style::faint())),
            space::horizontal(),
            text(popup::ago(notification.received.elapsed()))
                .size(11)
                .color(style::color(style::faint())),
        ],
        style::verbatim(shorten(&notification.summary, 40))
            .size(13)
            .font(style::semibold()),
    ]
    .spacing(2)
    .width(Fill);
    if !notification.body.is_empty() {
        words = words.push(
            style::verbatim(shorten(&notification.body, 48))
                .size(12)
                .color(style::color(style::faint())),
        );
    }
    let words = button(
        row![popup::face(notification, apps, palette, 28.0), words]
            .spacing(10)
            .align_y(Alignment::Start),
    )
    .padding(0)
    .style(style::ghost(6.0))
    .on_press(Message::Activate(notification.id));
    container(
        row![
            words,
            button(icon(Icon::X, 12.0, style::faint()))
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
            .wrapping(iced::widget::text::Wrapping::None);
        button(label)
            .padding([5, 10])
            .style(style::choice(device.default))
            .on_press_maybe((!device.default).then(|| Message::Change(pick(device.id))))
            .into()
    }))
    .spacing(4)
    .wrap();
    row![
        container(icon(glyph, 16.0, style::faint())).width(26),
        chips,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([2, 0])
    .into()
}

fn frame<'a>(
    body: iced::widget::Column<'a, Message>,
    edge: apolline_config::Edge,
    gap: f32,
    look: apolline_config::Surface,
) -> Element<'a, Message> {
    let card = container(body.width(WIDTH))
        .padding(16)
        .style(style::surface_as(look));
    mouse_area(style::beside_bar(
        opaque(card),
        edge,
        Alignment::End,
        gap,
        metrics::BAR_INSET,
    ))
    .on_press(Message::Overlay(None))
    .into()
}

fn detail_header<'a>(title: &'static str, busy: bool) -> Element<'a, Message> {
    row![
        button(icon(Icon::ChevronLeft, 16.0, neutral::text()))
            .padding(6)
            .style(style::ghost(8.0))
            .on_press(Message::Detail(None)),
        text(title).size(15).font(style::semibold()).width(Fill),
        text(if busy { "Working…" } else { "" })
            .size(12)
            .color(style::color(style::faint())),
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
                .color(style::color(style::faint())),
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
            neutral::text()
        };
        let bars = row((1..=4).map(|bar| {
            let tint = if bar <= level {
                lit
            } else {
                neutral::text().alpha(0.18)
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
        if network.stranger {
            line = line.push(icon(Icon::Warning, 13.0, style::WARN));
        } else if network.secure() {
            line = line.push(icon(Icon::Lock, 13.0, style::faint()));
        }
        if network.active {
            line = line.push(icon(Icon::Check, 14.0, palette.accent));
        }
        let pick = button(line)
            .width(Fill)
            .padding([8, 10])
            .style(style::ghost(8.0))
            .on_press_maybe(
                (!network.active && !panel.busy).then(|| Message::PickNetwork(network.clone())),
            );
        list = list.push(forgettable(
            pick,
            (network.known || network.stranger).then(|| Message::ForgetNetwork(network.clone())),
            panel.busy,
        ));
        if let Some((joining, password)) = panel.joining
            && joining.ssid == network.ssid
        {
            list = list.push(
                row![
                    iced::widget::text_input(crate::i18n::t("Password"), password)
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

// A row with a small "forget" button after it, for what's saved.
fn forgettable<'a>(
    row_button: iced::widget::Button<'a, Message>,
    forget: Option<Message>,
    busy: bool,
) -> Element<'a, Message> {
    let Some(forget) = forget else {
        return row_button.into();
    };
    row![
        row_button,
        button(icon(Icon::Trash, 14.0, style::faint()))
            .padding(8)
            .style(style::ghost(8.0))
            .on_press_maybe((!busy).then_some(forget)),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .into()
}

fn gadget_icon(kind: &str) -> Icon {
    match kind {
        "audio-headset" | "audio-headphones" => Icon::Headphones,
        "audio-card" => Icon::Speaker,
        "input-keyboard" => Icon::Keyboard,
        "input-mouse" | "input-tablet" => Icon::Mouse,
        "input-gaming" => Icon::Gamepad,
        "phone" => Icon::Smartphone,
        "computer" => Icon::Laptop,
        _ if kind.starts_with("watch") => Icon::Watch,
        _ => Icon::Bluetooth,
    }
}

fn gadget_list<'a>(panel: &Panel<'a>) -> Element<'a, Message> {
    let palette = panel.palette;
    let mut list = column![detail_header("Bluetooth", panel.busy)].spacing(4);
    if let Some(ask) = panel.asking {
        let device = panel
            .gadgets
            .iter()
            .find(|d| ask.about(d))
            .map_or("the device", |d| d.name.as_str());
        list = list.push(ask_card(ask, device, panel.pin, palette));
    }
    let label = |words: &'static str| {
        container(text(words).size(11).color(style::color(style::faint()))).padding([6, 10])
    };
    let (mine, nearby): (Vec<_>, Vec<_>) = panel.gadgets.iter().partition(|d| d.paired);
    if !mine.is_empty() {
        list = list.push(label("Your devices"));
    }
    for device in mine {
        let mut state = if device.connected {
            "Connected".to_owned()
        } else {
            "Not connected".to_owned()
        };
        if let Some(battery) = device.battery {
            state = format!("{state} · {battery}%");
        }
        let line = row![
            icon(
                gadget_icon(&device.kind),
                15.0,
                if device.connected {
                    palette.accent
                } else {
                    neutral::muted()
                }
            ),
            column![
                text(shorten(&device.name, 30)).size(13),
                text(state).size(11).color(style::color(style::faint())),
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
        let toggle = button(line)
            .width(Fill)
            .padding([8, 10])
            .style(style::ghost(8.0))
            .on_press_maybe((!panel.busy).then(|| Message::Gadget(device.clone())));
        list = list.push(forgettable(
            toggle,
            Some(Message::ForgetGadget(device.clone())),
            panel.busy,
        ));
    }
    list = list.push(label("Nearby"));
    if nearby.is_empty() {
        list = list.push(
            container(
                text("Looking… put the device in pairing mode")
                    .size(12)
                    .color(style::color(style::faint())),
            )
            .padding([4, 10]),
        );
    }
    for device in nearby.into_iter().take(8) {
        let line = row![
            icon(gadget_icon(&device.kind), 15.0, neutral::muted()),
            text(shorten(&device.name, 30)).size(13).width(Fill),
            text("Pair").size(12).color(style::color(palette.accent)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        list = list.push(
            button(line)
                .width(Fill)
                .padding([8, 10])
                .style(style::ghost(8.0))
                .on_press_maybe(
                    (!panel.busy && panel.asking.is_none())
                        .then(|| Message::Gadget(device.clone())),
                ),
        );
    }
    if let Some(note) = panel.note {
        list = list.push(text(note).size(12).color(style::color(style::WARN)));
    }
    list.into()
}

fn ask_card<'a>(
    ask: &'a Ask,
    device: &'a str,
    pin: &'a str,
    palette: Palette,
) -> Element<'a, Message> {
    let code = |code: &'a str| {
        text(code)
            .size(26)
            .font(style::mono())
            .color(style::color(neutral::text()))
    };
    let buttons = |yes: &'static str, ready: bool| {
        row![
            space::horizontal(),
            button(text("Cancel").size(13))
                .padding([7, 14])
                .style(style::secondary)
                .on_press(Message::PairReply(false)),
            button(text(yes).size(13))
                .padding([7, 16])
                .style(style::primary(palette))
                .on_press_maybe(ready.then_some(Message::PairReply(true))),
        ]
        .spacing(8)
    };
    let body: Column<'a, Message> = match &ask.what {
        Asking::Confirm(number) => column![
            text(crate::i18n::fill(
                "Pair with {device}?",
                &[("device", &device)]
            ))
            .size(13),
            text("It should show the same code")
                .size(12)
                .color(style::color(style::faint())),
            code(number),
            buttons("Pair", true),
        ],
        Asking::Type(number) => column![
            text(crate::i18n::fill(
                "Type this on {device}, then press Enter",
                &[("device", &device)],
            ))
            .size(13),
            code(number),
            row![
                space::horizontal(),
                button(text("Cancel").size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::PairReply(false)),
            ],
        ],
        Asking::Allow => column![
            text(crate::i18n::fill(
                "Pair with {device}?",
                &[("device", &device)]
            ))
            .size(13),
            buttons("Pair", true),
        ],
        Asking::Pin | Asking::Passkey => column![
            text(if ask.what == Asking::Pin {
                crate::i18n::fill("The PIN for {device}", &[("device", &device)])
            } else {
                crate::i18n::fill("The code {device} shows", &[("device", &device)])
            })
            .size(13),
            text(if ask.what == Asking::Pin {
                "Often 0000 or 1234; it's in the manual"
            } else {
                "Six digits on its screen"
            })
            .size(12)
            .color(style::color(style::faint())),
            iced::widget::text_input(crate::i18n::t("PIN"), pin)
                .id(PIN)
                .on_input(Message::PinTyped)
                .on_submit_maybe((!pin.is_empty()).then_some(Message::PairReply(true)))
                .padding([7, 10])
                .size(13)
                .style(style::field(palette)),
            buttons("Pair", !pin.is_empty()),
        ],
    };
    container(body.spacing(8))
        .width(Fill)
        .padding(12)
        .style(style::tinted(palette.accent_soft))
        .into()
}

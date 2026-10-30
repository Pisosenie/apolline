use apolline_config::{Accent, Config, ModeSettings, Notifications};
use apolline_theme::{Mode, font, neutral};
use iced::gradient::Linear;
use iced::widget::{
    Column, Row, button, column, container, image, mouse_area, row, space, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Font, Gradient, Radians, Theme};

use crate::shell::Message;
use crate::style;

const CARD: f32 = 380.0;
const PICTURE: f32 = 236.0;

pub fn summary(mode: Mode, settings: &ModeSettings) -> Vec<String> {
    let wallpaper = match settings.wallpaper.strip_suffix(".jpg") {
        Some(name @ ("fog" | "ember" | "night")) => {
            format!("{}{} wallpaper", name[..1].to_uppercase(), &name[1..])
        }
        _ => "Your own wallpaper".to_owned(),
    };
    let accent = match settings.accent {
        Accent::Auto => "accent from it",
        Accent::Fixed(_) => "your accent",
    };
    let notifications = match settings.notifications {
        Notifications::All => "Every notification shows",
        Notifications::Mentions => "Low-priority notifications wait",
        Notifications::Hold => "Only urgent notifications show",
    };
    let open = match settings.open.as_slice() {
        [] => "Opens nothing".to_owned(),
        apps => format!(
            "Opens {}",
            apps.iter()
                .map(|a| a.split_whitespace().next().unwrap_or(a))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let mut lines = vec![
        format!("{wallpaper}, {accent}"),
        open,
        notifications.to_owned(),
    ];
    if mode == Mode::Play {
        lines.push("Effects off, games drive refresh, power boost".to_owned());
    }
    lines
}

pub fn view<'a>(
    current: Mode,
    config: &'a Config,
    thumbs: &'a [Option<image::Handle>; 3],
) -> Element<'a, Message> {
    let cards = Row::with_children(Mode::ALL.map(|mode| {
        card(
            mode,
            mode == current,
            config,
            thumbs[mode as usize].as_ref(),
        )
    }))
    .spacing(24);

    let key = |label: &'static str| {
        container(text(label).font(Font::with_name(font::MONO)).size(12))
            .padding([2, 8])
            .style(|_| container::Style {
                border: Border {
                    color: style::color(neutral::RAISED),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            })
    };
    let muted = style::color(neutral::MUTED);
    let hints = row![
        key("1–3"),
        text("pick").size(12).color(muted),
        space().width(12),
        key("esc"),
        text("stay").size(12).color(muted)
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let panel = column![
        row![
            text("Switch mode").size(28).font(semibold()),
            space::horizontal(),
            hints
        ]
        .align_y(Alignment::Center),
        cards,
        text("A mode is just a list of switches. Change them with apolline settings.")
            .size(13)
            .color(muted),
    ]
    .spacing(28)
    .width(CARD * 3.0 + 48.0);

    mouse_area(
        container(panel)
            .center(Fill)
            .style(|_| container::background(Color::from_rgba8(6, 7, 9, 0.62))),
    )
    .on_press(Message::Overlay(None))
    .into()
}

fn card<'a>(
    mode: Mode,
    current: bool,
    config: &'a Config,
    thumb: Option<&'a image::Handle>,
) -> Element<'a, Message> {
    let palette = config.palette(mode);
    let picture: Element<'a, Message> = match thumb {
        Some(handle) => image(handle.clone())
            .width(Fill)
            .height(Fill)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(16.0)
            .into(),
        None => container(space())
            .width(Fill)
            .height(Fill)
            .style(|_| container::background(style::color(neutral::INK)))
            .into(),
    };
    let scrim = container(space())
        .width(Fill)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Background::Gradient(Gradient::Linear(
                Linear::new(Radians(std::f32::consts::PI))
                    .add_stop(0.45, Color::from_rgba8(0, 0, 0, 0.0))
                    .add_stop(1.0, Color::from_rgba8(0, 0, 0, 0.62)),
            ))),
            border: Border {
                radius: 16.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    let tag = current.then(|| {
        container(
            text("Current")
                .size(11)
                .font(semibold())
                .color(style::color(palette.on_accent)),
        )
        .padding([4, 10])
        .style(move |_| container::Style {
            background: Some(Background::Color(style::color(palette.accent))),
            border: Border {
                radius: 999.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
    });
    let number = container(
        text((mode as usize + 1).to_string())
            .font(Font::with_name(font::MONO))
            .size(12),
    )
    .padding([3, 9])
    .style(|_| container::Style {
        background: Some(Background::Color(style::color(neutral::GLASS))),
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let overlay = column![
        row![tag, space::horizontal(), number].padding(16),
        space::vertical(),
        container(text(mode.name()).size(30).font(semibold())).padding([16, 20]),
    ];
    let ring = if current {
        palette.accent
    } else {
        neutral::HAIRLINE
    };
    let framed = container(stack![picture, scrim, overlay].height(PICTURE))
        .padding(3)
        .style(move |_| container::Style {
            border: Border {
                color: style::color(ring),
                width: if current { 3.0 } else { 1.0 },
                radius: 19.0.into(),
            },
            ..container::Style::default()
        });

    let lines = Column::with_children(summary(mode, config.mode(mode)).into_iter().map(|line| {
        row![
            container(space())
                .width(6)
                .height(6)
                .style(style::dot(palette.accent)),
            text(line)
                .size(13)
                .color(style::color(neutral::TEXT.alpha(0.86))),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }))
    .spacing(9)
    .padding([0, 4]);

    button(column![framed, lines].spacing(18))
        .width(CARD)
        .padding(0)
        .style(|_: &Theme, _| button::Style {
            background: None,
            text_color: style::color(neutral::TEXT),
            ..button::Style::default()
        })
        .on_press(Message::Mode(mode))
        .into()
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::with_name(font::UI)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_describe_the_real_config() {
        let (config, _) = Config::parse(include_str!("../../../assets/config.toml"));
        assert_eq!(
            summary(Mode::Play, config.mode(Mode::Play)),
            [
                "Ember wallpaper, accent from it",
                "Opens steam, discord",
                "Only urgent notifications show",
                "Effects off, games drive refresh, power boost",
            ]
        );
        let (custom, _) = Config::parse(
            "[mode.work]\nwallpaper = \"~/me.png\"\naccent = \"#ffffff\"\nopen = [\"firefox --new-window\"]\n",
        );
        let work = summary(Mode::Work, custom.mode(Mode::Work));
        assert_eq!(work[0], "Your own wallpaper, your accent");
        assert_eq!(work[1], "Opens firefox");
    }
}

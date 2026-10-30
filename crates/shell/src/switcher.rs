use std::collections::HashMap;

use apolline_config::{Accent, Config, ModeSettings, Notifications};
use apolline_theme::{Palette, font, neutral};
use iced::gradient::Linear;
use iced::widget::{
    Column, Row, button, column, container, image, mouse_area, opaque, row, space, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Font, Gradient, Radians, Theme};

use crate::icon::{Icon, icon};
use crate::shell::Message;
use crate::style;

pub fn summary(mode: Option<&ModeSettings>) -> Vec<String> {
    let Some(mode) = mode else {
        return vec![
            "The desktop's own wallpaper and accent".to_owned(),
            "Every notification shows".to_owned(),
        ];
    };
    let wallpaper = match mode.wallpaper.as_deref() {
        None => "The desktop's wallpaper".to_owned(),
        Some(name @ ("fog.jpg" | "ember.jpg" | "night.jpg")) => {
            let name = name.trim_end_matches(".jpg");
            format!("{}{} wallpaper", name[..1].to_uppercase(), &name[1..])
        }
        Some(_) => "Your own wallpaper".to_owned(),
    };
    let accent = match mode.accent {
        None => "the desktop's accent",
        Some(Accent::Auto) => "accent from it",
        Some(Accent::Fixed(_)) => "its own accent",
    };
    let mut lines = vec![format!("{wallpaper}, {accent}")];
    if !mode.open.is_empty() {
        let apps: Vec<&str> = mode
            .open
            .iter()
            .map(|a| a.split_whitespace().next().unwrap_or(a))
            .collect();
        lines.push(format!("Opens {}", apps.join(", ")));
    }
    lines.push(
        match mode.notifications {
            Notifications::All => "Every notification shows",
            Notifications::Mentions => "Low-priority notifications wait",
            Notifications::Hold => "Only urgent notifications show",
        }
        .to_owned(),
    );
    if mode.performance {
        lines.push("Effects off, games drive refresh, power boost".to_owned());
    }
    if mode.keep_awake {
        lines.push("The screen stays awake".to_owned());
    }
    match mode.keys.len() {
        0 => {}
        1 => lines.push("1 shortcut of its own".to_owned()),
        n => lines.push(format!("{n} shortcuts of its own")),
    }
    lines
}

pub fn view<'a>(
    current: Option<usize>,
    config: &'a Config,
    thumbs: &'a HashMap<String, image::Handle>,
) -> Element<'a, Message> {
    let muted = style::color(neutral::MUTED);
    let count = config.modes.len() + 1;
    let width = match count {
        0..=3 => 380.0,
        4 => 300.0,
        _ => 250.0,
    };
    let mut cards: Vec<Element<'a, Message>> = vec![card(None, current, config, thumbs, width)];
    cards.extend((0..config.modes.len()).map(|i| card(Some(i), current, config, thumbs, width)));
    let cards = Row::with_children(cards)
        .spacing(24)
        .wrap()
        .vertical_spacing(24);

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
    let hints = row![
        key("1–9"),
        text("pick").size(12).color(muted),
        space().width(12),
        key("0"),
        text("no mode").size(12).color(muted),
        space().width(12),
        key("esc"),
        text("stay").size(12).color(muted)
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let footer: Element<'a, Message> =
        if config.modes.is_empty() {
            row![
            text("No modes yet. A mode changes wallpaper, notifications, apps and keys in one go.")
                .size(13)
                .color(muted),
            space::horizontal(),
            button(
                row![icon(Icon::Sparkles, 14.0, neutral::INK), text("Make a mode").size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center)
            )
            .padding([8, 14])
            .style(style::primary(Palette::default()))
            .on_press(Message::SettingsPage("modes")),
        ]
            .align_y(Alignment::Center)
            .into()
        } else {
            row![
                text("Modes are lists of switches. Change them in Settings → Modes.")
                    .size(13)
                    .color(muted),
                space::horizontal(),
                button(text("Edit modes").size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::SettingsPage("modes")),
            ]
            .align_y(Alignment::Center)
            .into()
        };

    let columns = count.min(4) as f32;
    let panel = column![
        row![
            text("Switch mode").size(28).font(semibold()),
            space::horizontal(),
            hints
        ]
        .align_y(Alignment::Center),
        cards,
        footer,
    ]
    .spacing(28)
    .width(width * columns + 24.0 * (columns - 1.0));

    mouse_area(
        container(opaque(panel))
            .center(Fill)
            .style(|_| container::background(Color::from_rgba8(6, 7, 9, 0.62))),
    )
    .on_press(Message::Overlay(None))
    .into()
}

fn card<'a>(
    mode: Option<usize>,
    current: Option<usize>,
    config: &'a Config,
    thumbs: &'a HashMap<String, image::Handle>,
    width: f32,
) -> Element<'a, Message> {
    let settings = mode.and_then(|i| config.modes.get(i));
    let palette = config.palette(mode);
    let on = mode == current;
    let height = (width * 0.62).round();
    let picture: Element<'a, Message> = match thumbs.get(config.wallpaper(mode)) {
        Some(handle) => container(
            image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(iced::ContentFit::Fill)
                .border_radius(16.0),
        )
        .clip(true)
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

    let tag = on.then(|| {
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
        text(mode.map_or(0, |i| i + 1).to_string())
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
    let (glyph, name) = match settings {
        Some(settings) => (Icon::named(&settings.icon), settings.name.as_str()),
        None => (Icon::Circle, "No mode"),
    };
    let overlay = column![
        row![tag, space::horizontal(), number].padding(16),
        space::vertical(),
        container(
            row![
                icon(glyph, 22.0, neutral::TEXT),
                text(name).size(26).font(semibold())
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        )
        .padding([16, 20]),
    ];
    let ring = if on {
        palette.accent
    } else {
        neutral::HAIRLINE
    };
    let framed = container(stack![picture, scrim, overlay].height(height))
        .padding(3)
        .style(move |_| container::Style {
            border: Border {
                color: style::color(ring),
                width: if on { 3.0 } else { 1.0 },
                radius: 19.0.into(),
            },
            ..container::Style::default()
        });

    let lines = Column::with_children(summary(settings).into_iter().map(|line| {
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
        .width(width)
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
        let (config, _) = Config::parse(
            "[[mode]]\nname = \"Play\"\nwallpaper = \"ember.jpg\"\naccent = \"auto\"\nnotifications = \"hold\"\nopen = [\"steam\", \"discord --start-minimized\"]\nperformance = true\nkeys = { \"Mod+G\" = \"steam\" }\n\n[[mode]]\nname = \"Quiet\"\n",
        );
        assert_eq!(
            summary(config.modes.first()),
            [
                "Ember wallpaper, accent from it",
                "Opens steam, discord",
                "Only urgent notifications show",
                "Effects off, games drive refresh, power boost",
                "1 shortcut of its own",
            ]
        );
        assert_eq!(
            summary(config.modes.get(1)),
            [
                "The desktop's wallpaper, the desktop's accent",
                "Every notification shows"
            ]
        );
        assert_eq!(summary(None)[0], "The desktop's own wallpaper and accent");
    }
}

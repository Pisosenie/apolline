use apolline_theme::{Rgba, metrics, neutral};
use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};

pub fn color(c: Rgba) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn rounded(radius: f32) -> Border {
    Border {
        radius: radius.into(),
        ..Border::default()
    }
}

pub fn bar(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(color(neutral::TEXT)),
        background: Some(Background::Color(color(neutral::GLASS))),
        border: Border {
            color: color(neutral::HAIRLINE),
            width: 1.0,
            radius: metrics::RADIUS_BAR.into(),
        },
        ..container::Style::default()
    }
}

pub fn dot(fill: Rgba) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color(fill))),
        border: rounded(3.0),
        ..container::Style::default()
    }
}

pub fn chip(fill: Rgba, text: Rgba) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: Some(Background::Color(color(fill))),
        text_color: color(text),
        border: rounded(metrics::RADIUS_CONTROL),
        ..button::Style::default()
    }
}

pub fn workspace(active: bool, busy: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let fill = match status {
            _ if active => Some(neutral::RAISED),
            button::Status::Hovered | button::Status::Pressed => Some(neutral::HAIRLINE),
            _ => None,
        };
        button::Style {
            background: fill.map(|c| Background::Color(color(c))),
            text_color: color(if active || busy {
                neutral::TEXT
            } else {
                neutral::MUTED
            }),
            border: rounded(metrics::RADIUS_CONTROL),
            ..button::Style::default()
        }
    }
}

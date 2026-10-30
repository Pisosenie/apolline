use std::time::Duration;

use apolline_theme::{Palette, font, metrics, neutral};
use iced::widget::{button, column, container, row, space, text};
use iced::{Background, Border, Element, Fill, Theme};

use crate::media::shorten;
use crate::notify::{Notification, Urgency};
use crate::shell::Message;
use crate::style;

pub const WIDTH: u32 = 360;
pub const MAX: usize = 3;
const HEIGHT: u32 = 84;
const GAP: u32 = 8;

#[cfg(any(target_os = "linux", test))]
pub fn height(count: usize) -> u32 {
    let count = count.min(MAX) as u32;
    count * HEIGHT + count.saturating_sub(1) * GAP
}

pub fn stack<'a>(
    notifications: impl Iterator<Item = &'a Notification>,
    palette: Palette,
) -> Element<'a, Message> {
    column(notifications.take(MAX).map(|n| card(n, palette)))
        .spacing(GAP as f32)
        .width(WIDTH as f32)
        .into()
}

fn card(notification: &Notification, palette: Palette) -> Element<'_, Message> {
    let muted = style::color(neutral::MUTED);
    let edge = if notification.urgency == Urgency::Critical {
        palette.accent
    } else {
        neutral::HAIRLINE
    };

    let content = column![
        row![
            text(shorten(&notification.app, 32)).size(11).color(muted),
            space::horizontal(),
            text(ago(notification.received.elapsed()))
                .size(11)
                .color(muted),
        ],
        text(shorten(&notification.summary, 40))
            .size(14)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::with_name(font::UI)
            }),
        text(shorten(&notification.body, 52))
            .size(font::SMALL)
            .color(muted),
    ]
    .spacing(3);

    button(
        container(content)
            .padding([12, 14])
            .width(Fill)
            .height(HEIGHT as f32),
    )
    .padding(0)
    .width(Fill)
    .style(move |_: &Theme, _| button::Style {
        background: Some(Background::Color(style::color(neutral::PANEL))),
        text_color: style::color(neutral::TEXT),
        border: Border {
            color: style::color(edge),
            width: 1.0,
            radius: metrics::RADIUS_PANEL.into(),
        },
        ..button::Style::default()
    })
    .on_press(Message::Dismiss(notification.id))
    .into()
}

pub fn ago(elapsed: Duration) -> String {
    match elapsed.as_secs() / 60 {
        0 => "now".into(),
        minutes @ 1..60 => format!("{minutes} min"),
        minutes => format!("{} h", minutes / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_reads_like_a_person_would_say_it() {
        assert_eq!(ago(Duration::from_secs(20)), "now");
        assert_eq!(ago(Duration::from_secs(150)), "2 min");
        assert_eq!(ago(Duration::from_secs(3 * 3600 + 120)), "3 h");
    }

    #[test]
    fn surface_fits_the_visible_cards_exactly() {
        assert_eq!(height(0), 0);
        assert_eq!(height(1), 84);
        assert_eq!(height(2), 176);
        assert_eq!(height(7), height(MAX));
    }
}

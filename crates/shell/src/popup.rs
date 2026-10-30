use std::time::Duration;

use apolline_theme::{Palette, font, metrics, neutral};
use iced::widget::{button, column, container, image, row, space, text};
use iced::{Background, Border, ContentFit, Element, Fill, Theme};

use crate::apps::App;

use crate::media::shorten;
use crate::notify::{Notification, Urgency};
use crate::shell::Message;
use crate::style;

pub const WIDTH: u32 = 360;
pub const MAX: usize = 3;
const HEIGHT: u32 = 84;
const BUTTONS: u32 = 38;
const GAP: u32 = 8;
const FACE: f32 = 36.0;

fn card_height(notification: &Notification) -> u32 {
    if notification.buttons().next().is_some() {
        HEIGHT + BUTTONS
    } else {
        HEIGHT
    }
}

#[cfg(any(target_os = "linux", test))]
pub fn height<'a>(cards: impl Iterator<Item = &'a Notification>) -> u32 {
    let heights: Vec<u32> = cards.take(MAX).map(card_height).collect();
    heights.iter().sum::<u32>() + (heights.len() as u32).saturating_sub(1) * GAP
}

pub fn stack<'a>(
    notifications: impl Iterator<Item = &'a Notification>,
    apps: &'a [App],
    palette: Palette,
) -> Element<'a, Message> {
    column(notifications.take(MAX).map(|n| card(n, apps, palette)))
        .spacing(GAP as f32)
        .width(WIDTH as f32)
        .into()
}

// What the app sent along, else the app's own icon.
fn face<'a>(
    notification: &'a Notification,
    apps: &'a [App],
    palette: Palette,
) -> Element<'a, Message> {
    match &notification.image {
        Some(picture) => image(picture.clone())
            .width(FACE)
            .height(FACE)
            .content_fit(ContentFit::Cover)
            .border_radius(FACE / 2.0)
            .into(),
        None => {
            let app = notification.desktop.as_deref().unwrap_or(&notification.app);
            crate::windows::picture(apps, app, FACE, palette)
        }
    }
}

fn card<'a>(
    notification: &'a Notification,
    apps: &'a [App],
    palette: Palette,
) -> Element<'a, Message> {
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
        text(shorten(&notification.summary, 34))
            .size(14)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::with_name(crate::style::family())
            }),
        text(shorten(&notification.body, 44))
            .size(font::SMALL)
            .color(muted),
    ]
    .spacing(3);

    let buttons: Vec<Element<'_, Message>> = notification
        .buttons()
        .map(|(key, label)| {
            button(text(shorten(label, 18)).size(12).font(style::medium()))
                .padding([5, 12])
                .style(style::secondary)
                .on_press(Message::Invoke(notification.id, key.clone()))
                .into()
        })
        .collect();
    let content = if buttons.is_empty() {
        content
    } else {
        content
            .push(space().height(6))
            .push(row(buttons).spacing(6))
    };
    let content = row![face(notification, apps, palette), content.width(Fill)].spacing(12);

    button(
        container(content)
            .padding([12, 14])
            .width(Fill)
            .height(card_height(notification) as f32),
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
    .on_press(Message::Activate(notification.id))
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
        let plain = crate::notify::shot_card("/tmp/a.png");
        let mut asking = plain.clone();
        asking.actions = vec![
            ("default".into(), "Open".into()),
            ("reply".into(), "Reply".into()),
        ];
        assert_eq!(height([].iter()), 0);
        assert_eq!(height([&plain].into_iter()), 84);
        assert_eq!(height([&plain, &asking].into_iter()), 84 + 8 + 122);
        assert_eq!(height(std::iter::repeat_n(&plain, 7)), 3 * 84 + 2 * 8);
    }
}

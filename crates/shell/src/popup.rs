use std::time::Duration;

use crate::style::text;
use apolline_theme::{Palette, font, metrics, neutral};
use iced::widget::{
    button, column, container, image, keyed_column, progress_bar, row, space, text_input,
};
use iced::{Background, Border, ContentFit, Element, Fill, Theme};

use crate::apps::App;
use crate::icon::{Icon, icon};

use crate::media::shorten;
use crate::notify::{Notification, Urgency};
use crate::shell::Message;
use crate::style;

pub const WIDTH: u32 = 360;
pub const REPLY: &str = "notification-reply";
pub const MAX: usize = 3;
const HEIGHT: u32 = 84;
const BUTTONS: u32 = 38;
const GAP: u32 = 8;
const FACE: f32 = 36.0;

const PROGRESS: u32 = 12;

fn card_height(notification: &Notification) -> u32 {
    let buttons = if notification.buttons().next().is_some() || notification.reply.is_some() {
        BUTTONS
    } else {
        0
    };
    let progress = if notification.progress.is_some() {
        PROGRESS
    } else {
        0
    };
    HEIGHT + buttons + progress
}

// The cards that show: the newest few, and always the one being answered.
pub fn shown<'a>(
    cards: impl Iterator<Item = &'a Notification>,
    answering: Option<u32>,
) -> Vec<&'a Notification> {
    let all: Vec<&Notification> = cards.collect();
    let mut shown: Vec<&Notification> = all.iter().copied().take(MAX).collect();
    if let Some(id) = answering
        && !shown.iter().any(|n| n.id == id)
        && let Some(card) = all.iter().find(|n| n.id == id)
    {
        shown.pop();
        shown.push(card);
    }
    shown
}

#[cfg(any(target_os = "linux", test))]
pub fn height(cards: &[&Notification]) -> u32 {
    let heights: Vec<u32> = cards.iter().map(|n| card_height(n)).collect();
    heights.iter().sum::<u32>() + (heights.len() as u32).saturating_sub(1) * GAP
}

// Keyed by id, so a card keeps its field's focus when others come and go around it.
pub fn stack<'a>(
    cards: Vec<&'a Notification>,
    apps: &'a [App],
    palette: Palette,
    replying: Option<&'a (u32, String)>,
    look: apolline_config::Surface,
) -> Element<'a, Message> {
    keyed_column(cards.into_iter().map(|n| {
        let typed = replying
            .filter(|(id, _)| *id == n.id)
            .map(|(_, typed)| typed.as_str());
        (n.id, card(n, apps, palette, typed, look))
    }))
    .spacing(GAP as f32)
    .width(WIDTH as f32)
    .into()
}

// What the app sent along, else the app's own icon.
pub fn face<'a>(
    notification: &'a Notification,
    apps: &'a [App],
    palette: Palette,
    size: f32,
) -> Element<'a, Message> {
    let glyph = match notification.id {
        crate::notify::BATTERY_CARD => Some(Icon::BatteryWarning),
        id if crate::notify::is_reminder(id) && notification.actions.is_empty() => Some(Icon::Bell),
        id if crate::notify::is_reminder(id) => Some(Icon::Video),
        _ => None,
    };
    if let Some(glyph) = glyph {
        return container(icon(glyph, size * 0.5, palette.accent))
            .center(size)
            .style(style::pill(palette.accent_soft))
            .into();
    }
    match &notification.image {
        Some(picture) => image(picture.clone())
            .width(size)
            .height(size)
            .content_fit(ContentFit::Cover)
            .border_radius(size / 2.0)
            .into(),
        None => {
            let app = notification.desktop.as_deref().unwrap_or(&notification.app);
            crate::windows::picture(apps, app, size, palette)
        }
    }
}

fn card<'a>(
    notification: &'a Notification,
    apps: &'a [App],
    palette: Palette,
    typed: Option<&'a str>,
    look: apolline_config::Surface,
) -> Element<'a, Message> {
    let muted = style::color(neutral::muted());
    let edge = if notification.urgency == Urgency::Critical {
        palette.accent
    } else {
        neutral::hairline()
    };

    let content = column![
        row![
            text(shorten(&notification.app, 32)).size(11).color(muted),
            space::horizontal(),
            text(ago(notification.received.elapsed()))
                .size(11)
                .color(muted),
            button(icon(Icon::X, 11.0, neutral::muted()))
                .padding(2)
                .style(style::ghost(6.0))
                .on_press(Message::Dismiss(notification.id)),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
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

    let reply = notification.reply.as_ref().map(|_| {
        button(text("Reply").size(12).font(style::medium()))
            .padding([5, 12])
            .style(style::secondary)
            .on_press(Message::ReplyOpen(notification.id))
            .into()
    });
    let buttons: Vec<Element<'_, Message>> = reply
        .into_iter()
        .chain(notification.buttons().map(|(key, label)| {
            button(text(shorten(label, 18)).size(12).font(style::medium()))
                .padding([5, 12])
                .style(style::secondary)
                .on_press(Message::Invoke(notification.id, key.clone()))
                .into()
        }))
        .take(3)
        .collect();
    let content = match notification.progress {
        Some(value) => content.push(space().height(4)).push(
            progress_bar(0.0..=100.0, f32::from(value))
                .girth(4)
                .style(move |_| progress_bar::Style {
                    background: Background::Color(style::color(neutral::hairline())),
                    bar: Background::Color(style::color(palette.accent)),
                    border: Border::default().rounded(2),
                }),
        ),
        None => content,
    };
    let content = match (typed, &notification.reply) {
        (Some(typed), Some(hint)) => content.push(space().height(6)).push(
            row![
                text_input(hint, typed)
                    .id(REPLY)
                    .on_input(Message::ReplyTyped)
                    .on_submit(Message::ReplySend)
                    .padding([5, 10])
                    .size(12)
                    .style(style::field(palette)),
                button(icon(Icon::Send, 13.0, palette.on_accent))
                    .padding([6, 10])
                    .style(style::primary(palette))
                    .on_press_maybe((!typed.trim().is_empty()).then_some(Message::ReplySend)),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        ),
        _ if buttons.is_empty() => content,
        _ => content
            .push(space().height(6))
            .push(row(buttons).spacing(6)),
    };
    let content = row![face(notification, apps, palette, FACE), content.width(Fill)].spacing(12);

    button(
        container(content)
            .padding([12, 14])
            .width(Fill)
            .height(card_height(notification) as f32),
    )
    .padding(0)
    .width(Fill)
    .style(move |_: &Theme, _| button::Style {
        background: Some(Background::Color(style::color(
            neutral::panel().alpha(look.opacity.unwrap_or(1.0)),
        ))),
        text_color: style::color(neutral::text()),
        border: Border {
            color: style::color(edge),
            width: 1.0,
            radius: look.radius.map_or(metrics::RADIUS_PANEL, f32::from).into(),
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

// The cards' hold on the keyboard. niri hands an on-demand surface the keyboard for a click made
// while it already is one, and at once when one is mapped, so the cards turn on-demand only once
// the pointer is over one that can be answered, and give the keyboard back after any click that
// didn't start an answer.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    keys: Keys,
    pointer: bool,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Keys {
    #[default]
    Off,
    // A click on the cards will give it.
    Ready,
    // An answer is being typed.
    Held,
    // Given back; waiting for the compositor to take it.
    Returning,
}

// What reached the cards' window.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug)]
pub enum Felt {
    PointerIn,
    PointerOut,
    Click,
    KeysLost,
}

// What a change on the cards' window asks of the answer being typed.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    // The keyboard went elsewhere.
    Unfocused,
    // A click on the cards: the keys stay with the answer, not with what was clicked.
    Refocus,
}

#[cfg(any(target_os = "linux", test))]
impl Hold {
    pub fn on_demand(self) -> bool {
        matches!(self.keys, Keys::Ready | Keys::Held)
    }

    // Has the keyboard, or is giving it back.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn holds(self) -> bool {
        matches!(self.keys, Keys::Held | Keys::Returning)
    }

    pub fn settle(self, typing: bool, answerable: bool) -> Hold {
        let keys = match self.keys {
            _ if typing => Keys::Held,
            Keys::Off if self.pointer && answerable => Keys::Ready,
            Keys::Ready if !answerable => Keys::Off,
            Keys::Held if !typing => Keys::Returning,
            keys => keys,
        };
        Hold { keys, ..self }
    }

    pub fn felt(self, felt: Felt) -> (Hold, Option<Nudge>) {
        let keys = match (felt, self.keys) {
            // Back after giving the keyboard back: long enough for it to have gone.
            (Felt::PointerIn, Keys::Returning) => Keys::Off,
            // A click that started no answer still handed over the keyboard.
            (Felt::Click, Keys::Ready) => Keys::Returning,
            (Felt::KeysLost, Keys::Held) => Keys::Ready,
            (Felt::KeysLost, Keys::Returning) => Keys::Off,
            (_, keys) => keys,
        };
        let pointer = match felt {
            Felt::PointerIn => true,
            Felt::PointerOut => false,
            _ => self.pointer,
        };
        let nudge = match (felt, self.keys) {
            (Felt::KeysLost, Keys::Held) => Some(Nudge::Unfocused),
            (Felt::Click, Keys::Held) => Some(Nudge::Refocus),
            _ => None,
        };
        (Hold { keys, pointer }, nudge)
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
        assert_eq!(height(&[]), 0);
        assert_eq!(height(&[&plain]), 84);
        assert_eq!(height(&[&plain, &asking]), 84 + 8 + 122);
        let many = shown(std::iter::repeat_n(&plain, 7), None);
        assert_eq!(height(&many), 3 * 84 + 2 * 8);
    }

    #[test]
    fn the_cards_take_the_keyboard_only_to_answer() {
        let hold = Hold::default().settle(false, true);
        assert!(!hold.on_demand(), "not before the pointer gets there");
        let (hold, _) = hold.felt(Felt::PointerIn);
        assert!(!hold.settle(false, false).on_demand(), "nothing to answer");
        let ready = hold.settle(false, true);
        assert!(ready.on_demand());

        let typing = ready.settle(true, true);
        assert_eq!(
            typing.felt(Felt::Click),
            (typing, Some(Nudge::Refocus)),
            "a click on the cards keeps typing in the answer"
        );
        let (lost, nudge) = typing.felt(Felt::KeysLost);
        assert_eq!(
            nudge,
            Some(Nudge::Unfocused),
            "a click elsewhere takes the keyboard"
        );
        assert!(
            lost.settle(true, true).on_demand(),
            "a click on the cards brings it back"
        );

        let sent = typing.settle(false, true);
        assert!(!sent.on_demand(), "sent: the keyboard goes back");
        let (gone, _) = sent.felt(Felt::KeysLost);
        assert_eq!(
            gone.settle(false, true),
            ready,
            "and the cards are ready again"
        );
    }

    #[test]
    fn a_click_that_answers_nothing_gives_the_keyboard_back() {
        let (hold, _) = Hold::default().felt(Felt::PointerIn);
        let (clicked, nudge) = hold.settle(false, true).felt(Felt::Click);
        assert_eq!(nudge, None);
        assert!(!clicked.settle(false, true).on_demand());
        assert!(
            clicked.settle(true, true).on_demand(),
            "Reply right after still gets a field to click into"
        );
        let (back, _) = clicked.felt(Felt::PointerOut).0.felt(Felt::PointerIn);
        assert!(
            back.settle(false, true).on_demand(),
            "rearmed when the pointer is back"
        );
    }

    #[test]
    fn the_card_being_answered_stays_in_view() {
        let cards: Vec<Notification> = (1..=5)
            .map(|id| Notification {
                id,
                ..crate::notify::shot_card("/tmp/a.png")
            })
            .collect();
        let ids = |answering| -> Vec<u32> {
            shown(cards.iter(), answering)
                .iter()
                .map(|n| n.id)
                .collect()
        };
        assert_eq!(ids(None), [1, 2, 3]);
        assert_eq!(ids(Some(2)), [1, 2, 3]);
        assert_eq!(ids(Some(5)), [1, 2, 5]);
        assert_eq!(ids(Some(9)), [1, 2, 3], "gone already");
    }
}

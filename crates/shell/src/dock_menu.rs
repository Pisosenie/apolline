use crate::lit::button;
use apolline_config::Edge;
use apolline_theme::{Palette, neutral};
use iced::widget::{Column, container, mouse_area, opaque, row, space};
use iced::{Alignment, Element, Fill, Padding};

use crate::dock::{Event, Item};
use crate::shell::Message;
use crate::style;
use crate::style::text;

const WIDTH: f32 = 260.0;

pub struct Menu {
    pub item: Item,
    pub name: String,
    pub palette: Palette,
    pub edge: Edge,
    // How far from the dock's middle the app's icon is.
    pub offset: f32,
    // How far from the screen edge the menu keeps.
    pub gap: f32,
}

// An app's windows to pick from, then what can be done with the app, by its icon on the dock.
pub fn view<'a>(menu: Menu) -> Element<'a, Message> {
    let Menu {
        item,
        name,
        palette,
        edge,
        offset,
        gap,
    } = menu;
    let mut list = Column::new().spacing(2);
    list = list.push(
        container(
            style::verbatim(name)
                .size(12)
                .color(style::color(style::faint())),
        )
        .padding([4, 10]),
    );
    for window in &item.windows {
        let mark = if window.focused {
            container(space())
                .width(6)
                .height(6)
                .style(style::dot(palette.accent))
        } else {
            container(space()).width(6).height(6)
        };
        let title = if window.title.is_empty() {
            &item.key
        } else {
            &window.title
        };
        let line = row![mark, style::verbatim(short(title)).size(13)]
            .spacing(8)
            .align_y(Alignment::Center);
        list = list.push(entry(line, Event::Pick(window.id)));
    }
    if !item.windows.is_empty() {
        list = list.push(rule());
    }
    list = list.push(entry(
        text("New window").size(13),
        Event::Launch(item.key.clone()),
    ));
    let pin = if item.pinned { "Unpin" } else { "Pin" };
    list = list.push(entry(text(pin).size(13), Event::Pin(item.key.clone())));
    if !item.windows.is_empty() {
        let close = text("Close all")
            .size(13)
            .color(style::color(style::DANGER));
        list = list.push(entry(close, Event::CloseAll(item.key.clone())));
    }
    let card = container(list.width(WIDTH))
        .padding(6)
        .style(style::surface)
        .id(style::CARD);
    // Moved off the middle of the edge to sit by its icon.
    let along = 2.0 * offset;
    let nudge = match (edge.vertical(), along >= 0.0) {
        (false, true) => Padding::ZERO.left(along),
        (false, false) => Padding::ZERO.right(-along),
        (true, true) => Padding::ZERO.top(along),
        (true, false) => Padding::ZERO.bottom(-along),
    };
    let placed = container(opaque(card)).padding(nudge);
    mouse_area(style::beside_bar(placed, edge, Alignment::Center, gap, 0.0))
        .on_press(Message::Overlay(None))
        .into()
}

// A window's title on one line of the menu.
fn short(title: &str) -> String {
    const LONGEST: usize = 34;
    if title.chars().count() <= LONGEST {
        return title.to_owned();
    }
    let kept: String = title.chars().take(LONGEST - 1).collect();
    format!("{}…", kept.trim_end())
}

fn entry<'a>(label: impl Into<Element<'a, Message>>, event: Event) -> Element<'a, Message> {
    button(label)
        .width(Fill)
        .padding([7, 10])
        .style(style::ghost(8.0))
        .on_press(Message::Dock(event))
        .into()
}

fn rule<'a>() -> Element<'a, Message> {
    container(
        container(space().width(Fill).height(1))
            .style(|_| container::background(style::color(neutral::hairline()))),
    )
    .padding([4, 10])
    .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_title_is_cut_on_one_line() {
        assert_eq!(super::short("Inbox"), "Inbox");
        let long = "a".repeat(80);
        let cut = super::short(&long);
        assert_eq!(cut.chars().count(), 34);
        assert!(cut.ends_with('…'));
    }
}

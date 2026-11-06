use crate::lit::button;
use crate::style::text;
use apolline_theme::{Palette, metrics, neutral};
use iced::widget::{Column, column, container, mouse_area, opaque, row, space};
use iced::{Alignment, Element, Fill};

use crate::icon::{Icon, icon};
use crate::shell::Message;
use crate::style;
use crate::tray::{Item, MenuEntry};

const WIDTH: f32 = 260.0;

// An app's tray menu, under the bar on the right where its icon is.
pub fn view<'a>(
    item: &'a Item,
    entries: &'a [MenuEntry],
    palette: Palette,
    edge: apolline_config::Edge,
    gap: f32,
) -> Element<'a, Message> {
    let mut list = Column::new().spacing(2);
    list = list.push(
        container(
            text(item.title.clone())
                .size(12)
                .color(style::color(style::faint())),
        )
        .padding([4, 10]),
    );
    for entry in entries {
        if entry.separator {
            list = list.push(
                container(
                    container(space().width(Fill).height(1))
                        .style(|_| container::background(style::color(neutral::hairline()))),
                )
                .padding([4, 10]),
            );
            continue;
        }
        let mark: Element<'a, Message> = match entry.toggled {
            Some(true) => icon(Icon::Done, 13.0, palette.accent).into(),
            Some(false) => space().width(13).into(),
            None => space().width(0).into(),
        };
        let ink = if entry.enabled {
            neutral::text()
        } else {
            style::faint()
        };
        let label = row![
            mark,
            text(entry.label.clone()).size(13).color(style::color(ink))
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        list = list.push(
            button(label)
                .width(Fill)
                .padding([7, 10])
                .style(style::ghost(8.0))
                .on_press_maybe(entry.enabled.then_some(Message::TrayChoose(entry.id))),
        );
    }
    let card = container(column![list].width(WIDTH))
        .padding(6)
        .style(style::surface);
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

use apolline_theme::{Mode, Palette, font, metrics, neutral};
use chrono::{DateTime, Local};
use iced::widget::{Row, button, container, row, space, text};
use iced::{Alignment, Element, Fill, Font};

use crate::shell::Message;
use crate::status::Status;
use crate::style;

pub struct Bar<'a> {
    pub mode: Mode,
    pub palette: Palette,
    pub active: usize,
    pub status: &'a Status,
    pub now: DateTime<Local>,
}

pub fn view(bar: Bar<'_>) -> Element<'_, Message> {
    let mono = Font::with_name(font::MONO);
    let muted = style::color(neutral::MUTED);

    let chip = button(
        row![
            container(space())
                .width(6)
                .height(6)
                .style(style::dot(bar.palette.accent)),
            text(bar.mode.name()).size(font::SMALL),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style::chip(bar.palette.accent_soft, bar.palette.accent))
    .on_press(Message::Mode(bar.mode.next()));

    let workspaces = Row::with_children((0..bar.status.workspaces).map(|i| {
        button(
            text((i + 1).to_string())
                .font(mono)
                .size(font::SMALL)
                .center(),
        )
        .width(26)
        .height(24)
        .padding(0)
        .style(style::workspace(
            i == bar.active,
            bar.status.busy.contains(&i),
        ))
        .on_press(Message::Workspace(i))
        .into()
    }))
    .spacing(2);

    let clock = row![
        text(bar.now.format("%a %-d %b").to_string()).color(muted),
        text(bar.now.format("%H:%M").to_string()).font(mono),
    ]
    .spacing(10);

    let mut status = Row::new().spacing(14).align_y(Alignment::Center);
    if let Some(media) = bar.status.media {
        status = status.push(text(format!("♪ {media}")).color(muted).size(font::SMALL));
    }
    status = status.push(text(bar.status.keyboard).font(mono).size(font::SMALL));
    if let Some(level) = bar.status.battery {
        status = status.push(text(format!("{level}%")).font(mono).size(font::SMALL));
    }

    container(
        row![
            container(row![chip, workspaces].spacing(8).align_y(Alignment::Center)).width(Fill),
            clock,
            container(status).width(Fill).align_x(Alignment::End),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(metrics::BAR_HEIGHT)
    .padding([0, 6])
    .align_y(Alignment::Center)
    .style(style::bar)
    .into()
}

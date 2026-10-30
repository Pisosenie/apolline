use apolline_theme::{Palette, Rgba, neutral};
use iced::widget::operation::{focus, scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use iced::widget::{
    Column, Space, button, column, container, image, mouse_area, opaque, row, scrollable, space,
    text, text_input,
};
use iced::{Alignment, Background, Border, ContentFit, Element, Fill, Length, Padding, Task};

use crate::clipboard::{Clip, Kind, png_size};
use crate::icon::{Icon, icon};
use crate::media::shorten;
use crate::popup::ago;
use crate::shell::Message;
use crate::style;

const WIDTH: f32 = 820.0;
const ROW: f32 = 48.0;
const SEARCH: &str = "clips-search";
const LIST: &str = "clips-list";

#[derive(Default)]
pub struct Picker {
    pub query: String,
    pub selected: usize,
}

impl Picker {
    pub fn open(&mut self) -> Task<Message> {
        self.query.clear();
        self.selected = 0;
        Task::batch([
            focus(SEARCH),
            scroll_to(LIST, AbsoluteOffset { x: 0.0, y: 0.0 }),
        ])
    }

    pub fn focus(&self) -> Task<Message> {
        focus(SEARCH)
    }

    pub fn search(&mut self, query: String) -> Task<Message> {
        self.query = query;
        self.selected = 0;
        scroll_to(LIST, AbsoluteOffset { x: 0.0, y: 0.0 })
    }

    pub fn visible(&self, clips: &[Clip]) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| {
                query.is_empty()
                    || clip
                        .text()
                        .unwrap_or("image")
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn chosen(&self, clips: &[Clip]) -> Option<usize> {
        self.visible(clips).get(self.selected).copied()
    }

    pub fn step(&mut self, by: isize, clips: &[Clip]) -> Task<Message> {
        let count = self.visible(clips).len();
        if count == 0 {
            return Task::none();
        }
        self.selected = (self.selected as isize + by).rem_euclid(count as isize) as usize;
        let top = self.selected.saturating_sub(3) as f32 * ROW;
        scroll_to(LIST, AbsoluteOffset { x: 0.0, y: top })
    }

    pub fn view<'a>(&'a self, clips: &'a [Clip], palette: Palette) -> Element<'a, Message> {
        let visible = self.visible(clips);
        let pinned = clips.iter().filter(|c| c.pinned).count();
        let count = match (clips.len(), pinned) {
            (1, 0) => "1 item".to_owned(),
            (n, 0) => format!("{n} items"),
            (n, p) => format!("{n} items · {p} pinned"),
        };
        let header = row![
            icon(Icon::Search, 18.0, style::FAINT),
            text_input("Search clipboard", &self.query)
                .id(SEARCH)
                .on_input(Message::ClipQuery)
                .on_submit(Message::ClipCopy(None))
                .size(16)
                .padding(0)
                .style(style::bare_field(palette)),
            text(count).size(12).color(style::color(style::FAINT)),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let body: Element<'a, Message> = if visible.is_empty() {
            container(
                column![
                    icon(Icon::Clipboard, 28.0, style::FAINT),
                    text(if clips.is_empty() {
                        "Copy something and it shows up here".to_owned()
                    } else {
                        format!("Nothing matches “{}”", self.query.trim())
                    })
                    .size(14)
                    .color(style::color(style::SOFT_TEXT)),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .center_x(Fill)
            .padding([60, 0])
            .into()
        } else {
            let list =
                Column::with_children(visible.iter().enumerate().map(|(position, &index)| {
                    entry(&clips[index], index, position == self.selected)
                }));
            let selected = visible
                .get(self.selected)
                .or(visible.first())
                .map(|&index| (index, &clips[index]));
            row![
                container(
                    scrollable(list)
                        .id(LIST)
                        .direction(style::thin())
                        .style(style::scroller)
                        .height(400)
                )
                .width(330)
                .padding(8),
                container(Space::new().width(1).height(Fill))
                    .style(|_| container::background(style::color(neutral::HAIRLINE))),
                match selected {
                    Some((index, clip)) => preview(index, clip, palette),
                    None => Space::new().into(),
                },
            ]
            .height(416)
            .into()
        };

        let footer = row![
            hint("↑↓", "move"),
            hint("↵", "copy"),
            hint("del", "remove"),
            hint("esc", "close"),
            space::horizontal(),
            text("Kept in memory only, gone when you log out")
                .size(11)
                .color(style::color(style::FAINT)),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let panel = container(
            column![
                container(header).padding([16, 18]),
                divider(),
                body,
                divider(),
                container(footer).padding([10, 18]),
            ]
            .width(WIDTH),
        )
        .style(style::surface);

        mouse_area(
            container(opaque(panel))
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .padding(Padding::new(0.0).top(150.0))
                .style(style::scrim),
        )
        .on_press(Message::Overlay(None))
        .into()
    }
}

fn divider<'a>() -> Element<'a, Message> {
    container(Space::new().height(1))
        .width(Fill)
        .style(|_| container::background(style::color(neutral::HAIRLINE)))
        .into()
}

fn glyph<'a>(clip: &Clip) -> Element<'a, Message> {
    let boxed = |content: Element<'a, Message>, fill: Rgba| {
        container(content)
            .center(26)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(fill))),
                border: Border {
                    radius: 7.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
    };
    match clip.kind() {
        Kind::Text => boxed(
            text("T")
                .size(12)
                .font(style::mono())
                .color(style::color(style::SOFT_TEXT))
                .into(),
            style::FILL,
        ),
        Kind::Link => boxed(icon(Icon::Link, 13.0, style::SOFT_TEXT).into(), style::FILL),
        Kind::Image => boxed(
            icon(Icon::Image, 13.0, style::SOFT_TEXT).into(),
            style::FILL,
        ),
        Kind::Color => {
            let color = clip
                .text()
                .and_then(|t| Rgba::parse(t.trim()))
                .unwrap_or(neutral::INK);
            boxed(Space::new().into(), color)
        }
    }
}

fn entry(clip: &Clip, index: usize, selected: bool) -> Element<'_, Message> {
    let mut line = row![
        glyph(clip),
        text(shorten(&clip.line(), 24))
            .size(13)
            .font(style::mono())
            .width(Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if clip.pinned {
        line = line.push(icon(Icon::Pin, 12.0, style::FAINT));
    }
    line = line.push(
        text(ago(clip.when.elapsed()))
            .size(11)
            .color(style::color(style::FAINT)),
    );
    button(container(line).height(ROW - 4.0).align_y(Alignment::Center))
        .width(Fill)
        .padding([2, 10])
        .style(move |theme, status| {
            let mut style = style::ghost(10.0)(theme, status);
            if selected {
                style.background = Some(Background::Color(style::color(style::HOVER)));
            }
            style
        })
        .on_press(Message::ClipCopy(Some(index)))
        .into()
}

fn preview(index: usize, clip: &Clip, palette: Palette) -> Element<'_, Message> {
    let kind = match clip.kind() {
        Kind::Text => "TEXT",
        Kind::Link => "LINK",
        Kind::Color => "COLOR",
        Kind::Image => "IMAGE",
    };
    let pin_tint = if clip.pinned {
        palette.accent
    } else {
        style::FAINT
    };
    let top = row![
        text(kind)
            .size(10)
            .font(style::semibold())
            .color(style::color(style::FAINT)),
        space::horizontal(),
        button(
            row![
                icon(Icon::Pin, 13.0, pin_tint),
                text(if clip.pinned { "Pinned" } else { "Pin" })
                    .size(12)
                    .color(style::color(pin_tint))
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        )
        .padding([4, 10])
        .style(style::ghost(8.0))
        .on_press(Message::ClipPin(index)),
        button(icon(Icon::X, 13.0, style::FAINT))
            .padding(6)
            .style(style::ghost(8.0))
            .on_press(Message::ClipRemove(index)),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let (content, meta): (Element<'_, Message>, String) = match (clip.kind(), clip.text()) {
        (Kind::Color, Some(value)) => {
            let color = Rgba::parse(value.trim()).unwrap_or(neutral::INK);
            let swatch = container(Space::new())
                .width(Fill)
                .height(96)
                .style(move |_| container::Style {
                    background: Some(Background::Color(style::color(color))),
                    border: Border {
                        radius: 12.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                });
            let rgb = format!("rgb({}, {}, {})", color.r, color.g, color.b);
            (
                column![
                    swatch,
                    text(color.to_hex()).size(14).font(style::mono()),
                    text(rgb.clone())
                        .size(13)
                        .font(style::mono())
                        .color(style::color(style::SOFT_TEXT)),
                ]
                .spacing(10)
                .into(),
                "color".to_owned(),
            )
        }
        (_, Some(value)) => {
            let characters = value.chars().count();
            let lines = value.lines().count();
            let meta = match lines {
                0 | 1 => format!("{characters} characters"),
                n => format!("{characters} characters · {n} lines"),
            };
            (
                scrollable(text(value).size(13).font(style::mono()).line_height(1.5))
                    .direction(style::thin())
                    .style(style::scroller)
                    .height(Fill)
                    .into(),
                meta,
            )
        }
        (_, None) => {
            let meta = match &clip.content {
                crate::clipboard::Content::Png(bytes) => match png_size(bytes) {
                    Some((w, h)) => format!("{w}×{h} PNG · {} KB", bytes.len() / 1024),
                    None => format!("PNG · {} KB", bytes.len() / 1024),
                },
                crate::clipboard::Content::Text(_) => String::new(),
            };
            let picture: Element<'_, Message> = match &clip.picture {
                Some(handle) => image(handle.clone())
                    .width(Fill)
                    .height(Fill)
                    .content_fit(ContentFit::Contain)
                    .into(),
                None => Space::new().into(),
            };
            (picture, meta)
        }
    };

    column![
        top,
        container(content).height(Fill).width(Fill),
        text(meta).size(11).color(style::color(style::FAINT)),
    ]
    .spacing(12)
    .padding(18)
    .width(Length::Fill)
    .into()
}

fn hint<'a>(key: &'a str, label: &'a str) -> Element<'a, Message> {
    row![
        container(text(key).size(11).font(style::mono()))
            .padding([1, 6])
            .style(|_| container::Style {
                border: Border {
                    color: style::color(neutral::RAISED),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..container::Style::default()
            }),
        text(label).size(12).color(style::color(style::FAINT)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::sample;

    #[test]
    fn search_narrows_and_enter_takes_the_highlighted_clip() {
        let clips = sample();
        let mut picker = Picker::default();
        assert_eq!(picker.visible(&clips).len(), 5);
        let _ = picker.search("ssh".into());
        assert_eq!(picker.chosen(&clips), Some(4));
        let _ = picker.search("".into());
        let _ = picker.step(-1, &clips);
        assert_eq!(picker.chosen(&clips), Some(4));
    }
}

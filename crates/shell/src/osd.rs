use apolline_theme::{Palette, Rgba, neutral};
use iced::widget::{Space, container, row, text};
use iced::{Alignment, Element, Fill, Length};

use crate::controls::Osd;
use crate::icon::{Icon, icon};
use crate::shell::Message;
use crate::style;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 52;
pub const SURFACE: (u32, u32) = (WIDTH + 48, HEIGHT + 48);
pub const SHOWN_FOR: std::time::Duration = std::time::Duration::from_millis(1600);

pub fn view(osd: &Osd, palette: Palette) -> Element<'_, Message> {
    let (glyph, level, label) = match osd {
        Osd::Volume(volume) => (
            Icon::volume(volume.level, volume.muted),
            Some(if volume.muted { 0.0 } else { volume.level }),
            if volume.muted {
                "muted".to_owned()
            } else {
                format!("{:.0}", volume.level * 100.0)
            },
        ),
        Osd::Brightness(percent) => (
            Icon::Sun,
            Some(f32::from(*percent) / 100.0),
            percent.to_string(),
        ),
        Osd::Mic(muted) => (
            if *muted { Icon::MicOff } else { Icon::Mic },
            None,
            if *muted {
                "Microphone off"
            } else {
                "Microphone on"
            }
            .to_owned(),
        ),
        Osd::Layout(name) => (Icon::Keyboard, None, name.clone()),
    };
    let body: Element<'_, Message> = match level {
        Some(level) => {
            let filled = (level.clamp(0.0, 1.0) * 1000.0).round() as u16;
            row![
                container(Space::new().height(4))
                    .width(Length::FillPortion(filled.max(1)))
                    .style(style::pill(palette.accent)),
                container(Space::new().height(4))
                    .width(Length::FillPortion((1000 - filled).max(1)))
                    .style(style::pill(Rgba::hex(0xffffff).alpha(0.14))),
            ]
            .width(Fill)
            .into()
        }
        None => text(label.clone())
            .size(14)
            .font(style::medium())
            .width(Fill)
            .into(),
    };
    let mut line = row![icon(glyph, 17.0, neutral::TEXT), body]
        .spacing(14)
        .align_y(Alignment::Center);
    if level.is_some() {
        line = line.push(
            text(label)
                .size(12)
                .font(style::mono())
                .color(style::color(style::SOFT_TEXT))
                .width(Length::Fixed(40.0))
                .align_x(Alignment::End),
        );
    }
    let pill = container(line)
        .width(WIDTH as f32)
        .height(HEIGHT as f32)
        .padding([0, 18])
        .align_y(Alignment::Center)
        .style(|theme| {
            let mut style = style::surface(theme);
            style.border.radius = 16.0.into();
            style
        });
    container(pill)
        .center_x(SURFACE.0 as f32)
        .center_y(SURFACE.1 as f32)
        .into()
}

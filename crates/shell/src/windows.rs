use apolline_theme::Palette;
use iced::widget::{container, image, svg, text};
use iced::{Background, Border, Element};

use crate::apps::{self, App, Picture};
use crate::shell::Message;
use crate::style;

pub fn picture<'a>(
    apps: &'a [App],
    app_id: &str,
    size: f32,
    palette: Palette,
) -> Element<'a, Message> {
    match apps::find(apps, app_id).and_then(|app| app.icon.as_ref()) {
        Some(Picture::Raster(path)) => image(image::Handle::from_path(path))
            .width(size)
            .height(size)
            .into(),
        Some(Picture::Vector(path)) => svg(svg::Handle::from_path(path))
            .width(size)
            .height(size)
            .into(),
        None => {
            let letter = app_id
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".into());
            container(
                text(letter)
                    .size(size * 0.45)
                    .font(style::semibold())
                    .color(style::color(palette.accent)),
            )
            .center(size)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(palette.accent_soft))),
                border: Border {
                    radius: (size * 0.28).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
        }
    }
}

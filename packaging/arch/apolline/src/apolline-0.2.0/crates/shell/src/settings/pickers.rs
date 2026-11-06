use super::*;
use iced::widget::column;

pub(super) fn accent_picker<'a>(
    current: Option<Accent>,
    auto: Rgba,
    inherit: bool,
    on: fn(Option<Accent>) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    let mut swatches = Row::new().spacing(10).align_y(Alignment::Center);
    if inherit {
        swatches = swatches.push(
            button(text("Desktop's").size(12))
                .padding([6, 14])
                .style(style::choice(current.is_none()))
                .on_press(on(None)),
        );
    }
    swatches = swatches.push(
        button(
            row![
                container(Space::new().width(10).height(10)).style(style::pill(auto)),
                text("From wallpaper").size(12),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([6, 14])
        .style(style::choice(current == Some(Accent::Auto)))
        .on_press(on(Some(Accent::Auto))),
    );
    for color in ACCENTS {
        let chosen = current == Some(Accent::Fixed(color));
        swatches = swatches.push(
            button(
                container(if chosen {
                    Element::from(icon(Icon::Check, 14.0, neutral::ink()))
                } else {
                    space().into()
                })
                .center(26),
            )
            .padding(0)
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(style::color(color))),
                border: Border {
                    color: style::color(neutral::text()),
                    width: if chosen || status == button::Status::Hovered {
                        2.0
                    } else {
                        0.0
                    },
                    radius: style::PILL.into(),
                },
                ..button::Style::default()
            })
            .on_press(on(Some(Accent::Fixed(color)))),
        );
    }
    let (badge, tint) = if readable(palette) {
        ("Readable on any wallpaper", style::GOOD)
    } else {
        ("Low contrast", style::WARN)
    };
    column![
        swatches
            .push(space::horizontal())
            .push(text(badge).size(12).color(style::color(tint))),
        text("The color is taken from the picture and lightened until it reads on glass.")
            .size(12)
            .color(style::color(style::faint())),
    ]
    .spacing(10)
    .into()
}

// A small screen with a spot in each corner and the middle of each edge for the cards.
pub(super) fn corner_picker<'a>(chosen: Corner, palette: Palette) -> Element<'a, Message> {
    let spot = move |corner: Corner| -> Element<'a, Message> {
        let on = corner == chosen;
        button(space().width(22).height(10))
            .padding(0)
            .style(move |_: &Theme, status| button::Style {
                background: Some(Background::Color(style::color(if on {
                    palette.accent
                } else if status == button::Status::Hovered {
                    neutral::text().alpha(0.35)
                } else {
                    neutral::text().alpha(0.14)
                }))),
                border: Border::default().rounded(4),
                ..button::Style::default()
            })
            .on_press(Message::CardsCorner(corner))
            .into()
    };
    let edge = |a, b, c| {
        row![
            spot(a),
            space::horizontal(),
            spot(b),
            space::horizontal(),
            spot(c)
        ]
    };
    container(
        column![
            edge(Corner::TopLeft, Corner::Top, Corner::TopRight),
            space::vertical(),
            edge(Corner::BottomLeft, Corner::Bottom, Corner::BottomRight),
        ]
        .padding(8),
    )
    .width(128)
    .height(78)
    .style(|_| container::Style {
        background: Some(Background::Color(style::color(style::sunken_color()))),
        border: Border {
            color: style::color(neutral::hairline()),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

// Each palette as a little screen: its background, a card on it, a line of its text and its accent.
pub(super) fn palette_picker<'a>(chosen: &str, palette: Palette) -> Element<'a, Message> {
    let wallpaper = auto_accent("fog.jpg");
    let named = apolline_theme::palettes::NAMED
        .iter()
        .map(|(name, scheme, accent)| (*name, *scheme, *accent))
        .chain([
            (
                "wallpaper",
                apolline_theme::palettes::tonal(wallpaper, false),
                wallpaper,
            ),
            (
                "wallpaper-light",
                apolline_theme::palettes::tonal(wallpaper, true),
                wallpaper,
            ),
        ]);
    let cards = named.map(|(name, scheme, accent)| {
        let on = name == chosen;
        let accent = apolline_theme::palettes::readable(accent, &scheme);
        let sample = container(
            container(
                column![
                    container(space().width(36).height(4))
                        .style(style::pill(scheme.text.alpha(0.85))),
                    container(space().width(24).height(4))
                        .style(style::pill(scheme.text.alpha(0.45))),
                    container(space().width(14).height(8)).style(style::pill(accent)),
                ]
                .spacing(5),
            )
            .padding(8)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(scheme.surface))),
                border: Border::default().rounded(6),
                ..container::Style::default()
            }),
        )
        .padding(8)
        .width(Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(style::color(scheme.ink))),
            border: Border::default().rounded(8),
            ..container::Style::default()
        });
        button(
            column![sample, text(palette_name(name)).size(11)]
                .spacing(6)
                .width(118),
        )
        .padding(4)
        .style(move |_: &Theme, status| button::Style {
            background: Some(Background::Color(style::color(
                if status == button::Status::Hovered {
                    style::fill_color()
                } else {
                    neutral::text().alpha(0.0)
                },
            ))),
            border: Border {
                color: style::color(if on {
                    palette.accent
                } else {
                    neutral::text().alpha(0.0)
                }),
                width: 2.0,
                radius: 12.0.into(),
            },
            text_color: style::color(neutral::text()),
            ..button::Style::default()
        })
        .on_press(Message::Palette(name.to_owned()))
        .into()
    });
    column![
        text("Colors").size(13).font(style::medium()),
        Row::with_children(cards).spacing(6).wrap(),
    ]
    .spacing(10)
    .into()
}

pub(super) fn palette_name(name: &str) -> String {
    match name {
        "dark" => "Apolline dark".to_owned(),
        "light" => "Apolline light".to_owned(),
        "wallpaper" => "From the wallpaper".to_owned(),
        "wallpaper-light" => "Wallpaper, light".to_owned(),
        name => name
            .split('-')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

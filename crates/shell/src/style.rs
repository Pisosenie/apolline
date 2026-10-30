use apolline_theme::{Palette, Rgba, font, metrics, neutral};
use iced::font::Weight;
use iced::widget::overlay::menu;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{button, container, pick_list, scrollable, slider, text_input, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

pub const SURFACE: Rgba = Rgba::hex(0x15171b).alpha(0.95);
pub const SUNKEN: Rgba = Rgba::hex(0xffffff).alpha(0.04);
pub const FILL: Rgba = Rgba::hex(0xffffff).alpha(0.07);
pub const HOVER: Rgba = Rgba::hex(0xffffff).alpha(0.11);
pub const SOFT_TEXT: Rgba = neutral::TEXT.alpha(0.86);
pub const FAINT: Rgba = neutral::TEXT.alpha(0.5);
pub const GOOD: Rgba = Rgba::hex(0x5dd39e);
pub const WARN: Rgba = Rgba::hex(0xf2b35c);
pub const DANGER: Rgba = Rgba::hex(0xff7a6b);
pub const RADIUS_SURFACE: f32 = 20.0;
pub const RADIUS_TILE: f32 = 14.0;
pub const PILL: f32 = 999.0;

const WHITE: Rgba = Rgba::hex(0xffffff);

pub fn color(c: Rgba) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn fill(c: Rgba) -> Option<Background> {
    Some(Background::Color(color(c)))
}

fn rounded(radius: f32) -> Border {
    Border {
        radius: radius.into(),
        ..Border::default()
    }
}

fn outlined(c: Rgba, radius: f32) -> Border {
    Border {
        color: color(c),
        width: 1.0,
        radius: radius.into(),
    }
}

fn lifted(depth: f32) -> Shadow {
    Shadow {
        color: Color::from_rgba8(0, 0, 0, 0.42),
        offset: Vector::new(0.0, depth * 0.5),
        blur_radius: depth * 1.5,
    }
}

pub fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..Font::with_name(font::UI)
    }
}

pub fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::with_name(font::UI)
    }
}

pub fn light() -> Font {
    Font {
        weight: Weight::ExtraLight,
        ..Font::with_name(font::UI)
    }
}

pub fn mono() -> Font {
    Font::with_name(font::MONO)
}

pub fn bar(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(color(neutral::TEXT)),
        background: fill(neutral::GLASS),
        border: outlined(neutral::HAIRLINE, metrics::RADIUS_BAR),
        ..container::Style::default()
    }
}

pub fn surface(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(color(neutral::TEXT)),
        background: fill(SURFACE),
        border: outlined(neutral::HAIRLINE, RADIUS_SURFACE),
        shadow: lifted(24.0),
        ..container::Style::default()
    }
}

pub fn card(_: &Theme) -> container::Style {
    container::Style {
        background: fill(SUNKEN),
        border: outlined(neutral::HAIRLINE, RADIUS_TILE),
        ..container::Style::default()
    }
}

pub fn pill(background: Rgba) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: fill(background),
        border: rounded(PILL),
        ..container::Style::default()
    }
}

pub fn dot(c: Rgba) -> impl Fn(&Theme) -> container::Style {
    pill(c)
}

pub fn scrim(_: &Theme) -> container::Style {
    container::background(Color::from_rgba8(6, 7, 9, 0.62))
}

pub fn ghost(radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: match status {
            button::Status::Hovered => fill(FILL),
            button::Status::Pressed => fill(HOVER),
            _ => None,
        },
        text_color: color(neutral::TEXT),
        border: rounded(radius),
        ..button::Style::default()
    }
}

pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: fill(match status {
            button::Status::Hovered | button::Status::Pressed => HOVER,
            _ => FILL,
        }),
        text_color: color(neutral::TEXT),
        border: rounded(metrics::RADIUS_CONTROL + 2.0),
        ..button::Style::default()
    }
}

pub fn primary(palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: fill(if hovered {
                WHITE.alpha(0.14).over(palette.accent)
            } else {
                palette.accent
            }),
            text_color: color(palette.on_accent),
            border: rounded(metrics::RADIUS_CONTROL + 2.0),
            shadow: if hovered {
                Shadow {
                    color: color(palette.accent.alpha(0.35)),
                    offset: Vector::ZERO,
                    blur_radius: 14.0,
                }
            } else {
                Shadow::default()
            },
            ..button::Style::default()
        }
    }
}

pub fn danger(armed: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: fill(match (armed, status) {
            (true, _) => DANGER,
            (_, button::Status::Hovered | button::Status::Pressed) => HOVER,
            _ => FILL,
        }),
        text_color: color(if armed { neutral::INK } else { neutral::TEXT }),
        border: rounded(PILL),
        ..button::Style::default()
    }
}

pub fn tile(on: bool, palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = match (on, hovered) {
            (true, false) => palette.accent,
            (true, true) => WHITE.alpha(0.14).over(palette.accent),
            (false, false) => FILL,
            (false, true) => HOVER,
        };
        button::Style {
            background: fill(background),
            text_color: color(if on { palette.on_accent } else { neutral::TEXT }),
            border: rounded(RADIUS_TILE),
            ..button::Style::default()
        }
    }
}

pub fn choice(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: match (on, hovered) {
                (true, _) => fill(WHITE.alpha(0.14)),
                (false, true) => fill(FILL),
                (false, false) => None,
            },
            text_color: color(if on { neutral::TEXT } else { SOFT_TEXT }),
            border: rounded(metrics::RADIUS_CONTROL),
            shadow: if on { lifted(4.0) } else { Shadow::default() },
            ..button::Style::default()
        }
    }
}

pub fn segments(_: &Theme) -> container::Style {
    container::Style {
        background: fill(Rgba::hex(0x000000).alpha(0.28)),
        border: outlined(neutral::HAIRLINE, metrics::RADIUS_CONTROL + 3.0),
        ..container::Style::default()
    }
}

pub fn switch(palette: Palette) -> impl Fn(&Theme, toggler::Status) -> toggler::Style {
    move |_, status| {
        let (on, hovered) = match status {
            toggler::Status::Active { is_toggled } | toggler::Status::Disabled { is_toggled } => {
                (is_toggled, false)
            }
            toggler::Status::Hovered { is_toggled } => (is_toggled, true),
        };
        let track = match (on, hovered) {
            (true, false) => palette.accent,
            (true, true) => WHITE.alpha(0.14).over(palette.accent),
            (false, false) => WHITE.alpha(0.16),
            (false, true) => WHITE.alpha(0.22),
        };
        toggler::Style {
            background: Background::Color(color(track)),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Background::Color(color(WHITE)),
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.12,
        }
    }
}

pub fn rail(palette: Palette, thickness: f32) -> impl Fn(&Theme, slider::Status) -> slider::Style {
    move |_, status| slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(color(palette.accent)),
                Background::Color(color(WHITE.alpha(0.14))),
            ),
            width: thickness,
            border: rounded(PILL),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: match status {
                    slider::Status::Active => thickness.max(6.0) + 1.0,
                    _ => thickness.max(6.0) + 2.0,
                },
            },
            background: Background::Color(color(WHITE)),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

pub fn field(palette: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let edge = match status {
            text_input::Status::Focused { .. } => palette.accent.alpha(0.7),
            text_input::Status::Hovered => WHITE.alpha(0.16),
            _ => neutral::HAIRLINE,
        };
        text_input::Style {
            background: Background::Color(color(Rgba::hex(0x000000).alpha(0.28))),
            border: outlined(edge, metrics::RADIUS_CONTROL + 2.0),
            icon: color(FAINT),
            placeholder: color(FAINT),
            value: color(neutral::TEXT),
            selection: color(palette.accent.alpha(0.35)),
        }
    }
}

pub fn thin() -> Direction {
    Direction::Vertical(Scrollbar::new().width(4).scroller_width(4).spacing(10))
}

pub fn scroller(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let grip = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => 0.24,
        _ => 0.12,
    };
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = None;
        rail.border = Border::default();
        rail.scroller.background = Background::Color(color(WHITE.alpha(grip)));
        rail.scroller.border = rounded(PILL);
    }
    style
}

pub fn picker(palette: Palette) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let edge = match status {
            pick_list::Status::Opened { .. } => palette.accent.alpha(0.7),
            pick_list::Status::Hovered => WHITE.alpha(0.16),
            pick_list::Status::Active => neutral::HAIRLINE,
        };
        pick_list::Style {
            text_color: color(neutral::TEXT),
            placeholder_color: color(SOFT_TEXT),
            handle_color: color(FAINT),
            background: Background::Color(color(FILL)),
            border: outlined(edge, metrics::RADIUS_CONTROL + 2.0),
        }
    }
}

pub fn menu(palette: Palette) -> impl Fn(&Theme) -> menu::Style {
    move |_| menu::Style {
        background: Background::Color(color(Rgba::hex(0x1b1d22))),
        border: outlined(neutral::HAIRLINE, metrics::RADIUS_CONTROL + 2.0),
        text_color: color(neutral::TEXT),
        selected_text_color: color(palette.on_accent),
        selected_background: Background::Color(color(palette.accent)),
        shadow: lifted(16.0),
    }
}

pub fn bare_field(palette: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: color(FAINT),
        placeholder: color(FAINT),
        value: color(neutral::TEXT),
        selection: color(palette.accent.alpha(0.35)),
    }
}

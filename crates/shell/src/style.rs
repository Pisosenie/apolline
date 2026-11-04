use apolline_theme::{Palette, Rgba, font, metrics, neutral};
use iced::font::Weight;
use iced::widget::overlay::menu;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{button, container, pick_list, scrollable, slider, text_input, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

pub fn surface_color() -> Rgba {
    apolline_theme::scheme().surface
}

pub fn sunken_color() -> Rgba {
    apolline_theme::scheme().overlay.alpha(0.04)
}

pub fn fill_color() -> Rgba {
    apolline_theme::scheme().overlay.alpha(0.07)
}

pub fn hover_color() -> Rgba {
    apolline_theme::scheme().overlay.alpha(0.11)
}

pub fn soft_text() -> Rgba {
    neutral::text().alpha(0.86)
}

pub fn faint() -> Rgba {
    neutral::text().alpha(0.5)
}

pub const GOOD: Rgba = Rgba::hex(0x5dd39e);
pub const WARN: Rgba = Rgba::hex(0xf2b35c);
pub const DANGER: Rgba = Rgba::hex(0xff7a6b);
pub const RADIUS_SURFACE: f32 = 20.0;
pub const RADIUS_TILE: f32 = 14.0;
pub const PILL: f32 = 999.0;

// Lines, tracks and fills: white on dark schemes, black on light ones.
fn ink(a: f32) -> Rgba {
    apolline_theme::scheme().overlay.alpha(a)
}

// What a field or a well is cut into: darker on dark, lighter on light.
fn well() -> Rgba {
    if apolline_theme::scheme().light {
        Rgba::hex(0xffffff).alpha(0.6)
    } else {
        Rgba::hex(0x000000).alpha(0.28)
    }
}

// The knob of a switch or a slider stays white: it reads on the accent and on the track.
const KNOB: Rgba = Rgba::hex(0xffffff);

// iced's own theme, made from the current scheme so default text, scrollbars and fields follow it;
// kept until the scheme changes.
pub fn theme() -> Theme {
    static MADE: std::sync::Mutex<Option<(apolline_theme::Scheme, Theme)>> =
        std::sync::Mutex::new(None);
    let scheme = apolline_theme::scheme();
    let Ok(mut made) = MADE.lock() else {
        return Theme::Dark;
    };
    if let Some((was, theme)) = made.as_ref()
        && *was == scheme
    {
        return theme.clone();
    }
    let theme = Theme::custom(
        "Apolline",
        iced::theme::Palette {
            background: color(scheme.window),
            text: color(scheme.text),
            primary: color(apolline_theme::ACCENTS[0]),
            success: color(GOOD),
            warning: color(WARN),
            danger: color(DANGER),
        },
    );
    *made = Some((scheme, theme.clone()));
    theme
}

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

static FAMILY: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

thread_local! {
    // The font of the surface being built, while it is.
    static SCOPE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

// Builds a surface's widgets in its own font, when it has one.
pub fn within<T>(font: Option<apolline_config::FontName>, build: impl FnOnce() -> T) -> T {
    let was = SCOPE.replace(font.map(apolline_config::FontName::name));
    let built = build();
    SCOPE.set(was);
    built
}

pub fn family() -> &'static str {
    SCOPE
        .get()
        .unwrap_or_else(|| FAMILY.get().copied().unwrap_or(font::UI))
}

pub fn regular() -> Font {
    Font::with_name(family())
}

// Text in the font of the surface it's on, and in the language the shell speaks.
pub fn text<'a>(content: impl iced::widget::text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    let content = content.into_fragment();
    let content = match crate::i18n::find(&content) {
        Some(said) => std::borrow::Cow::Borrowed(said),
        None => content,
    };
    iced::widget::text(content).font(regular())
}

// Someone's own words, a copied line or a message, shown as they are and never translated.
pub fn verbatim<'a>(content: impl iced::widget::text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    iced::widget::text(content).font(regular())
}

pub fn choose_family(name: Option<String>) {
    if let Some(name) = name {
        let _ = FAMILY.set(Box::leak(name.into_boxed_str()));
    }
}

pub fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..Font::with_name(family())
    }
}

pub fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::with_name(family())
    }
}

pub fn light() -> Font {
    Font {
        weight: Weight::ExtraLight,
        ..Font::with_name(family())
    }
}

pub fn mono() -> Font {
    Font::with_name(font::MONO)
}

pub fn bar(_: &Theme) -> container::Style {
    glass(0.55, true, metrics::RADIUS_BAR)(&Theme::Dark)
}

pub fn glass(opacity: f32, floating: bool, radius: f32) -> impl Fn(&Theme) -> container::Style {
    // Light glass over a dark wallpaper turns grey and its dark text with it, so it's thicker.
    let opacity = if apolline_theme::scheme().light {
        0.85 + opacity * 0.15
    } else {
        opacity
    };
    move |_| container::Style {
        text_color: Some(color(neutral::text())),
        background: fill(neutral::ink().alpha(opacity)),
        border: if floating {
            outlined(neutral::hairline(), radius)
        } else {
            Border::default()
        },
        ..container::Style::default()
    }
}

// The bar's height on screen and its island's corners, from [style.bar].
pub fn bar_size(style: &apolline_config::Styles) -> (f32, f32) {
    let scale = style.bar.size.unwrap_or(1.0);
    let radius = style.bar.radius.map_or(metrics::RADIUS_BAR, f32::from);
    (metrics::BAR_HEIGHT * scale, radius)
}

// Where an overlay goes by the bar on `edge`: `along` is its place along that edge (the end for
// quick settings, the middle for the calendar), `gap` how far it keeps from the edge.
pub fn beside_bar<'a, M: 'a>(
    card: impl Into<iced::Element<'a, M>>,
    edge: apolline_config::Edge,
    along: iced::Alignment,
    gap: f32,
    inset: f32,
) -> container::Container<'a, M> {
    use apolline_config::Edge;
    use iced::Alignment::{End, Start};
    let (x, y, padding) = match edge {
        Edge::Top => (along, Start, iced::Padding::new(inset).top(gap)),
        Edge::Bottom => (along, End, iced::Padding::new(inset).bottom(gap)),
        Edge::Left => (
            Start,
            along_down(along),
            iced::Padding::new(inset).left(gap),
        ),
        Edge::Right => (End, along_down(along), iced::Padding::new(inset).right(gap)),
    };
    container(card)
        .width(iced::Fill)
        .height(iced::Fill)
        .align_x(x)
        .align_y(y)
        .padding(padding)
}

// Down a side bar, the end of the top edge is its top.
fn along_down(along: iced::Alignment) -> iced::Alignment {
    match along {
        iced::Alignment::End => iced::Alignment::Start,
        other => other,
    }
}

// How wide a bar down the side is, before its size.
pub const RAIL: f32 = 44.0;

pub fn rail_width(style: &apolline_config::Styles) -> f32 {
    RAIL * style.bar.size.unwrap_or(1.0)
}

// How far an overlay at `scale` keeps clear of the bar, in its own units.
pub fn under_bar(bar: &apolline_config::Bar, style: &apolline_config::Styles, scale: f32) -> f32 {
    let inset = if bar.floating {
        metrics::BAR_INSET
    } else {
        0.0
    };
    let thick = if bar.position.vertical() {
        rail_width(style)
    } else {
        bar_size(style).0
    };
    (inset + thick + 8.0) / scale
}

pub fn surface_as(look: apolline_config::Surface) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let mut style = surface(theme);
        if let Some(opacity) = look.opacity {
            style.background = fill(surface_color().alpha(opacity));
        }
        if let Some(radius) = look.radius {
            style.border.radius = f32::from(radius).into();
        }
        style
    }
}

pub fn surface(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(color(neutral::text())),
        background: fill(surface_color()),
        border: outlined(neutral::hairline(), RADIUS_SURFACE),
        shadow: lifted(24.0),
        ..container::Style::default()
    }
}

pub fn card(_: &Theme) -> container::Style {
    container::Style {
        background: fill(sunken_color()),
        border: outlined(neutral::hairline(), RADIUS_TILE),
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

pub fn tinted(background: Rgba) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: fill(background),
        border: rounded(RADIUS_TILE),
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
            button::Status::Hovered => fill(fill_color()),
            button::Status::Pressed => fill(hover_color()),
            _ => None,
        },
        text_color: color(neutral::text()),
        border: rounded(radius),
        ..button::Style::default()
    }
}

pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: fill(match status {
            button::Status::Hovered | button::Status::Pressed => hover_color(),
            _ => fill_color(),
        }),
        text_color: color(neutral::text()),
        border: rounded(metrics::RADIUS_CONTROL + 2.0),
        ..button::Style::default()
    }
}

pub fn primary(palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: fill(if hovered {
                KNOB.alpha(0.14).over(palette.accent)
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
            (_, button::Status::Hovered | button::Status::Pressed) => hover_color(),
            _ => fill_color(),
        }),
        text_color: color(if armed {
            neutral::ink()
        } else {
            neutral::text()
        }),
        border: rounded(PILL),
        ..button::Style::default()
    }
}

pub fn tile(on: bool, palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = match (on, hovered) {
            (true, false) => palette.accent,
            (true, true) => KNOB.alpha(0.14).over(palette.accent),
            (false, false) => fill_color(),
            (false, true) => hover_color(),
        };
        button::Style {
            background: fill(background),
            text_color: color(if on {
                palette.on_accent
            } else {
                neutral::text()
            }),
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
                (true, _) => fill(ink(0.14)),
                (false, true) => fill(fill_color()),
                (false, false) => None,
            },
            text_color: color(if on { neutral::text() } else { soft_text() }),
            border: rounded(metrics::RADIUS_CONTROL),
            shadow: if on { lifted(4.0) } else { Shadow::default() },
            ..button::Style::default()
        }
    }
}

pub fn segments(_: &Theme) -> container::Style {
    container::Style {
        background: fill(well()),
        border: outlined(neutral::hairline(), metrics::RADIUS_CONTROL + 3.0),
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
            (true, true) => KNOB.alpha(0.14).over(palette.accent),
            (false, false) => ink(0.16),
            (false, true) => ink(0.22),
        };
        toggler::Style {
            background: Background::Color(color(track)),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Background::Color(color(KNOB)),
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
                Background::Color(color(ink(0.14))),
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
            background: Background::Color(color(KNOB)),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

pub fn field(palette: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let edge = match status {
            text_input::Status::Focused { .. } => palette.accent.alpha(0.7),
            text_input::Status::Hovered => ink(0.16),
            _ => neutral::hairline(),
        };
        text_input::Style {
            background: Background::Color(color(well())),
            border: outlined(edge, metrics::RADIUS_CONTROL + 2.0),
            icon: color(faint()),
            placeholder: color(faint()),
            value: color(neutral::text()),
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
        rail.scroller.background = Background::Color(color(ink(grip)));
        rail.scroller.border = rounded(PILL);
    }
    style
}

pub fn picker(palette: Palette) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let edge = match status {
            pick_list::Status::Opened { .. } => palette.accent.alpha(0.7),
            pick_list::Status::Hovered => ink(0.16),
            pick_list::Status::Active => neutral::hairline(),
        };
        pick_list::Style {
            text_color: color(neutral::text()),
            placeholder_color: color(soft_text()),
            handle_color: color(faint()),
            background: Background::Color(color(fill_color())),
            border: outlined(edge, metrics::RADIUS_CONTROL + 2.0),
        }
    }
}

pub fn menu(palette: Palette) -> impl Fn(&Theme) -> menu::Style {
    move |_| menu::Style {
        background: Background::Color(color(neutral::window())),
        border: outlined(neutral::hairline(), metrics::RADIUS_CONTROL + 2.0),
        text_color: color(neutral::text()),
        selected_text_color: color(palette.on_accent),
        selected_background: Background::Color(color(palette.accent)),
        shadow: lifted(16.0),
    }
}

pub fn bare_field(palette: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: color(faint()),
        placeholder: color(faint()),
        value: color(neutral::text()),
        selection: color(palette.accent.alpha(0.35)),
    }
}

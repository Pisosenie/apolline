use apolline_config::Edge;
use apolline_theme::{Palette, neutral};
use iced::widget::{
    Column, Row, button, column, container, mouse_area, row, space, stack, tooltip,
};
use iced::{Alignment, Background, Border, Element, Padding, Point, mouse};

use crate::apps::{self, App};
use crate::dock::{Event, Item};
use crate::dock_shape::{self, DOTS, GAP, LINE, PAD, Shape};
use crate::drag::drag;
use crate::shell::Message;
use crate::style;

// The glass's corners unless [style.dock] says otherwise.
pub const RADIUS: f32 = 18.0;
const DOT: f32 = 4.0;
const WIDE_DOT: f32 = 12.0;

pub struct Look<'a> {
    pub items: Vec<Item>,
    pub apps: &'a [App],
    pub palette: Palette,
    pub edge: Edge,
    pub shape: Shape,
    // The dock's surface, along the edge and across it.
    pub surface: (f32, f32),
    // How far along the resting row the pointer is, when it's over this dock.
    pub pointer: Option<f32>,
    pub strength: f32,
    pub opacity: f32,
    pub radius: f32,
    pub output: Option<String>,
    // The app being dragged.
    pub held: Option<String>,
}

fn say(event: Event) -> Message {
    Message::Dock(event)
}

// The glass with its icons, placed on a surface that has room for them to grow and for their
// names to show; the pointer is followed over all of it.
pub fn view(look: Look<'_>) -> Element<'_, Message> {
    let sizes = look.shape.sizes(look.pointer, look.strength);
    let shift = look.pointer.map_or(0.0, |at| look.shape.shift(at, &sizes));
    let row_start = look.shape.row_start(look.surface.0);
    let lead = (row_start - PAD + shift).max(0.0);
    let starts = look.shape.starts();
    let middle = look.shape.length() / 2.0;
    let mut parts = Vec::new();
    for (index, (item, size)) in look.items.iter().zip(&sizes).enumerate() {
        if look.shape.split == Some(index) {
            parts.push(line(look.edge, look.shape.size));
        }
        let offset = starts[index] + look.shape.size / 2.0 - middle;
        parts.push(icon(&look, item, *size, offset));
    }
    let spacing = look.shape.spacing();
    let glass = |strip| glass(strip, look.opacity, look.radius);
    let placed: Element<'_, Message> = if look.edge.vertical() {
        let side = if look.edge == Edge::Left {
            Alignment::Start
        } else {
            Alignment::End
        };
        let strip = Column::with_children(parts).spacing(spacing).align_x(side);
        column![space().height(lead), glass(strip.into())].into()
    } else {
        let side = if look.edge == Edge::Top {
            Alignment::Start
        } else {
            Alignment::End
        };
        let strip = Row::with_children(parts).spacing(spacing).align_y(side);
        row![space().width(lead), glass(strip.into())].into()
    };
    let surface = on_surface(look.edge, look.surface, placed);
    let (edge, across) = (look.edge, look.surface.1);
    let along = move |at: Point| dock_shape::local(edge, across, at).0 - row_start;
    let (over, left) = (look.output.clone(), look.output);
    let followed = mouse_area(surface)
        .on_move(move |at| say(Event::Pointer(over.clone(), Some(along(at)))))
        .on_exit(say(Event::Pointer(left, None)));
    drag(
        followed,
        |at| say(Event::DragStart(at)),
        |at| say(Event::DragMove(at)),
        |at| say(Event::DragDrop(at)),
    )
    .into()
}

fn glass(strip: Element<'_, Message>, opacity: f32, radius: f32) -> Element<'_, Message> {
    container(strip)
        .padding(PAD)
        .style(style::glass(opacity, true, radius))
        .into()
}

// The surface, with the glass against the screen edge, a gap from it.
fn on_surface<'a>(
    edge: Edge,
    (along, across): (f32, f32),
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    use Alignment::{End, Start};
    let (x, y, padding) = match edge {
        Edge::Top => (Start, Start, Padding::ZERO.top(GAP)),
        Edge::Bottom => (Start, End, Padding::ZERO.bottom(GAP)),
        Edge::Left => (Start, Start, Padding::ZERO.left(GAP)),
        Edge::Right => (End, Start, Padding::ZERO.right(GAP)),
    };
    let (width, height) = if edge.vertical() {
        (across, along)
    } else {
        (along, across)
    };
    container(content)
        .width(width)
        .height(height)
        .align_x(x)
        .align_y(y)
        .padding(padding)
        .into()
}

// The thin line between the pinned apps and the others, across the middle of the icons.
fn line<'a>(edge: Edge, size: f32) -> Element<'a, Message> {
    let mark =
        container(space()).style(|_| container::background(style::color(neutral::hairline())));
    let dots = match edge {
        Edge::Top => Padding::ZERO.top(DOTS),
        Edge::Bottom => Padding::ZERO.bottom(DOTS),
        Edge::Left => Padding::ZERO.left(DOTS),
        Edge::Right => Padding::ZERO.right(DOTS),
    };
    let mark = if edge.vertical() {
        mark.width(size * 0.6).height(LINE)
    } else {
        mark.width(LINE).height(size * 0.6)
    };
    let room = container(mark).padding(dots);
    if edge.vertical() {
        room.width(size + DOTS).align_x(Alignment::Center).into()
    } else {
        room.height(size + DOTS).align_y(Alignment::Center).into()
    }
}

fn icon<'a>(look: &Look<'a>, item: &Item, size: f32, offset: f32) -> Element<'a, Message> {
    let palette = look.palette;
    let urgent = item.urgent();
    let glyph = container(crate::windows::picture(look.apps, &item.key, size, palette))
        .style(move |_| ring(urgent, palette, size));
    let mut face = stack![glyph];
    if item.unread > 0 {
        face = face.push(
            container(badge(item.unread, palette))
                .width(size)
                .height(size)
                .align_x(Alignment::End)
                .align_y(Alignment::Start),
        );
    }
    let marks = dots(item, size, palette, look.edge);
    let body: Element<'a, Message> = match look.edge {
        Edge::Top => column![marks, face].align_x(Alignment::Center).into(),
        Edge::Bottom => column![face, marks].align_x(Alignment::Center).into(),
        Edge::Left => row![marks, face].align_y(Alignment::Center).into(),
        Edge::Right => row![face, marks].align_y(Alignment::Center).into(),
    };
    let held = look.held.as_deref() == Some(item.key.as_str());
    let still = look.strength == 0.0;
    let key = item.key.clone();
    let pressed = button(body)
        .padding(0)
        .style(move |_, status| face_style(held, still, palette, size, status))
        .on_press(say(Event::Open(key.clone())));
    let scrolled = key.clone();
    let area = mouse_area(pressed)
        .on_middle_press(say(Event::Launch(key.clone())))
        .on_right_press(say(Event::Menu(key, offset)))
        .on_scroll(move |delta| say(Event::Scroll(scrolled.clone(), forward(delta))));
    let name =
        apps::find(look.apps, &item.key).map_or_else(|| item.key.clone(), |app| app.name.clone());
    let label = container(style::verbatim(name).size(12))
        .padding([4, 8])
        .style(style::surface);
    tooltip(area, label, toward_screen(look.edge)).gap(6).into()
}

// Down or to the right turns to the next window.
fn forward(delta: mouse::ScrollDelta) -> bool {
    match delta {
        mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => {
            if y.abs() >= x.abs() { y < 0.0 } else { x < 0.0 }
        }
    }
}

fn toward_screen(edge: Edge) -> tooltip::Position {
    match edge {
        Edge::Top => tooltip::Position::Bottom,
        Edge::Bottom => tooltip::Position::Top,
        Edge::Left => tooltip::Position::Right,
        Edge::Right => tooltip::Position::Left,
    }
}

// The ring the tray uses for an app asking for attention.
fn ring(urgent: bool, palette: Palette, size: f32) -> container::Style {
    if !urgent {
        return container::Style::default();
    }
    container::Style {
        border: Border {
            color: style::color(palette.accent),
            width: 2.0,
            radius: (size * 0.28).into(),
        },
        ..container::Style::default()
    }
}

fn face_style(
    held: bool,
    still: bool,
    palette: Palette,
    size: f32,
    status: button::Status,
) -> button::Style {
    let background = match status {
        _ if held => Some(palette.accent_soft),
        button::Status::Hovered if still => Some(style::fill_color()),
        button::Status::Pressed => Some(style::hover_color()),
        _ => None,
    };
    button::Style {
        background: background.map(|c| Background::Color(style::color(c))),
        text_color: style::color(neutral::text()),
        border: Border {
            radius: (size * 0.28).into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

// How many notifications wait, on the icon's corner.
fn badge<'a>(count: usize, palette: Palette) -> Element<'a, Message> {
    let shown = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    container(
        style::verbatim(shown)
            .size(10)
            .font(style::semibold())
            .color(style::color(palette.on_accent)),
    )
    .padding([1, 5])
    .style(style::pill(palette.accent))
    .into()
}

// A dot for each open window, up to three, the first wide in the accent when the app is in front.
fn dots<'a>(item: &Item, size: f32, palette: Palette, edge: Edge) -> Element<'a, Message> {
    let focused = item.focused();
    let marks = (0..item.windows.len().min(3)).map(|index| {
        let wide = index == 0 && focused;
        let (long, tint) = if wide {
            (WIDE_DOT, palette.accent)
        } else {
            (DOT, neutral::muted())
        };
        let mark = container(space()).style(style::pill(tint));
        if edge.vertical() {
            mark.width(DOT).height(long).into()
        } else {
            mark.width(long).height(DOT).into()
        }
    });
    if edge.vertical() {
        container(Column::with_children(marks).spacing(3))
            .width(DOTS)
            .height(size)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    } else {
        container(Row::with_children(marks).spacing(3))
            .width(size)
            .height(DOTS)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    }
}

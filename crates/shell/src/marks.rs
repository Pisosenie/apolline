use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::style::text;
use apolline_theme::{Palette, Rgba, neutral};
use iced::widget::canvas::{self, Frame, Geometry, Stroke};
use iced::widget::image::Handle;
use iced::widget::{button, canvas as board, column, container, image, row, space, stack};
use iced::{Alignment, ContentFit, Element, Fill, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::icon::{Icon, icon};
use crate::shell::Message;
use crate::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Arrow,
    Box,
    Marker,
    Hide,
    Text,
    Step,
}

const TOOLS: [(Tool, Icon); 7] = [
    (Tool::Pen, Icon::Pencil),
    (Tool::Arrow, Icon::MoveUpRight),
    (Tool::Box, Icon::Square),
    (Tool::Marker, Icon::Highlighter),
    (Tool::Text, Icon::Type),
    (Tool::Step, Icon::ListOrdered),
    (Tool::Hide, Icon::EyeOff),
];

// The first ink is the accent.
const INKS: [Option<Rgba>; 4] = [
    None,
    Some(Rgba::hex(0xff5a4e)),
    Some(Rgba::hex(0xffd23f)),
    Some(Rgba::hex(0xffffff)),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    tool: Tool,
    ink: Rgba,
    points: Vec<Point>,
    // What a text mark says, or a step's number.
    label: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Drawn(Mark),
    // Typing starts at a point in an ink, takes letters, loses the last one, and is placed or dropped.
    Write(Point, Rgba),
    Typed(String),
    Erase,
    Place,
    Drop,
    Tool(Tool),
    Ink(usize),
    Undo,
    Copy,
    Save,
    Both,
}

#[derive(Clone)]
pub struct Marks {
    pub file: PathBuf,
    width: u32,
    height: u32,
    picture: Handle,
    marks: Vec<Mark>,
    tool: Tool,
    ink: usize,
    typing: Option<Mark>,
}

impl std::fmt::Debug for Marks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Marks")
            .field("file", &self.file)
            .field("marks", &self.marks.len())
            .finish()
    }
}

pub async fn open(file: PathBuf) -> Result<Arc<Marks>, String> {
    tokio::task::spawn_blocking(move || Marks::load(file).map(Arc::new))
        .await
        .map_err(|err| err.to_string())?
}

// The newest picture in the screenshot folder, for marking one taken earlier.
pub fn latest() -> Option<PathBuf> {
    let marked = |path: &Path| {
        path.file_stem()
            .is_some_and(|stem| stem.to_string_lossy().ends_with(" marked"))
    };
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let pictures = std::env::var_os("XDG_PICTURES_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join("Pictures"));
    std::fs::read_dir(pictures.join("Screenshots"))
        .ok()?
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.extension().is_some_and(|e| e == "png") && !marked(&path)
        })
        .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .map(|entry| entry.path())
}

impl Marks {
    fn load(file: PathBuf) -> Result<Self, String> {
        let picture = ::image::open(&file)
            .map_err(|err| format!("{}: {err}", file.display()))?
            .into_rgba8();
        let (width, height) = picture.dimensions();
        Ok(Self {
            file,
            width,
            height,
            picture: Handle::from_rgba(width, height, picture.into_raw()),
            marks: Vec::new(),
            tool: Tool::Arrow,
            ink: 0,
            typing: None,
        })
    }

    pub fn update(&mut self, event: Event) {
        match event {
            Event::Drawn(mut mark) => {
                self.settle();
                if mark.tool == Tool::Hide
                    && let Some(Shape::Hide(rect, _)) = shapes(&mark, 1.0).first()
                {
                    mark.ink = self.tone(*rect);
                }
                if mark.tool == Tool::Step {
                    let steps = self.marks.iter().filter(|m| m.tool == Tool::Step).count();
                    mark.label = Some((steps + 1).to_string());
                }
                if !shapes(&mark, self.base()).is_empty() {
                    self.marks.push(mark);
                }
            }
            Event::Write(at, ink) => {
                self.settle();
                self.typing = Some(Mark {
                    tool: Tool::Text,
                    ink,
                    points: vec![at],
                    label: Some(String::new()),
                });
            }
            Event::Typed(text) => {
                if let Some(label) = self.typing.as_mut().and_then(|m| m.label.as_mut())
                    && label.chars().count() < 120
                {
                    label.push_str(&text);
                }
            }
            Event::Erase => {
                if let Some(label) = self.typing.as_mut().and_then(|m| m.label.as_mut()) {
                    label.pop();
                }
            }
            Event::Place => self.settle(),
            Event::Drop => self.typing = None,
            Event::Tool(tool) => {
                self.settle();
                self.tool = tool;
            }
            Event::Ink(ink) => self.ink = ink.min(INKS.len() - 1),
            Event::Undo => {
                if self.typing.take().is_none() {
                    self.marks.pop();
                }
            }
            Event::Copy | Event::Save | Event::Both => self.settle(),
        }
    }

    // Text being typed becomes a mark of its own, unless nothing was typed.
    fn settle(&mut self) {
        if let Some(mark) = self.typing.take()
            && mark.label.as_deref().is_some_and(|l| !l.trim().is_empty())
        {
            self.marks.push(mark);
        }
    }

    pub fn typing(&self) -> bool {
        self.typing.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty() && self.typing.is_none()
    }

    pub fn hides(&self) -> bool {
        self.marks.iter().any(|mark| mark.tool == Tool::Hide)
    }

    // The average color inside the rectangle, and nothing from outside it,
    // so the patch says no more about what's under it than its overall tone.
    fn tone(&self, rect: Rectangle) -> Rgba {
        let pixels = rgba(&self.picture);
        let (w, h) = (self.width as usize, self.height as usize);
        let left = (rect.x.max(0.0) as usize).min(w);
        let right = ((rect.x + rect.width).ceil().max(0.0) as usize).min(w);
        let top = (rect.y.max(0.0) as usize).min(h);
        let bottom = ((rect.y + rect.height).ceil().max(0.0) as usize).min(h);
        let mut sum = [0u64; 3];
        for y in top..bottom {
            for px in pixels[(y * w + left) * 4..(y * w + right) * 4]
                .as_chunks::<4>()
                .0
            {
                for (total, value) in sum.iter_mut().zip(px) {
                    *total += u64::from(*value);
                }
            }
        }
        let count = ((right - left) * (bottom - top)).max(1) as u64;
        let [r, g, b] = sum.map(|total| (total / count) as u8);
        Rgba { r, g, b, a: 1.0 }
    }

    pub fn marked(&self) -> PathBuf {
        let stem = self
            .file
            .file_stem()
            .map_or_else(|| "Screenshot".into(), |s| s.to_string_lossy().into_owned());
        self.file.with_file_name(format!("{stem} marked.png"))
    }

    // Strokes scale with the picture, so a 4K shot gets marks as visible as a 1080p one.
    fn base(&self) -> f32 {
        (self.width.max(self.height) as f32 / 1200.0).max(1.0)
    }

    fn fit(&self, room: Size) -> Rectangle {
        let scale = (room.width / self.width as f32)
            .min(room.height / self.height as f32)
            .min(1.0);
        let size = Size::new(self.width as f32 * scale, self.height as f32 * scale);
        Rectangle::new(
            Point::new(
                (room.width - size.width) / 2.0,
                (room.height - size.height) / 2.0,
            ),
            size,
        )
    }

    pub fn render(&self) -> Result<Vec<u8>, String> {
        let size =
            tiny_skia::IntSize::from_wh(self.width, self.height).ok_or("an empty picture")?;
        // tiny-skia works on premultiplied pixels; window shots have see-through corners.
        let mut pixels = rgba(&self.picture).to_vec();
        for px in pixels.as_chunks_mut::<4>().0 {
            let alpha = u16::from(px[3]);
            for channel in &mut px[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
        let mut pixmap =
            tiny_skia::Pixmap::from_vec(pixels, size).ok_or("the picture has an odd size")?;
        for shape in self.shapes() {
            paint(&mut pixmap, &shape);
        }
        pixmap.encode_png().map_err(|err| err.to_string())
    }

    // Hidden parts go under every stroke, on screen and in the file.
    fn shapes(&self) -> Vec<Shape> {
        let mut all: Vec<Shape> = self
            .marks
            .iter()
            .flat_map(|mark| shapes(mark, self.base()))
            .collect();
        all.sort_by_key(|shape| !matches!(shape, Shape::Hide(..)));
        all
    }
}

fn rgba(handle: &Handle) -> &[u8] {
    match handle {
        Handle::Rgba { pixels, .. } => pixels,
        _ => &[],
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Line(Vec<Point>, f32, Rgba),
    Dot(Point, f32, Rgba),
    Fill(Vec<Point>, Rgba),
    Frame(Rectangle, f32, Rgba),
    Hide(Rectangle, Rgba),
    Pill(Rectangle, f32, Rgba),
    Glyphs(tiny_skia::Path, Rgba),
}

// Both the screen and the saved file draw from these, so what you see is what you get.
fn shapes(mark: &Mark, base: f32) -> Vec<Shape> {
    let (Some(&first), Some(&last)) = (mark.points.first(), mark.points.last()) else {
        return Vec::new();
    };
    let span = Rectangle::new(
        Point::new(first.x.min(last.x), first.y.min(last.y)),
        Size::new((last.x - first.x).abs(), (last.y - first.y).abs()),
    );
    let small = span.width < 3.0 && span.height < 3.0;
    match mark.tool {
        Tool::Pen | Tool::Marker => {
            let (width, ink) = if mark.tool == Tool::Pen {
                (3.0 * base, mark.ink)
            } else {
                (18.0 * base, mark.ink.alpha(0.38))
            };
            if mark.points.len() == 1 {
                vec![Shape::Dot(first, width / 2.0, ink)]
            } else {
                vec![Shape::Line(mark.points.clone(), width, ink)]
            }
        }
        Tool::Arrow => {
            let (dx, dy) = (last.x - first.x, last.y - first.y);
            let length = dx.hypot(dy);
            if length < 4.0 * base {
                return Vec::new();
            }
            let (ux, uy) = (dx / length, dy / length);
            let width = 4.0 * base;
            let head = (width * 4.5).min(length * 0.6);
            let back = Point::new(last.x - ux * head, last.y - uy * head);
            let side = |by: f32| Point::new(back.x - uy * head * by, back.y + ux * head * by);
            let shaft = Point::new(last.x - ux * head * 0.5, last.y - uy * head * 0.5);
            vec![
                Shape::Line(vec![first, shaft], width, mark.ink),
                Shape::Fill(vec![last, side(0.55), side(-0.55)], mark.ink),
            ]
        }
        Tool::Box if !small => vec![Shape::Frame(span, 4.0 * base, mark.ink)],
        // Whole pixels, so no edge is half the original.
        Tool::Hide if !small => {
            let (left, top) = (span.x.floor(), span.y.floor());
            let size = Size::new(
                (span.x + span.width).ceil() - left,
                (span.y + span.height).ceil() - top,
            );
            vec![Shape::Hide(
                Rectangle::new(Point::new(left, top), size),
                mark.ink,
            )]
        }
        Tool::Box | Tool::Hide => Vec::new(),
        Tool::Text => {
            let label = mark.label.as_deref().unwrap_or_default();
            let size = 20.0 * base;
            let Some((path, width, height)) = lettering(label, size, first) else {
                return Vec::new();
            };
            let pad = 6.0 * base;
            let back = Rectangle::new(
                Point::new(first.x - pad, first.y - pad / 2.0),
                Size::new(width + pad * 2.0, height + pad),
            );
            vec![
                Shape::Pill(back, pad, Rgba::hex(0x0c0d10).alpha(0.62)),
                Shape::Glyphs(path, mark.ink),
            ]
        }
        Tool::Step => {
            let label = mark.label.as_deref().unwrap_or("1");
            let radius = 13.0 * base;
            let size = 15.0 * base;
            let mut shapes = vec![Shape::Dot(first, radius, mark.ink)];
            if let Some((_, width, height)) = lettering(label, size, first) {
                let corner = Point::new(first.x - width / 2.0, first.y - height / 2.0);
                let on = if mark.ink.contrast(neutral::ink()) >= mark.ink.contrast(neutral::text())
                {
                    neutral::ink()
                } else {
                    neutral::text()
                };
                if let Some((path, _, _)) = lettering(label, size, corner) {
                    shapes.push(Shape::Glyphs(path, on));
                }
            }
            shapes
        }
    }
}

// Text as outlines from the bundled font, so the screen and the file draw the same shapes.
// Returns the outline with its width and line height; `at` is the top left corner.
fn lettering(text: &str, size: f32, at: Point) -> Option<(tiny_skia::Path, f32, f32)> {
    use ab_glyph::{Font, ScaleFont};
    let font = ab_glyph::FontRef::try_from_slice(crate::fonts::SEMIBOLD).ok()?;
    let scaled = font.as_scaled(ab_glyph::PxScale::from(size));
    let (sx, sy) = (scaled.h_scale_factor(), scaled.v_scale_factor());
    let baseline = at.y + scaled.ascent();
    let mut path = tiny_skia::PathBuilder::new();
    let mut pen = at.x;
    let mut previous: Option<ab_glyph::GlyphId> = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(before) = previous {
            pen += scaled.kern(before, id);
        }
        if let Some(outline) = font.outline(id) {
            let point = |p: ab_glyph::Point| (pen + p.x * sx, baseline - p.y * sy);
            let mut last: Option<(f32, f32)> = None;
            for curve in outline.curves {
                let (start, end) = match curve {
                    ab_glyph::OutlineCurve::Line(a, b) => (point(a), point(b)),
                    ab_glyph::OutlineCurve::Quad(a, _, c) => (point(a), point(c)),
                    ab_glyph::OutlineCurve::Cubic(a, _, _, d) => (point(a), point(d)),
                };
                if last != Some(start) {
                    if last.is_some() {
                        path.close();
                    }
                    path.move_to(start.0, start.1);
                }
                match curve {
                    ab_glyph::OutlineCurve::Line(_, _) => path.line_to(end.0, end.1),
                    ab_glyph::OutlineCurve::Quad(_, b, _) => {
                        let b = point(b);
                        path.quad_to(b.0, b.1, end.0, end.1)
                    }
                    ab_glyph::OutlineCurve::Cubic(_, b, c, _) => {
                        let (b, c) = (point(b), point(c));
                        path.cubic_to(b.0, b.1, c.0, c.1, end.0, end.1)
                    }
                }
                last = Some(end);
            }
            if last.is_some() {
                path.close();
            }
        }
        pen += scaled.h_advance(id);
        previous = Some(id);
    }
    let width = pen - at.x;
    let height = scaled.ascent() - scaled.descent();
    Some((path.finish()?, width, height))
}

// A rectangle with round corners, for the dark backing behind text.
fn rounded(r: &Rectangle, radius: f32) -> Option<tiny_skia::Path> {
    let k = radius.min(r.width / 2.0).min(r.height / 2.0);
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.width, r.y + r.height);
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(x0 + k, y0);
    path.line_to(x1 - k, y0);
    path.quad_to(x1, y0, x1, y0 + k);
    path.line_to(x1, y1 - k);
    path.quad_to(x1, y1, x1 - k, y1);
    path.line_to(x0 + k, y1);
    path.quad_to(x0, y1, x0, y1 - k);
    path.line_to(x0, y0 + k);
    path.quad_to(x0, y0, x0 + k, y0);
    path.close();
    path.finish()
}

fn polyline(points: &[Point]) -> tiny_skia::PathBuilder {
    let mut builder = tiny_skia::PathBuilder::new();
    builder.move_to(points[0].x, points[0].y);
    for point in &points[1..] {
        builder.line_to(point.x, point.y);
    }
    builder
}

// Every shape as a filled outline. The screen fills the very outline the file does,
// so a see-through marker doesn't darken where its stroke overlaps itself.
fn outline(shape: &Shape) -> Option<(tiny_skia::Path, Rgba)> {
    let stroked = |path: tiny_skia::Path, width: f32| {
        path.stroke(
            &tiny_skia::Stroke {
                width,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..tiny_skia::Stroke::default()
            },
            1.0,
        )
    };
    let rect = |r: &Rectangle| tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height);
    match shape {
        Shape::Line(points, width, color) => {
            Some((stroked(polyline(points).finish()?, *width)?, *color))
        }
        Shape::Dot(at, radius, color) => Some((
            tiny_skia::PathBuilder::from_circle(at.x, at.y, *radius)?,
            *color,
        )),
        Shape::Fill(points, color) => {
            let mut path = polyline(points);
            path.close();
            Some((path.finish()?, *color))
        }
        Shape::Frame(r, width, color) => Some((
            stroked(tiny_skia::PathBuilder::from_rect(rect(r)?), *width)?,
            *color,
        )),
        Shape::Hide(r, color) => Some((tiny_skia::PathBuilder::from_rect(rect(r)?), *color)),
        Shape::Pill(r, radius, color) => Some((rounded(r, *radius)?, *color)),
        Shape::Glyphs(path, color) => Some((path.clone(), *color)),
    }
}

fn paint(pixmap: &mut tiny_skia::Pixmap, shape: &Shape) {
    let Some((path, color)) = outline(shape) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, (color.a * 255.0).round() as u8);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
}

struct Board<'a> {
    marks: &'a Marks,
    ink: Rgba,
}

impl canvas::Program<Message> for Board<'_> {
    type State = Option<Mark>;

    fn update(
        &self,
        drawing: &mut Option<Mark>,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let fit = self.marks.fit(bounds.size());
        let scale = fit.width / self.marks.width as f32;
        let at = |cursor: mouse::Cursor| {
            let point = cursor.position()?;
            let x = (point.x - bounds.x - fit.x).clamp(0.0, fit.width) / scale;
            let y = (point.y - bounds.y - fit.y).clamp(0.0, fit.height) / scale;
            Some(Point::new(x, y))
        };
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let inside = cursor
                    .position_in(bounds)
                    .is_some_and(|point| fit.contains(point));
                if !inside {
                    return None;
                }
                let point = at(cursor)?;
                match self.marks.tool {
                    Tool::Text => {
                        let write = Message::Mark(Event::Write(point, self.ink));
                        return Some(canvas::Action::publish(write).and_capture());
                    }
                    Tool::Step => {
                        let step = Mark {
                            tool: Tool::Step,
                            ink: self.ink,
                            points: vec![point],
                            label: None,
                        };
                        let drawn = Message::Mark(Event::Drawn(step));
                        return Some(canvas::Action::publish(drawn).and_capture());
                    }
                    _ => {}
                }
                *drawing = Some(Mark {
                    tool: self.marks.tool,
                    ink: self.ink,
                    points: vec![point],
                    label: None,
                });
                Some(canvas::Action::request_redraw().and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let mark = drawing.as_mut()?;
                let point = at(cursor)?;
                if !matches!(mark.tool, Tool::Pen | Tool::Marker) {
                    mark.points.truncate(1);
                }
                mark.points.push(point);
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let mark = drawing.take()?;
                Some(canvas::Action::publish(Message::Mark(Event::Drawn(mark))).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        drawing: &Option<Mark>,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let fit = self.marks.fit(bounds.size());
        let scale = fit.width / self.marks.width as f32;
        let on_screen = |p: &Point| Point::new(fit.x + p.x * scale, fit.y + p.y * scale);
        let on_screen = |p: tiny_skia::Point| on_screen(&Point::new(p.x, p.y));
        // What's being typed shows with a caret until it's placed.
        let typing = self.marks.typing.as_ref().map(|mark| Mark {
            label: mark.label.as_ref().map(|l| format!("{l}|")),
            ..mark.clone()
        });
        let live: Vec<Shape> = drawing
            .iter()
            .chain(typing.iter())
            .flat_map(|mark| shapes(mark, self.marks.base()))
            .collect();
        for shape in self.marks.shapes().iter().chain(&live) {
            // The hidden part being drawn shows as a frame until it's let go.
            if let Shape::Hide(rect, _) = shape
                && drawing.is_some()
                && live.contains(shape)
            {
                frame.stroke(
                    &canvas::Path::rectangle(
                        on_screen(tiny_skia::Point::from_xy(rect.x, rect.y)),
                        rect.size() * scale,
                    ),
                    Stroke::default()
                        .with_color(style::color(neutral::text().alpha(0.9)))
                        .with_width(1.5),
                );
                continue;
            }
            let Some((outline, color)) = outline(shape) else {
                continue;
            };
            let path = canvas::Path::new(|path| {
                for segment in outline.segments() {
                    match segment {
                        tiny_skia::PathSegment::MoveTo(p) => path.move_to(on_screen(p)),
                        tiny_skia::PathSegment::LineTo(p) => path.line_to(on_screen(p)),
                        tiny_skia::PathSegment::QuadTo(c, p) => {
                            path.quadratic_curve_to(on_screen(c), on_screen(p))
                        }
                        tiny_skia::PathSegment::CubicTo(a, b, p) => {
                            path.bezier_curve_to(on_screen(a), on_screen(b), on_screen(p))
                        }
                        tiny_skia::PathSegment::Close => path.close(),
                    }
                }
            });
            frame.fill(
                &path,
                canvas::Fill {
                    style: canvas::Style::Solid(style::color(color)),
                    rule: canvas::fill::Rule::NonZero,
                },
            );
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &Option<Mark>,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let fit = self.marks.fit(bounds.size());
        match cursor.position_in(bounds) {
            Some(point) if fit.contains(point) => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}

pub fn view(marks: &Marks, palette: Palette) -> Element<'_, Message> {
    let ink = INKS[marks.ink].unwrap_or(palette.accent);
    let picture = stack![
        image(marks.picture.clone())
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::ScaleDown),
        board(Board { marks, ink }).width(Fill).height(Fill),
    ];

    let tool = |(tool, glyph): (Tool, Icon)| {
        button(container(icon(glyph, 16.0, neutral::text())).center(34))
            .padding(0)
            .style(style::choice(marks.tool == tool))
            .on_press(Message::Mark(Event::Tool(tool)))
            .into()
    };
    let swatch = |(index, color): (usize, &Option<Rgba>)| {
        let color = color.unwrap_or(palette.accent);
        button(
            container(space())
                .width(14)
                .height(14)
                .style(style::pill(color)),
        )
        .padding(10)
        .style(style::choice(marks.ink == index))
        .on_press(Message::Mark(Event::Ink(index)))
        .into()
    };
    let action = |glyph: Icon, label: &'static str, event: Option<Event>| {
        button(
            row![
                icon(glyph, 14.0, neutral::text()),
                text(label).size(13).font(style::medium())
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(style::ghost(17.0))
        .on_press_maybe(event.map(Message::Mark))
    };
    let divider = || {
        container(space().width(1).height(22))
            .style(|_| container::background(style::color(neutral::hairline())))
    };
    let bar = row![
        row(TOOLS.into_iter().map(tool)).spacing(4),
        divider(),
        row(INKS.iter().enumerate().map(swatch)).spacing(2),
        divider(),
        action(
            Icon::Undo,
            "Undo",
            (!marks.marks.is_empty()).then_some(Event::Undo)
        ),
        space().width(18),
        action(Icon::Copy, "Copy", Some(Event::Copy)),
        action(Icon::Save, "Save", Some(Event::Save)),
        text("↵ both").size(12).color(style::color(style::faint())),
        action(Icon::X, "Close", None).on_press(Message::CloseMarks),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(
        column![
            picture,
            container(container(bar).padding([8, 12]).style(style::surface)).center_x(Fill),
        ]
        .spacing(18),
    )
    .width(Fill)
    .height(Fill)
    .padding([28, 36])
    .style(style::scrim)
    .into()
}

pub fn save(file: &Path, png: &[u8]) -> Result<(), String> {
    std::fs::write(file, png).map_err(|err| format!("{}: {err}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(tool: Tool, points: &[(f32, f32)]) -> Mark {
        Mark {
            tool,
            ink: Rgba::hex(0xff0000),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            label: None,
        }
    }

    #[test]
    fn words_and_numbers_become_outlines() {
        let (path, width, height) = lettering("Hi", 20.0, Point::new(10.0, 10.0)).unwrap();
        let bounds = path.bounds();
        assert!(width > 10.0 && height > 15.0);
        assert!(bounds.left() >= 10.0 && bounds.top() >= 10.0 && bounds.right() <= 10.0 + width);
        let text = Mark {
            label: Some("Click here".into()),
            ..mark(Tool::Text, &[(5.0, 5.0)])
        };
        assert!(matches!(
            shapes(&text, 1.0)[..],
            [Shape::Pill(..), Shape::Glyphs(..)]
        ));
        assert!(
            lettering("", 20.0, Point::ORIGIN).is_none(),
            "nothing typed, nothing drawn"
        );
    }

    #[test]
    fn steps_count_up_and_typing_lands_as_a_mark() {
        let (width, height) = (64u32, 48u32);
        let mut marks = Marks {
            file: PathBuf::from("/tmp/shot.png"),
            width,
            height,
            picture: Handle::from_rgba(width, height, vec![255u8; (width * height * 4) as usize]),
            marks: Vec::new(),
            tool: Tool::Step,
            ink: 0,
            typing: None,
        };
        marks.update(Event::Drawn(mark(Tool::Step, &[(10.0, 10.0)])));
        marks.update(Event::Drawn(mark(Tool::Step, &[(30.0, 10.0)])));
        let labels: Vec<_> = marks.marks.iter().map(|m| m.label.clone()).collect();
        assert_eq!(labels, [Some("1".into()), Some("2".into())]);
        marks.update(Event::Write(Point::new(5.0, 30.0), Rgba::hex(0xffffff)));
        marks.update(Event::Typed("Ok".into()));
        marks.update(Event::Typed("!".into()));
        marks.update(Event::Erase);
        assert!(marks.typing());
        marks.update(Event::Save);
        assert!(!marks.typing());
        assert_eq!(
            marks.marks.last().and_then(|m| m.label.as_deref()),
            Some("Ok")
        );
        marks.update(Event::Write(Point::new(5.0, 30.0), Rgba::hex(0xffffff)));
        marks.update(Event::Place);
        assert_eq!(marks.marks.len(), 3, "nothing typed leaves nothing");
        assert!(marks.render().is_ok());
    }

    #[test]
    fn clicks_without_a_drag_leave_only_dots() {
        assert!(shapes(&mark(Tool::Box, &[(10.0, 10.0), (11.0, 11.0)]), 1.0).is_empty());
        assert!(shapes(&mark(Tool::Arrow, &[(10.0, 10.0)]), 1.0).is_empty());
        assert!(matches!(
            shapes(&mark(Tool::Pen, &[(10.0, 10.0)]), 1.0)[..],
            [Shape::Dot(..)]
        ));
        let arrow = shapes(&mark(Tool::Arrow, &[(0.0, 0.0), (100.0, 0.0)]), 1.0);
        assert!(matches!(arrow[..], [Shape::Line(..), Shape::Fill(..)]));
    }

    #[test]
    fn marks_land_in_the_saved_picture() {
        let (width, height) = (64u32, 48u32);
        let mut pixels = vec![255u8; (width * height * 4) as usize];
        for (i, px) in pixels.chunks_mut(4).enumerate() {
            px[0] = (i % 251) as u8;
        }
        // A see-through corner, as window shots have.
        pixels[3] = 0;
        let mut marks = Marks {
            file: PathBuf::from("/tmp/shot.png"),
            width,
            height,
            picture: Handle::from_rgba(width, height, pixels.clone()),
            marks: Vec::new(),
            tool: Tool::Box,
            ink: 0,
            typing: None,
        };
        marks.update(Event::Drawn(mark(Tool::Box, &[(4.0, 4.0), (30.0, 30.0)])));
        marks.update(Event::Drawn(mark(Tool::Hide, &[(40.0, 4.0), (60.0, 40.0)])));
        let png = marks.render().unwrap();
        let saved = ::image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(saved.get_pixel(4, 15).0, [255, 0, 0, 255], "the box's edge");
        let inside = ((15 * 64 + 15) * 4) as usize;
        assert_eq!(
            saved.get_pixel(15, 15).0,
            pixels[inside..inside + 4],
            "inside is untouched"
        );
        assert_eq!(
            saved.get_pixel(0, 0).0[3],
            0,
            "the corner stays see-through"
        );
        let patch = saved.get_pixel(50, 20).0;
        assert_eq!(
            patch,
            saved.get_pixel(41, 39).0,
            "the hidden part is one flat tone"
        );
        assert_ne!(patch[0], pixels[((20 * 64 + 50) * 4) as usize]);
        assert!(marks.hides());
        assert_eq!(marks.marked(), PathBuf::from("/tmp/shot marked.png"));
    }
}

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use apolline_theme::{Palette, Rgba, neutral};
use iced::widget::canvas::{self, Frame, Geometry, LineCap, LineJoin, Stroke};
use iced::widget::image::Handle;
use iced::widget::{button, canvas as board, column, container, image, row, space, stack, text};
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
}

const TOOLS: [(Tool, Icon); 5] = [
    (Tool::Pen, Icon::Pencil),
    (Tool::Arrow, Icon::MoveUpRight),
    (Tool::Box, Icon::Square),
    (Tool::Marker, Icon::Highlighter),
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
}

#[derive(Clone, Debug)]
pub enum Event {
    Drawn(Mark),
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
    hidden: Handle,
    marks: Vec<Mark>,
    tool: Tool,
    ink: usize,
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
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let pictures = std::env::var_os("XDG_PICTURES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("Pictures"));
    std::fs::read_dir(pictures.join("Screenshots"))
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "png"))
        .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .map(|entry| entry.path())
}

impl Marks {
    fn load(file: PathBuf) -> Result<Self, String> {
        let picture = ::image::open(&file)
            .map_err(|err| format!("{}: {err}", file.display()))?
            .into_rgba8();
        let (width, height) = picture.dimensions();
        let pixels = picture.into_raw();
        let hidden = pixelate(&pixels, width, height);
        Ok(Self {
            file,
            width,
            height,
            picture: Handle::from_rgba(width, height, pixels),
            hidden: Handle::from_rgba(width, height, hidden),
            marks: Vec::new(),
            tool: Tool::Arrow,
            ink: 0,
        })
    }

    pub fn update(&mut self, event: Event) {
        match event {
            Event::Drawn(mark) => {
                if !shapes(&mark, self.base()).is_empty() {
                    self.marks.push(mark);
                }
            }
            Event::Tool(tool) => self.tool = tool,
            Event::Ink(ink) => self.ink = ink.min(INKS.len() - 1),
            Event::Undo => {
                self.marks.pop();
            }
            Event::Copy | Event::Save | Event::Both => {}
        }
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
        let mut pixmap = tiny_skia::Pixmap::from_vec(rgba(&self.picture).to_vec(), size)
            .ok_or("the picture has an odd size")?;
        let hidden = rgba(&self.hidden);
        // Hidden parts go under every stroke, as on screen.
        let mut all: Vec<Shape> = self
            .marks
            .iter()
            .flat_map(|mark| shapes(mark, self.base()))
            .collect();
        all.sort_by_key(|shape| !matches!(shape, Shape::Hide(_)));
        for shape in &all {
            paint(&mut pixmap, shape, hidden);
        }
        let picture = ::image::RgbaImage::from_raw(self.width, self.height, pixmap.take())
            .ok_or("the picture has an odd size")?;
        let mut png = Cursor::new(Vec::new());
        picture
            .write_to(&mut png, ::image::ImageFormat::Png)
            .map_err(|err| err.to_string())?;
        Ok(png.into_inner())
    }
}

fn rgba(handle: &Handle) -> &[u8] {
    match handle {
        Handle::Rgba { pixels, .. } => pixels,
        _ => &[],
    }
}

// Big blocks, so hidden text can't be read back out of them.
fn pixelate(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let block = (w.max(h) / 80).max(12);
    let mut out = vec![0u8; pixels.len()];
    for top in (0..h).step_by(block) {
        for left in (0..w).step_by(block) {
            let (right, bottom) = ((left + block).min(w), (top + block).min(h));
            let mut sum = [0u64; 4];
            for y in top..bottom {
                for x in left..right {
                    let at = (y * w + x) * 4;
                    for (total, value) in sum.iter_mut().zip(&pixels[at..at + 4]) {
                        *total += u64::from(*value);
                    }
                }
            }
            let count = ((right - left) * (bottom - top)) as u64;
            let average = sum.map(|total| (total / count) as u8);
            for y in top..bottom {
                for x in left..right {
                    let at = (y * w + x) * 4;
                    out[at..at + 4].copy_from_slice(&average);
                }
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Line(Vec<Point>, f32, Rgba),
    Dot(Point, f32, Rgba),
    Fill(Vec<Point>, Rgba),
    Frame(Rectangle, f32, Rgba),
    Hide(Rectangle),
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
        Tool::Hide if !small => vec![Shape::Hide(span)],
        Tool::Box | Tool::Hide => Vec::new(),
    }
}

fn paint(pixmap: &mut tiny_skia::Pixmap, shape: &Shape, hidden: &[u8]) {
    let ink = |color: Rgba| {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(color.r, color.g, color.b, (color.a * 255.0).round() as u8);
        paint.anti_alias = true;
        paint
    };
    let path = |points: &[Point]| {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.move_to(points[0].x, points[0].y);
        for point in &points[1..] {
            builder.line_to(point.x, point.y);
        }
        builder
    };
    let stroke = |width: f32| tiny_skia::Stroke {
        width,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..tiny_skia::Stroke::default()
    };
    let identity = tiny_skia::Transform::identity();
    match shape {
        Shape::Line(points, width, color) => {
            if let Some(line) = path(points).finish() {
                pixmap.stroke_path(&line, &ink(*color), &stroke(*width), identity, None);
            }
        }
        Shape::Dot(at, radius, color) => {
            if let Some(dot) = tiny_skia::PathBuilder::from_circle(at.x, at.y, *radius) {
                pixmap.fill_path(
                    &dot,
                    &ink(*color),
                    tiny_skia::FillRule::Winding,
                    identity,
                    None,
                );
            }
        }
        Shape::Fill(points, color) => {
            let mut outline = path(points);
            outline.close();
            if let Some(outline) = outline.finish() {
                pixmap.fill_path(
                    &outline,
                    &ink(*color),
                    tiny_skia::FillRule::Winding,
                    identity,
                    None,
                );
            }
        }
        Shape::Frame(rect, width, color) => {
            if let Some(rect) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
            {
                let outline = tiny_skia::PathBuilder::from_rect(rect);
                pixmap.stroke_path(&outline, &ink(*color), &stroke(*width), identity, None);
            }
        }
        Shape::Hide(rect) => {
            let width = pixmap.width() as usize;
            let height = pixmap.height() as usize;
            let left = (rect.x.max(0.0) as usize).min(width);
            let right = ((rect.x + rect.width).ceil().max(0.0) as usize).min(width);
            let top = (rect.y.max(0.0) as usize).min(height);
            let bottom = ((rect.y + rect.height).ceil().max(0.0) as usize).min(height);
            let data = pixmap.data_mut();
            for y in top..bottom {
                let row = (y * width + left) * 4..(y * width + right) * 4;
                data[row.clone()].copy_from_slice(&hidden[row]);
            }
        }
    }
}

struct Board<'a> {
    marks: &'a Marks,
    ink: Rgba,
}

// The blocks for hidden parts, in a layer of their own: iced draws a canvas' images above its strokes.
struct Hidden<'a>(&'a Marks);

impl canvas::Program<Message> for Hidden<'_> {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let marks = self.0;
        let mut frame = Frame::new(renderer, bounds.size());
        let fit = marks.fit(bounds.size());
        let scale = fit.width / marks.width as f32;
        for mark in &marks.marks {
            for shape in shapes(mark, marks.base()) {
                let Shape::Hide(rect) = shape else { continue };
                let region = Rectangle::new(
                    Point::new(fit.x + rect.x * scale, fit.y + rect.y * scale),
                    rect.size() * scale,
                );
                frame.with_clip(region, |inside| inside.draw_image(fit, &marks.hidden));
            }
        }
        vec![frame.into_geometry()]
    }
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
                *drawing = Some(Mark {
                    tool: self.marks.tool,
                    ink: self.ink,
                    points: vec![at(cursor)?],
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
        let base = self.marks.base();
        for mark in self.marks.marks.iter().chain(drawing.iter()) {
            for shape in shapes(mark, base) {
                match shape {
                    Shape::Line(points, width, color) => {
                        let line = canvas::Path::new(|path| {
                            path.move_to(on_screen(&points[0]));
                            for point in &points[1..] {
                                path.line_to(on_screen(point));
                            }
                        });
                        frame.stroke(
                            &line,
                            Stroke::default()
                                .with_color(style::color(color))
                                .with_width(width * scale)
                                .with_line_cap(LineCap::Round)
                                .with_line_join(LineJoin::Round),
                        );
                    }
                    Shape::Dot(at, radius, color) => frame.fill(
                        &canvas::Path::circle(on_screen(&at), radius * scale),
                        style::color(color),
                    ),
                    Shape::Fill(points, color) => {
                        let outline = canvas::Path::new(|path| {
                            path.move_to(on_screen(&points[0]));
                            for point in &points[1..] {
                                path.line_to(on_screen(point));
                            }
                            path.close();
                        });
                        frame.fill(&outline, style::color(color));
                    }
                    Shape::Frame(rect, width, color) => frame.stroke(
                        &canvas::Path::rectangle(on_screen(&rect.position()), rect.size() * scale),
                        Stroke::default()
                            .with_color(style::color(color))
                            .with_width(width * scale)
                            .with_line_join(LineJoin::Round),
                    ),
                    // Only the one being drawn; the rest are under the strokes.
                    Shape::Hide(rect) if drawing.as_ref() == Some(mark) => frame.stroke(
                        &canvas::Path::rectangle(on_screen(&rect.position()), rect.size() * scale),
                        Stroke::default()
                            .with_color(style::color(neutral::TEXT.alpha(0.9)))
                            .with_width(1.5),
                    ),
                    Shape::Hide(_) => {}
                }
            }
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
        board(Hidden(marks)).width(Fill).height(Fill),
        board(Board { marks, ink }).width(Fill).height(Fill),
    ];

    let tool = |(tool, glyph): (Tool, Icon)| {
        button(container(icon(glyph, 16.0, neutral::TEXT)).center(34))
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
                icon(glyph, 14.0, neutral::TEXT),
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
            .style(|_| container::background(style::color(neutral::HAIRLINE)))
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
        text("↵ both").size(12).color(style::color(style::FAINT)),
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
        }
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
        let hidden = pixelate(&pixels, width, height);
        let marks = Marks {
            file: PathBuf::from("/tmp/shot.png"),
            width,
            height,
            picture: Handle::from_rgba(width, height, pixels.clone()),
            hidden: Handle::from_rgba(width, height, hidden.clone()),
            marks: vec![
                mark(Tool::Box, &[(4.0, 4.0), (30.0, 30.0)]),
                mark(Tool::Hide, &[(40.0, 4.0), (60.0, 40.0)]),
            ],
            tool: Tool::Box,
            ink: 0,
        };
        let png = marks.render().unwrap();
        let saved = ::image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(saved.get_pixel(4, 15).0, [255, 0, 0, 255], "the box's edge");
        assert_eq!(
            saved.get_pixel(15, 15).0[0],
            pixels[(15 * 64 + 15) * 4],
            "inside is untouched"
        );
        let at = ((20 * 64 + 50) * 4) as usize;
        assert_eq!(
            saved.get_pixel(50, 20).0,
            hidden[at..at + 4],
            "the hidden part is blocks"
        );
        assert_eq!(marks.marked(), PathBuf::from("/tmp/shot marked.png"));
    }
}

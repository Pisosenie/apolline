use apolline_theme::{Palette, neutral};
use iced::alignment::Vertical;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Element, Fill, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::style;

// Closer than this, in logical pixels, an edge lines up with its neighbour's.
const ALIGN: i64 = 96;
const PAD: f32 = 24.0;

// A screen where niri puts it, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

impl Place {
    fn overlaps(&self, other: &Place) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub name: String,
    pub label: String,
    pub place: Place,
}

// Where a screen dropped at `at` ends up: against the nearest edge of another one, lined up with it
// when close, and never on top of one.
pub fn snap(at: Place, others: &[Place]) -> Option<(i64, i64)> {
    let mut best: Option<(i64, (i64, i64))> = None;
    for other in others {
        let along_y = slide(at.y, at.height, other.y, other.height);
        let along_x = slide(at.x, at.width, other.x, other.width);
        let sides = [
            (other.x - at.width, along_y),
            (other.x + other.width, along_y),
            (along_x, other.y - at.height),
            (along_x, other.y + other.height),
        ];
        for (x, y) in sides {
            let moved = Place { x, y, ..at };
            if others.iter().any(|o| moved.overlaps(o)) {
                continue;
            }
            let distance = (x - at.x).abs() + (y - at.y).abs();
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, (x, y)));
            }
        }
    }
    best.map(|(_, at)| at)
}

// Along an edge it touches: kept where it is, but always sharing some of the edge, and lined up
// with the neighbour's start, end or middle when close to one.
fn slide(at: i64, length: i64, other: i64, other_length: i64) -> i64 {
    let touching = at.clamp(other - length + 1, other + other_length - 1);
    [
        other,
        other + other_length - length,
        other + (other_length - length) / 2,
    ]
    .into_iter()
    .filter(|lined| (lined - touching).abs() <= ALIGN)
    .min_by_key(|lined| (lined - touching).abs())
    .unwrap_or(touching)
}

pub fn view<'a, M: 'a>(
    screens: Vec<Screen>,
    palette: Palette,
    place: impl Fn(String, i64, i64) -> M + 'a,
) -> Element<'a, M> {
    canvas::Canvas::new(Arrange {
        screens,
        palette,
        place: Box::new(place),
    })
    .width(Fill)
    .height(220)
    .into()
}

struct Arrange<'a, M> {
    screens: Vec<Screen>,
    palette: Palette,
    place: Box<dyn Fn(String, i64, i64) -> M + 'a>,
}

#[derive(Default)]
struct Drag {
    index: usize,
    grab: Point,
    from: Point,
    at: Point,
}

// Logical pixels to the canvas and back, fitting every screen with some room around.
struct Fit {
    scale: f32,
    origin: Point,
    left: i64,
    top: i64,
}

impl Fit {
    fn new(screens: &[Screen], bounds: Size) -> Fit {
        let left = screens.iter().map(|s| s.place.x).min().unwrap_or(0);
        let top = screens.iter().map(|s| s.place.y).min().unwrap_or(0);
        let right = screens
            .iter()
            .map(|s| s.place.x + s.place.width)
            .max()
            .unwrap_or(1);
        let bottom = screens
            .iter()
            .map(|s| s.place.y + s.place.height)
            .max()
            .unwrap_or(1);
        let (wide, high) = ((right - left).max(1) as f32, (bottom - top).max(1) as f32);
        let scale = ((bounds.width - 2.0 * PAD) / wide).min((bounds.height - 2.0 * PAD) / high);
        Fit {
            scale,
            origin: Point::new(
                (bounds.width - wide * scale) / 2.0,
                (bounds.height - high * scale) / 2.0,
            ),
            left,
            top,
        }
    }

    fn rect(&self, place: &Place) -> Rectangle {
        Rectangle {
            x: self.origin.x + (place.x - self.left) as f32 * self.scale,
            y: self.origin.y + (place.y - self.top) as f32 * self.scale,
            width: place.width as f32 * self.scale,
            height: place.height as f32 * self.scale,
        }
    }

    fn logical(&self, point: Point) -> (i64, i64) {
        (
            ((point.x - self.origin.x) / self.scale).round() as i64 + self.left,
            ((point.y - self.origin.y) / self.scale).round() as i64 + self.top,
        )
    }
}

impl<M> canvas::Program<M> for Arrange<'_, M> {
    type State = Option<Drag>;

    fn update(
        &self,
        drag: &mut Option<Drag>,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        let fit = Fit::new(&self.screens, bounds.size());
        let point = cursor.position_in(bounds);
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = point?;
                // One screen has nowhere to go.
                if self.screens.len() < 2 {
                    return None;
                }
                let index = self
                    .screens
                    .iter()
                    .position(|s| fit.rect(&s.place).contains(point))?;
                let rect = fit.rect(&self.screens[index].place);
                *drag = Some(Drag {
                    index,
                    grab: Point::new(point.x - rect.x, point.y - rect.y),
                    from: Point::new(rect.x, rect.y),
                    at: Point::new(rect.x, rect.y),
                });
                Some(canvas::Action::request_redraw().and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let moving = drag.as_mut()?;
                let point = point?;
                moving.at = Point::new(point.x - moving.grab.x, point.y - moving.grab.y);
                Some(canvas::Action::request_redraw().and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let moved = drag.take()?;
                // A click, not a move: nothing to settle.
                if moved.at.distance(moved.from) < 4.0 {
                    return Some(canvas::Action::request_redraw());
                }
                let screen = self.screens.get(moved.index)?;
                let (x, y) = fit.logical(moved.at);
                let others: Vec<Place> = self
                    .screens
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != moved.index)
                    .map(|(_, s)| s.place)
                    .collect();
                let (x, y) = snap(
                    Place {
                        x,
                        y,
                        ..screen.place
                    },
                    &others,
                )?;
                if (x, y) == (screen.place.x, screen.place.y) {
                    return Some(canvas::Action::request_redraw());
                }
                Some(canvas::Action::publish((self.place)(screen.name.clone(), x, y)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        drag: &Option<Drag>,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let fit = Fit::new(&self.screens, bounds.size());
        for (index, screen) in self.screens.iter().enumerate() {
            let mut rect = fit.rect(&screen.place);
            let dragged = drag.as_ref().filter(|d| d.index == index);
            if let Some(dragged) = dragged {
                rect.x = dragged.at.x;
                rect.y = dragged.at.y;
            }
            let path = Path::rounded_rectangle(rect.position(), rect.size(), 8.0.into());
            let tint = if dragged.is_some() {
                self.palette.accent_soft
            } else {
                style::fill_color()
            };
            frame.fill(&path, style::color(tint));
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(style::color(if dragged.is_some() {
                        self.palette.accent
                    } else {
                        neutral::text().alpha(0.25)
                    }))
                    .with_width(1.5),
            );
            for (line, (words, size, tint)) in [
                (&screen.name, 13.0, neutral::text()),
                (&screen.label, 11.0, style::faint()),
            ]
            .into_iter()
            .enumerate()
            {
                frame.fill_text(canvas::Text {
                    content: words.clone(),
                    position: Point::new(
                        rect.center_x(),
                        rect.center_y() + 16.0 * line as f32 - 8.0,
                    ),
                    color: style::color(tint),
                    size: size.into(),
                    align_x: Alignment::Center,
                    align_y: Vertical::Center,
                    max_width: rect.width - 8.0,
                    ..canvas::Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        drag: &Option<Drag>,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if drag.is_some() {
            return mouse::Interaction::Grabbing;
        }
        let fit = Fit::new(&self.screens, bounds.size());
        match cursor.position_in(bounds) {
            Some(point)
                if self.screens.len() > 1
                    && self
                        .screens
                        .iter()
                        .any(|s| fit.rect(&s.place).contains(point)) =>
            {
                mouse::Interaction::Grab
            }
            _ => mouse::Interaction::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Place = Place {
        x: 0,
        y: 0,
        width: 1600,
        height: 1000,
    };

    fn monitor(x: i64, y: i64) -> Place {
        Place {
            x,
            y,
            width: 2560,
            height: 1440,
        }
    }

    #[test]
    fn a_dropped_screen_meets_its_neighbour() {
        assert_eq!(
            snap(monitor(1700, -400), &[LAPTOP]),
            Some((1600, -440)),
            "to the right, bottoms lined up"
        );
        assert_eq!(
            snap(monitor(-2500, 300), &[LAPTOP]),
            Some((-2560, 300)),
            "to the left, kept where it was dropped along the edge"
        );
        assert_eq!(
            snap(monitor(-480, -1500), &[LAPTOP]),
            Some((-480, -1440)),
            "above, centred"
        );
        assert_eq!(
            snap(monitor(5000, 3000), &[LAPTOP]),
            Some((1600, 999)),
            "far away still ends up touching"
        );
        assert_eq!(snap(monitor(0, 0), &[]), None, "alone, nowhere to go");
    }

    #[test]
    fn it_never_lands_on_another_screen() {
        let others = [LAPTOP, monitor(1600, 0)];
        let (x, y) = snap(monitor(800, 200), &others).unwrap();
        let landed = Place {
            x,
            y,
            ..monitor(0, 0)
        };
        assert!(others.iter().all(|o| !landed.overlaps(o)), "{landed:?}");
    }
}

// niri starts a screen at its preferred mode, which is often 60 Hz on a 144 or 165 Hz panel.
// For each screen the config leaves alone: the fastest mode at the size it runs at, if faster.
pub fn fastest_modes(
    outputs: &[apolline_niri::Output],
    displays: &[apolline_config::Display],
) -> Vec<(String, String)> {
    outputs
        .iter()
        .filter(|output| {
            !displays
                .iter()
                .any(|d| d.name == output.name && d.mode.is_some())
        })
        .filter_map(|output| {
            let now = output.current_mode.and_then(|i| output.modes.get(i))?;
            // A mode other than the screen's own was picked by someone, in niri's config say.
            if !now.is_preferred {
                return None;
            }
            let best = output
                .modes
                .iter()
                .filter(|m| m.width == now.width && m.height == now.height)
                .max_by_key(|m| m.refresh_rate)?;
            (best.refresh_rate > now.refresh_rate + 1_000).then(|| {
                (
                    output.name.clone(),
                    format!(
                        "{}x{}@{:.3}",
                        best.width,
                        best.height,
                        f64::from(best.refresh_rate) / 1000.0
                    ),
                )
            })
        })
        .collect()
}

// A mode written by hand, "2560x1440@165" say, as the screen has it exactly: the closest refresh
// at that size, or the fastest when none was given. None when the screen isn't here.
pub fn exact_mode(outputs: &[apolline_niri::Output], name: &str, wanted: &str) -> Option<String> {
    let (size, rate) = wanted
        .split_once('@')
        .map_or((wanted, None), |(size, rate)| {
            (size, rate.parse::<f64>().ok())
        });
    let (width, height) = size.split_once('x')?;
    let (width, height): (u32, u32) = (width.parse().ok()?, height.parse().ok()?);
    let output = outputs.iter().find(|output| output.name == name)?;
    let sized = output
        .modes
        .iter()
        .filter(|m| m.width == width && m.height == height);
    let best = match rate {
        Some(rate) => {
            sized.min_by_key(|m| (f64::from(m.refresh_rate) - rate * 1000.0).abs() as u64)
        }
        None => sized.max_by_key(|m| m.refresh_rate),
    }?;
    Some(format!(
        "{}x{}@{:.3}",
        best.width,
        best.height,
        f64::from(best.refresh_rate) / 1000.0
    ))
}

#[cfg(test)]
mod fastest {
    use super::*;

    #[test]
    fn a_fast_panel_leaves_60_hz_and_a_set_mode_stays() {
        let outputs = apolline_niri::outputs(
            r#"{"DP-4":{"name":"DP-4","modes":[{"width":2560,"height":1440,"refresh_rate":60000,"is_preferred":true},{"width":2560,"height":1440,"refresh_rate":164999},{"width":1920,"height":1080,"refresh_rate":240000}],"current_mode":0},"HDMI-A-2":{"name":"HDMI-A-2","modes":[{"width":1920,"height":1080,"refresh_rate":60000},{"width":1920,"height":1080,"refresh_rate":100000}],"current_mode":0},"eDP-1":{"name":"eDP-1","modes":[{"width":1920,"height":1080,"refresh_rate":60000}],"current_mode":0}}"#,
        );
        let set = apolline_config::Display {
            name: "HDMI-A-2".into(),
            mode: Some("1920x1080@60.000".into()),
            scale: None,
            transform: None,
            position: None,
            vrr: None,
        };
        assert_eq!(
            fastest_modes(&outputs, &[set]),
            [("DP-4".to_owned(), "2560x1440@164.999".to_owned())]
        );
    }
}

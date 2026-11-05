// The dock's measures: where each icon sits at rest, how the ones near the pointer grow, and the
// surface that holds it all. Lengths run along the screen edge, thicknesses away from it.
use std::f32::consts::PI;

use apolline_config::Edge;
use apolline_theme::metrics;
use iced::Point;

// The icon under the pointer grows by this much of its size, its neighbours less, out to REACH
// icons away.
pub const GROW: f32 = 0.6;
const REACH: f32 = 2.5;
// Inside the glass, around the icons.
pub const PAD: f32 = 8.0;
// Between an icon and the glass's edge by the screen, for the window dots.
pub const DOTS: f32 = 8.0;
// From the screen edge to the glass.
pub const GAP: f32 = metrics::BAR_INSET;
// Past the grown icons, for the name that shows on hover: above them, or beside a dock down a
// side, where a long name needs the width.
const LABEL: f32 = 40.0;
const LABEL_BESIDE: f32 = 180.0;
// The line between pinned and running apps.
pub const LINE: f32 = 1.0;
// What a hidden dock feels the pointer through, at the screen edge.
const STRIP: f32 = 2.0;

// How much of the growth an icon gets `distance` icons from the pointer: all of it right under
// it, none from REACH on, a cosine between so the bump has no corners.
pub fn falloff(distance: f32) -> f32 {
    if distance >= REACH {
        return 0.0;
    }
    (1.0 + (PI * distance / REACH).cos()) / 2.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub size: f32,
    pub count: usize,
    // The first running app that isn't pinned, when there are pinned ones before it.
    pub split: Option<usize>,
    // Down a side of the screen.
    pub sideways: bool,
}

impl Shape {
    pub fn spacing(self) -> f32 {
        (self.size * 0.16).round()
    }

    fn step(self) -> f32 {
        self.size + self.spacing()
    }

    // Where each icon starts at rest, from the start of the row.
    pub fn starts(self) -> Vec<f32> {
        let line = LINE + self.spacing();
        (0..self.count)
            .map(|i| {
                let past_line = self.split.is_some_and(|split| i >= split);
                i as f32 * self.step() + if past_line { line } else { 0.0 }
            })
            .collect()
    }

    pub fn length(self) -> f32 {
        self.starts().last().map_or(0.0, |last| last + self.size)
    }

    // Each icon's size with the pointer `pointer` along the row, grown as far as `strength`.
    pub fn sizes(self, pointer: Option<f32>, strength: f32) -> Vec<f32> {
        let step = self.step();
        self.starts()
            .into_iter()
            .map(|start| match pointer {
                Some(at) => {
                    let distance = (at - start - self.size / 2.0).abs() / step;
                    self.size * (1.0 + GROW * strength * falloff(distance))
                }
                None => self.size,
            })
            .collect()
    }

    // Where the grown row starts against the resting one, so the spot under the pointer stays
    // under it and icons don't slide away from what's being aimed at.
    pub fn shift(self, pointer: f32, sizes: &[f32]) -> f32 {
        let mut grown = 0.0;
        let mut rest_end = 0.0;
        for (start, size) in self.starts().into_iter().zip(sizes) {
            grown += start - rest_end;
            if pointer < start {
                return start - grown;
            }
            if pointer <= start + self.size {
                return pointer - (grown + (pointer - start) / self.size * size);
            }
            grown += size;
            rest_end = start + self.size;
        }
        pointer - (grown + (pointer - rest_end))
    }

    // The icon a point along the row is on, its share of the spacing around it included.
    pub fn at(self, along: f32) -> Option<usize> {
        let half = self.spacing() / 2.0;
        self.starts()
            .into_iter()
            .position(|start| (start - half..start + self.size + half).contains(&along))
    }

    // As far as the grown row can stray from where it rests, either way.
    pub fn margin(self) -> f32 {
        (GROW * REACH * self.size).ceil()
    }

    // The glass's thickness, with its icons at `size`.
    pub fn glass(self, size: f32) -> f32 {
        PAD + size + DOTS + PAD
    }

    // The room windows keep clear of a dock that doesn't hide.
    pub fn zone(self) -> f32 {
        GAP + self.glass(self.size)
    }

    // The surface: the row with room to grow at both ends and away from the edge.
    pub fn surface(self) -> (f32, f32) {
        let along = self.length() + 2.0 * (PAD + self.margin());
        let label = if self.sideways { LABEL_BESIDE } else { LABEL };
        let across = GAP + self.glass(self.size * (1.0 + GROW)) + label;
        (along.ceil(), across.ceil())
    }

    // Where the resting row starts along a surface `along` long, with the row in its middle.
    pub fn row_start(self, along: f32) -> f32 {
        (along - self.length()) / 2.0
    }
}

// What of the surface takes the pointer: along the glass and out from the screen edge to it, all
// of the room to grow while the pointer is on it, and a strip at the edge while it's hidden.
// The gap to the edge counts, so a pointer pushed against the edge still finds the dock.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn input(shape: Shape, surface: (f32, f32), shown: bool, hovered: bool) -> (f32, f32, f32) {
    let start = shape.row_start(surface.0) - PAD;
    let length = shape.length() + 2.0 * PAD;
    match (shown, hovered) {
        (false, _) => (start, length, STRIP),
        (true, false) => (start, length, GAP + shape.glass(shape.size)),
        (true, true) => (0.0, surface.0, GAP + shape.glass(shape.size * (1.0 + GROW))),
    }
}

// A stretch `along` the edge reaching `across` from it, as a rectangle on the surface: x, y,
// width, height.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn rect(
    edge: Edge,
    surface: (f32, f32),
    (start, length, across): (f32, f32, f32),
) -> (i32, i32, i32, i32) {
    let round = |n: f32| n.round() as i32;
    let (start, length, across, thick) =
        (round(start), round(length), round(across), round(surface.1));
    match edge {
        Edge::Top => (start, 0, length, across),
        Edge::Bottom => (start, thick - across, length, across),
        Edge::Left => (0, start, across, length),
        Edge::Right => (thick - across, start, across, length),
    }
}

// A point on a dock's surface `across` thick as how far along the edge it is and how far from
// the screen edge.
pub fn local(edge: Edge, across: f32, at: Point) -> (f32, f32) {
    match edge {
        Edge::Top => (at.x, at.y),
        Edge::Bottom => (at.x, across - at.y),
        Edge::Left => (at.y, at.x),
        Edge::Right => (at.y, across - at.x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCK: Shape = Shape {
        size: 48.0,
        count: 5,
        split: Some(3),
        sideways: false,
    };

    #[test]
    fn the_growth_falls_off_smoothly_to_nothing() {
        assert_eq!(falloff(0.0), 1.0);
        assert!((falloff(REACH / 2.0) - 0.5).abs() < 1e-6, "halfway is half");
        assert_eq!(falloff(REACH), 0.0);
        assert_eq!(falloff(10.0), 0.0);
        let steps: Vec<f32> = (0..=25).map(|i| falloff(i as f32 / 10.0)).collect();
        assert!(steps.windows(2).all(|w| w[1] <= w[0]), "never grows back");
        // No corner at the far end: the last step is far gentler than the middle one.
        let end = falloff(REACH - 0.1) - falloff(REACH);
        let middle = falloff(REACH / 2.0 - 0.05) - falloff(REACH / 2.0 + 0.05);
        assert!(end < middle / 10.0);
    }

    #[test]
    fn the_icon_under_the_pointer_grows_most() {
        let at_second = DOCK.starts()[1] + 24.0;
        let sizes = DOCK.sizes(Some(at_second), 1.0);
        assert!((sizes[1] - 48.0 * (1.0 + GROW)).abs() < 1e-3);
        assert!(sizes[0] < sizes[1] && sizes[0] > 48.0);
        assert!((sizes[0] - sizes[2]).abs() < 1e-3, "even either side");
        assert!(sizes[3] < sizes[2], "the line puts the next one further");
        assert_eq!(DOCK.sizes(None, 1.0), vec![48.0; 5]);
        assert_eq!(DOCK.sizes(Some(at_second), 0.0), vec![48.0; 5], "at rest");
        let half = DOCK.sizes(Some(at_second), 0.5);
        assert!((half[1] - 48.0 * (1.0 + GROW / 2.0)).abs() < 1e-3);
    }

    #[test]
    fn the_spot_under_the_pointer_stays_put() {
        for pointer in [5.0, 60.0, 130.0, 200.0, 275.0] {
            let sizes = DOCK.sizes(Some(pointer), 1.0);
            let shift = DOCK.shift(pointer, &sizes);
            let index = DOCK.at(pointer).unwrap();
            let starts = DOCK.starts();
            // The grown icon's start, as the view lays them out from `shift`.
            let mut start = shift;
            for i in 0..index {
                start += sizes[i] + (starts[i + 1] - starts[i] - 48.0);
            }
            let inside = (pointer - starts[index]).clamp(0.0, 48.0) / 48.0;
            let grown_spot = start + inside * sizes[index];
            assert!(
                (grown_spot - pointer).abs() < 2.0,
                "{pointer}: {grown_spot}"
            );
            assert!(shift.abs() <= DOCK.margin(), "stays inside the surface");
        }
        assert_eq!(
            DOCK.shift(100.0, &[48.0; 5]),
            0.0,
            "nothing grown, nothing moves"
        );
    }

    #[test]
    fn the_line_takes_its_room_and_points_find_their_icon() {
        let starts = DOCK.starts();
        assert_eq!(starts[1] - starts[0], 56.0);
        assert_eq!(starts[3] - starts[2], 56.0 + LINE + 8.0);
        assert_eq!(DOCK.length(), starts[4] + 48.0);
        assert_eq!(DOCK.at(10.0), Some(0));
        assert_eq!(DOCK.at(starts[3] + 1.0), Some(3));
        assert_eq!(DOCK.at(-50.0), None);
        let empty = Shape {
            count: 0,
            split: None,
            ..DOCK
        };
        assert_eq!(empty.length(), 0.0);
    }

    #[test]
    fn the_pointer_is_felt_where_the_dock_is() {
        let surface = DOCK.surface();
        let (start, length, across) = input(DOCK, surface, true, false);
        assert_eq!(start + length / 2.0, surface.0 / 2.0, "in the middle");
        assert_eq!(across, DOCK.zone());
        assert_eq!(input(DOCK, surface, false, false).2, STRIP);
        let grown = input(DOCK, surface, true, true);
        assert_eq!((grown.0, grown.1), (0.0, surface.0));
        assert!(
            grown.2 > across && grown.2 < surface.1,
            "not the names' room"
        );
        let (x, y, w, h) = rect(Edge::Bottom, (300.0, 150.0), (10.0, 200.0, 80.0));
        assert_eq!((x, y, w, h), (10, 70, 200, 80));
        assert_eq!(
            rect(Edge::Right, (300.0, 150.0), (10.0, 200.0, 80.0)),
            (70, 10, 80, 200)
        );
        assert_eq!(
            rect(Edge::Left, (300.0, 150.0), (10.0, 200.0, 80.0)),
            (0, 10, 80, 200)
        );
    }

    #[test]
    fn the_surface_holds_the_grown_icons() {
        let (along, across) = DOCK.surface();
        assert!(along >= DOCK.length() + 2.0 * DOCK.margin());
        assert!(across > DOCK.zone() + 48.0 * GROW);
        assert_eq!(DOCK.zone(), GAP + 8.0 + 48.0 + DOTS + 8.0);
    }
}

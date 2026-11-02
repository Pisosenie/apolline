use apolline_theme::{Palette, neutral};
use iced::widget::{container, mouse_area, pin, responsive, space, stack, text};
use iced::{Background, Border, Element, Fill, Point, Rectangle, Shrink, Vector, mouse};

use crate::drag::drag;
use crate::style;

// Things arranged by dragging: a row for each zone, like the bar's left, center and right, and
// under them what's left off.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub zones: Vec<Vec<String>>,
    pub spare: Vec<String>,
}

const CHIP: f32 = 28.0;
const GAP: f32 = 6.0;
const PAD: f32 = 8.0;
// The zone's name, at the start of its row.
const NAME: f32 = 72.0;
const TEXT: f32 = 12.0;

// A zone's row, or (zone None) the space for what's left off.
#[derive(Clone, Debug, PartialEq)]
struct Lane {
    zone: Option<usize>,
    rect: Rectangle,
}

#[derive(Clone, Debug, PartialEq)]
struct Chip {
    name: String,
    zone: Option<usize>,
    rect: Rectangle,
}

fn chip_width(label: &str) -> f32 {
    label.chars().count() as f32 * TEXT * 0.6 + 26.0
}

// Chips flow along their lane and wrap onto the next line when it's full.
fn flow(
    names: &[String],
    zone: Option<usize>,
    label: fn(&str) -> String,
    left: f32,
    top: f32,
    right: f32,
) -> (Vec<Chip>, f32) {
    let (mut x, mut y) = (left, top);
    let mut chips = Vec::new();
    for name in names {
        let w = chip_width(&label(name));
        if x > left && x + w > right {
            (x, y) = (left, y + CHIP + GAP);
        }
        chips.push(Chip {
            name: name.clone(),
            zone,
            rect: Rectangle {
                x,
                y,
                width: w,
                height: CHIP,
            },
        });
        x += w + GAP;
    }
    (chips, y + CHIP)
}

fn layout(board: &Board, label: fn(&str) -> String, width: f32) -> (Vec<Lane>, Vec<Chip>, f32) {
    let mut lanes = Vec::new();
    let mut chips = Vec::new();
    let mut top = 0.0;
    let lanes_of = board
        .zones
        .iter()
        .map(Some)
        .enumerate()
        .map(|(z, names)| (Some(z), names));
    for (zone, names) in lanes_of.chain(std::iter::once((None, Some(&board.spare)))) {
        let names = names.map_or(&[][..], Vec::as_slice);
        let left = if zone.is_some() { NAME } else { PAD };
        let (placed, bottom) = flow(names, zone, label, left, top + PAD, width - PAD);
        let height = (bottom - top + PAD).max(CHIP + 2.0 * PAD);
        lanes.push(Lane {
            zone,
            rect: Rectangle {
                x: 0.0,
                y: top,
                width,
                height,
            },
        });
        chips.extend(placed);
        top += height + GAP;
    }
    (lanes, chips, top - GAP)
}

fn chips(board: &Board, label: fn(&str) -> String, width: f32) -> Vec<Chip> {
    layout(board, label, width).1
}

pub fn height(board: &Board, label: fn(&str) -> String, width: f32) -> f32 {
    // Room for one more line, so a chip dropped into a full lane doesn't make it jump.
    layout(board, label, width).2 + CHIP + GAP
}

// The board once `name` is let go at `at`: in a zone's lane it lands between the chips either side,
// anywhere else it's left off.
pub fn dropped(
    board: &Board,
    name: &str,
    at: Point,
    label: fn(&str) -> String,
    width: f32,
) -> Board {
    let without = |names: &Vec<String>| names.iter().filter(|n| *n != name).cloned().collect();
    let mut next = Board {
        zones: board.zones.iter().map(without).collect(),
        spare: without(&board.spare),
    };
    let (lanes, chips, _) = layout(&next, label, width);
    let lane = lanes
        .iter()
        .find(|lane| at.y >= lane.rect.y && at.y < lane.rect.y + lane.rect.height + GAP);
    match lane.and_then(|lane| lane.zone) {
        Some(z) => {
            let index = chips
                .iter()
                .filter(|chip| chip.zone == Some(z))
                .filter(|chip| {
                    chip.rect.y + CHIP < at.y || chip.rect.y <= at.y && chip.rect.center_x() < at.x
                })
                .count();
            next.zones[z].insert(index, name.to_owned());
        }
        None => next.spare.push(name.to_owned()),
    }
    next
}

// A chip being dragged: what it is, where it was taken hold of, and where the pointer is.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub name: String,
    pub grab: Vector,
    pub at: Point,
}

// Drawn with plain widgets; where things are is worked out here, so a press, a move and a drop
// each come back already resolved: what was taken, and the board it makes.
#[allow(clippy::too_many_arguments)]
pub fn view<'a, M: Clone + 'a>(
    board: Board,
    names: &'static [&'static str],
    label: fn(&str) -> String,
    palette: Palette,
    held: Option<Held>,
    on_hold: fn(Option<Held>) -> M,
    on_move: fn(Point) -> M,
    on_drop: fn(Option<Board>) -> M,
) -> Element<'a, M> {
    responsive(move |size| {
        let width = size.width;
        // While a chip is held, the others make room where it would land.
        let shown = match &held {
            Some(holding) => dropped(&board, &holding.name, holding.at, label, width),
            None => board.clone(),
        };
        let (lanes, placed, _) = layout(&shown, label, width);
        let tall = height(&board, label, width).max(height(&shown, label, width));
        let mut layers: Vec<Element<'a, M>> = Vec::new();
        for lane in &lanes {
            let name = match lane.zone {
                Some(z) => names.get(z).copied().unwrap_or_default(),
                None if shown.spare.is_empty() => "Drag something here to take it off the bar",
                None => "",
            };
            let filled = lane.zone.is_some();
            layers.push(
                pin(
                    container(text(name).size(11).color(style::color(style::faint())))
                        .padding([0.0, PAD + if filled { 6.0 } else { 0.0 }])
                        .width(lane.rect.width)
                        .height(PAD * 2.0 + CHIP)
                        .align_y(iced::Alignment::Center),
                )
                .position(lane.rect.position())
                .into(),
            );
            if filled {
                layers.insert(
                    0,
                    pin(container(space())
                        .width(lane.rect.width)
                        .height(lane.rect.height)
                        .style(|_| container::Style {
                            background: Some(Background::Color(style::color(style::fill_color()))),
                            border: Border::default().rounded(12),
                            ..container::Style::default()
                        }))
                    .position(lane.rect.position())
                    .into(),
                );
            }
        }
        for chip in &placed {
            let lifted = held.as_ref().is_some_and(|h| h.name == chip.name);
            layers.push(
                pin(if lifted {
                    spot(chip.rect.width, palette)
                } else {
                    piece(
                        &label(&chip.name),
                        chip.rect.width,
                        chip.zone.is_some(),
                        None,
                    )
                })
                .position(chip.rect.position())
                .into(),
            );
        }
        if let Some(holding) = &held {
            let w = chip_width(&label(&holding.name));
            layers.push(
                pin(piece(&label(&holding.name), w, true, Some(palette)))
                    .position(holding.at - holding.grab)
                    .into(),
            );
        }
        let before = chips(&board, label, width);
        let taken = held.clone();
        let original = board.clone();
        drag(
            stack(layers).width(Fill).height(tall),
            move |at| {
                on_hold(
                    before
                        .iter()
                        .find(|chip| chip.rect.contains(at))
                        .map(|chip| Held {
                            name: chip.name.clone(),
                            grab: at - chip.rect.position(),
                            at,
                        }),
                )
            },
            on_move,
            move |at| {
                on_drop(
                    taken
                        .as_ref()
                        .map(|holding| dropped(&original, &holding.name, at, label, width))
                        .filter(|next| *next != original),
                )
            },
        )
        .into()
    })
    .height(Shrink)
    .into()
}

// A chip: on the bar it's solid, left off it's an outline, held it wears the accent.
fn piece<'a, M: Clone + 'a>(
    name: &str,
    width: f32,
    on: bool,
    held: Option<Palette>,
) -> Element<'a, M> {
    let ink = if on || held.is_some() {
        neutral::text()
    } else {
        neutral::muted()
    };
    let chip = container(text(name.to_owned()).size(TEXT).color(style::color(ink)))
        .center_x(width)
        .center_y(CHIP)
        .style(move |_| {
            let (fill, edge) = match held {
                Some(palette) => (palette.accent_soft, palette.accent),
                None if on => (neutral::text().alpha(0.1), neutral::text().alpha(0.18)),
                None => (neutral::text().alpha(0.0), neutral::text().alpha(0.18)),
            };
            container::Style {
                background: Some(Background::Color(style::color(fill))),
                border: Border {
                    color: style::color(edge),
                    width: 1.0,
                    radius: 9.0.into(),
                },
                ..container::Style::default()
            }
        });
    mouse_area(chip)
        .interaction(mouse::Interaction::Grab)
        .into()
}

// Where the held chip would land.
fn spot<'a, M: 'a>(width: f32, palette: Palette) -> Element<'a, M> {
    container(space())
        .width(width)
        .height(CHIP)
        .style(move |_| container::Style {
            border: Border {
                color: style::color(palette.accent.alpha(0.6)),
                width: 1.0,
                radius: 9.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(name: &str) -> String {
        name.to_owned()
    }

    fn board(zones: &[&[&str]], spare: &[&str]) -> Board {
        let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect();
        Board {
            zones: zones.iter().map(|z| names(z)).collect(),
            spare: names(spare),
        }
    }

    const WIDTH: f32 = 600.0;

    // The middle of the lane for `zone` (None: what's left off), at `x`.
    fn spot(board: &Board, zone: Option<usize>, x: f32) -> Point {
        let (lanes, _, _) = layout(board, label, WIDTH);
        let lane = lanes.iter().find(|lane| lane.zone == zone).unwrap();
        Point::new(x, lane.rect.y + lane.rect.height / 2.0)
    }

    #[test]
    fn a_chip_moves_to_the_zone_it_is_dropped_on() {
        let start = board(&[&["mode", "clock"], &["tray"], &[]], &["media"]);
        let at = spot(&start, Some(2), 300.0);
        let moved = dropped(&start, "clock", at, label, WIDTH);
        assert_eq!(
            moved,
            board(&[&["mode"], &["tray"], &["clock"]], &["media"])
        );
    }

    #[test]
    fn it_lands_between_the_chips_either_side() {
        let start = board(&[&["a", "b", "c"], &[], &[]], &[]);
        // Lifted, `a` leaves a gap the others close, so `b` is first while it's held.
        let rest = board(&[&["b", "c"], &[], &[]], &[]);
        let after_b = chips(&rest, label, WIDTH)[0].rect.center_x() + 1.0;
        let moved = dropped(&start, "a", spot(&start, Some(0), after_b), label, WIDTH);
        assert_eq!(moved.zones[0], ["b", "a", "c"]);
    }

    #[test]
    fn taken_off_it_waits_below_and_goes_back_from_there() {
        let start = board(&[&["mode"], &["clock"], &["tray"]], &["media"]);
        let off = dropped(&start, "clock", spot(&start, None, 300.0), label, WIDTH);
        assert_eq!(
            off,
            board(&[&["mode"], &[], &["tray"]], &["media", "clock"])
        );
        let back = dropped(&off, "media", spot(&off, Some(0), 0.0), label, WIDTH);
        assert_eq!(
            back,
            board(&[&["media", "mode"], &[], &["tray"]], &["clock"])
        );
    }

    #[test]
    fn a_full_lane_wraps_and_grows() {
        let many: Vec<String> = (0..12).map(|i| format!("module{i}")).collect();
        let one = board(&[&["a"]], &[]);
        let full = Board {
            zones: vec![many],
            spare: Vec::new(),
        };
        let (lanes, chips, _) = layout(&full, label, 300.0);
        assert!(
            chips.iter().any(|c| c.rect.y > chips[0].rect.y),
            "a second line"
        );
        assert!(height(&full, label, 300.0) > height(&one, label, 300.0));
        assert!(lanes[0].rect.height > CHIP + 2.0 * PAD);
    }
}

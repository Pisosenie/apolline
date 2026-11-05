// The machine at a glance, opened from the bar's numbers: load, memory and the network over the
// last minute and a half, the disk, and the processes doing the most, your own ones endable.

use std::collections::VecDeque;

use apolline_theme::{Palette, Rgba, neutral};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{
    Column, button, canvas as graph, column, container, mouse_area, opaque, progress_bar, row,
    space,
};
use iced::{
    Alignment, Background, Border, Element, Fill, Length, Point, Rectangle, Renderer, Theme,
};

use crate::icon::{Icon, icon};
use crate::machine::{self, Machine, Process, bytes};
use crate::shell::Message;
use crate::style::{self, text, verbatim};

const WIDTH: f32 = 420.0;
const LEFT: f32 = 132.0;

pub struct Look<'a> {
    pub machine: Option<&'a Machine>,
    pub by_memory: bool,
    pub ending: Option<u32>,
    pub palette: Palette,
    pub edge: apolline_config::Edge,
    pub gap: f32,
}

pub fn view(look: Look<'_>) -> Element<'_, Message> {
    let body: Element<'_, Message> = match look.machine {
        Some(machine) => column![
            graphs(machine, look.palette),
            disk(machine, look.palette),
            processes(machine, look.by_memory, look.ending),
        ]
        .spacing(18)
        .into(),
        None => text("Reading…")
            .size(13)
            .color(style::color(style::faint()))
            .into(),
    };
    let card = container(body)
        .width(Length::Fixed(WIDTH))
        .padding(18)
        .style(style::surface);
    mouse_area(style::beside_bar(
        opaque(card),
        look.edge,
        Alignment::End,
        look.gap,
        0.0,
    ))
    .on_press(Message::Overlay(None))
    .into()
}

fn graphs(machine: &Machine, palette: Palette) -> Element<'_, Message> {
    let last = |line: &VecDeque<f32>| line.back().copied().unwrap_or_default();
    let mut cpu_note = crate::i18n::count(machine.cores, "1 core", "{n} cores");
    if let Some(degrees) = machine.temperature {
        cpu_note = format!("{cpu_note} · {degrees}°");
    }
    let mut memory_note =
        crate::i18n::fill("of {total}", &[("total", &bytes(machine.total as f32))]);
    if machine.swap.0 > 0 {
        let swap = crate::i18n::fill("swap {used}", &[("used", &bytes(machine.swap.0 as f32))]);
        memory_note = format!("{memory_note} · {swap}");
    }
    let rate = |n: f32| format!("{}/s", bytes(n));
    let busiest_net = machine
        .down
        .iter()
        .chain(&machine.up)
        .fold(64_000.0_f32, |a, b| a.max(*b));
    column![
        figure(
            Icon::Cpu,
            "Processor",
            format!("{:.0}%", last(&machine.cpu)),
            cpu_note,
            vec![Line::new(&machine.cpu, 100.0, palette.accent)],
        ),
        figure(
            Icon::MemoryStick,
            "Memory",
            bytes(machine.used as f32),
            memory_note,
            vec![Line::new(&machine.memory, 100.0, palette.accent)],
        ),
        figure(
            Icon::Globe,
            "Network",
            format!("↓ {}", rate(last(&machine.down))),
            format!("↑ {}", rate(last(&machine.up))),
            vec![
                Line::new(&machine.down, busiest_net, palette.accent),
                Line::new(&machine.up, busiest_net, neutral::muted()),
            ],
        ),
    ]
    .spacing(14)
    .into()
}

fn figure<'a>(
    glyph: Icon,
    name: &'static str,
    value: String,
    note: String,
    lines: Vec<Line<'a>>,
) -> Element<'a, Message> {
    row![
        column![
            row![
                icon(glyph, 13.0, neutral::muted()),
                text(name).size(12).color(style::color(neutral::muted())),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            text(value).size(18).font(style::mono()),
            text(note).size(11).color(style::color(style::faint())),
        ]
        .spacing(3)
        .width(Length::Fixed(LEFT)),
        graph(Graph(lines)).width(Fill).height(52),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn disk(machine: &Machine, palette: Palette) -> Element<'_, Message> {
    let Some((used, total)) = machine.disk else {
        return space().into();
    };
    let share = used as f32 / total.max(1) as f32 * 100.0;
    column![
        row![
            icon(Icon::Folder, 13.0, neutral::muted()),
            text("Disk").size(12).color(style::color(neutral::muted())),
            space().width(Fill),
            text(crate::i18n::fill(
                "{used} of {total}",
                &[
                    ("used", &bytes(used as f32)),
                    ("total", &bytes(total as f32))
                ],
            ))
            .size(12)
            .font(style::mono()),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        progress_bar(0.0..=100.0, share)
            .girth(4)
            .style(move |_| progress_bar::Style {
                background: Background::Color(style::color(neutral::hairline())),
                bar: Background::Color(style::color(if share > 90.0 {
                    style::WARN
                } else {
                    palette.accent
                })),
                border: Border::default().rounded(2),
            }),
    ]
    .spacing(8)
    .into()
}

fn processes(machine: &Machine, by_memory: bool, ending: Option<u32>) -> Element<'_, Message> {
    let sort = |label: &'static str, memory: bool| {
        button(text(label).size(11))
            .padding([4, 10])
            .style(style::choice(by_memory == memory))
            .on_press(Message::MachineSort(memory))
    };
    let listed = if by_memory {
        &machine.biggest
    } else {
        &machine.busiest
    };
    let rows = listed
        .iter()
        .map(|process| process_row(process, by_memory, ending == Some(process.pid)));
    column![
        row![
            text("Processes")
                .size(12)
                .color(style::color(neutral::muted())),
            space().width(Fill),
            sort("Processor", false),
            sort("Memory", true),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        Column::with_children(rows).spacing(2),
    ]
    .spacing(8)
    .into()
}

fn process_row(process: &Process, by_memory: bool, armed: bool) -> Element<'_, Message> {
    let figure = if by_memory {
        bytes(process.memory as f32)
    } else {
        format!("{:.0}%", process.cpu)
    };
    // Ending takes two clicks: the first turns the × into a red "End".
    let end: Element<'_, Message> = if !process.yours {
        space().width(26).into()
    } else if armed {
        button(text("End").size(11))
            .padding([3, 10])
            .style(style::danger(true))
            .on_press(Message::EndProcess(process.pid))
            .into()
    } else {
        button(icon(Icon::X, 12.0, style::faint()))
            .padding(5)
            .style(style::ghost(6.0))
            .on_press(Message::EndProcess(process.pid))
            .into()
    };
    row![
        verbatim(process.name.as_str()).size(13).width(Fill),
        text(figure)
            .size(12)
            .font(style::mono())
            .color(style::color(neutral::muted())),
        end,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .height(28)
    .into()
}

struct Line<'a> {
    values: &'a VecDeque<f32>,
    top: f32,
    color: Rgba,
}

impl<'a> Line<'a> {
    fn new(values: &'a VecDeque<f32>, top: f32, color: Rgba) -> Self {
        Self { values, top, color }
    }
}

// Newest at the right edge, filling in from there as the readings come.
struct Graph<'a>(Vec<Line<'a>>);

impl canvas::Program<Message> for Graph<'_> {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (width, height) = (bounds.width, bounds.height);
        let step = width / (machine::KEPT - 1) as f32;
        frame.stroke(
            &Path::line(
                Point::new(0.0, height - 0.5),
                Point::new(width, height - 0.5),
            ),
            Stroke::default()
                .with_width(1.0)
                .with_color(style::color(neutral::hairline())),
        );
        for line in &self.0 {
            let count = line.values.len();
            if count < 2 {
                continue;
            }
            let points: Vec<Point> = line
                .values
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    let share = (value / line.top.max(f32::EPSILON)).clamp(0.0, 1.0);
                    Point::new(
                        width - (count - 1 - i) as f32 * step,
                        2.0 + (height - 4.0) * (1.0 - share),
                    )
                })
                .collect();
            let curve = Path::new(|path| {
                path.move_to(points[0]);
                points[1..].iter().for_each(|p| path.line_to(*p));
            });
            let area = Path::new(|path| {
                path.move_to(Point::new(points[0].x, height));
                points.iter().for_each(|p| path.line_to(*p));
                path.line_to(Point::new(width, height));
                path.close();
            });
            frame.fill(&area, style::color(line.color.alpha(0.14)));
            frame.stroke(
                &curve,
                Stroke::default()
                    .with_width(1.5)
                    .with_color(style::color(line.color)),
            );
        }
        vec![frame.into_geometry()]
    }
}

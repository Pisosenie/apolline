use super::*;
use iced::widget::column;

pub(super) fn group<'a>(
    title: impl AsRef<str>,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut body = Column::new();
    let count = rows.len();
    for (index, row) in rows.into_iter().enumerate() {
        body = body.push(row);
        if index + 1 < count {
            body = body.push(
                container(Space::new().height(1))
                    .width(Fill)
                    .style(|_| container::background(style::color(neutral::hairline()))),
            );
        }
    }
    column![
        text(title.as_ref().to_uppercase())
            .size(11)
            .font(style::semibold())
            .color(style::color(style::faint())),
        container(body).width(Fill).style(style::card),
    ]
    .spacing(10)
    .into()
}

pub(super) fn padded<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).padding([14, 16]).width(Fill).into()
}

pub(super) fn setting<'a>(
    title: &'a str,
    about: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    setting_owned(
        title,
        (!about.is_empty()).then(|| about.to_owned()),
        control,
    )
}

pub(super) fn setting_owned<'a>(
    title: &'a str,
    about: Option<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut words = column![text(title).size(14).font(style::medium())].spacing(3);
    if let Some(about) = about {
        words = words.push(text(about).size(12).color(style::color(style::faint())));
    }
    row![words.width(Fill), control.into()]
        .spacing(20)
        .align_y(Alignment::Center)
        .padding([14, 16])
        .into()
}

pub(super) fn setting_with_icon<'a>(
    glyph: Icon,
    title: &'a str,
    about: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        container(icon(glyph, 15.0, neutral::text()))
            .center(30)
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(style::fill_color()))),
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        container(setting(title, about, control)).width(Fill),
    ]
    .align_y(Alignment::Center)
    .padding(iced::Padding::new(0.0).left(16.0))
    .into()
}

pub(super) fn fact<'a>(glyph: Icon, line: &'a str) -> Element<'a, Message> {
    row![
        icon(glyph, 15.0, style::soft_text()),
        text(line).size(13).color(style::color(style::soft_text()))
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

pub(super) fn note<'a>(line: &str) -> Element<'a, Message> {
    row![
        icon(Icon::Info, 14.0, style::faint()),
        text(line.to_owned())
            .size(12)
            .color(style::color(style::faint()))
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

pub(super) fn warning<'a>(line: &str) -> Element<'a, Message> {
    row![
        icon(Icon::Warning, 14.0, style::WARN),
        text(line.to_owned())
            .size(12)
            .color(style::color(style::WARN))
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([12, 16])
    .into()
}

pub(super) fn segmented<'a, T: Copy + PartialEq + 'a, const N: usize>(
    options: [(T, &'a str, Option<Icon>); N],
    current: impl Into<Option<T>>,
    on: impl Fn(T) -> Message,
) -> Element<'a, Message> {
    let current = current.into();
    let buttons = Row::with_children(options.map(|(value, label, glyph)| {
        let on_now = Some(value) == current;
        let mut content = Row::new().spacing(7).align_y(Alignment::Center);
        if let Some(glyph) = glyph {
            content = content.push(icon(
                glyph,
                14.0,
                if on_now {
                    neutral::text()
                } else {
                    style::faint()
                },
            ));
        }
        content = content.push(text(label).size(13));
        button(content)
            .padding([6, 14])
            .style(style::choice(on_now))
            .on_press(on(value))
            .into()
    }))
    .spacing(2);
    container(buttons).padding(3).style(style::segments).into()
}

pub(super) fn switch<'a>(
    on: bool,
    message: impl Fn(bool) -> Message + 'a,
    palette: Palette,
) -> Element<'a, Message> {
    toggler(on)
        .on_toggle(message)
        .size(22)
        .style(style::switch(palette))
        .into()
}

pub(super) fn value_slider<'a>(
    range: std::ops::RangeInclusive<u16>,
    value: u16,
    message: impl Fn(u16) -> Message + 'a,
    unit: &'a str,
    palette: Palette,
) -> Element<'a, Message> {
    row![
        slider(range, value, message)
            .on_release(Message::Commit)
            .width(240)
            .style(style::rail(palette, 4.0)),
        text(format!("{value} {unit}"))
            .size(12)
            .font(style::mono())
            .color(style::color(style::soft_text()))
            .width(Length::Fixed(64.0))
            .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn minutes<'a>(
    value: u16,
    message: fn(u16) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    let label = match value {
        0 => "never".to_owned(),
        minutes => format!("{minutes} min"),
    };
    row![
        slider(0..=60u16, value, message)
            .on_release(Message::Commit)
            .width(240)
            .style(style::rail(palette, 4.0)),
        text(label)
            .size(12)
            .font(style::mono())
            .color(style::color(style::soft_text()))
            .width(Length::Fixed(64.0))
            .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn speed_slider<'a>(
    value: f32,
    message: fn(f32) -> Message,
    palette: Palette,
) -> Element<'a, Message> {
    row![
        text("Slow").size(12).color(style::color(style::faint())),
        slider(-1.0..=1.0, value, message)
            .step(0.05_f32)
            .on_release(Message::Commit)
            .width(200)
            .style(style::rail(palette, 4.0)),
        text("Fast").size(12).color(style::color(style::faint())),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn highlight<'a>(code: &str, palette: Palette) -> Element<'a, Message> {
    let faint = style::color(style::faint());
    let lines = code.lines().map(|line| {
        let mut spans: Vec<Span<'static, ()>> = Vec::new();
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            spans.push(span(line.to_owned()).color(faint));
        } else if trimmed.starts_with('[') {
            spans.push(span(line.to_owned()).color(style::color(palette.accent)));
        } else if let Some((key, rest)) = line.split_once('=') {
            let (value, comment) = split_comment(rest);
            // Strings in green, numbers and the rest in amber, darker on a light scheme.
            let light = apolline_theme::scheme().light;
            let tint = match (value.trim_start().starts_with(['"', '[']), light) {
                (true, false) => Rgba::hex(0xa8d8a0),
                (true, true) => Rgba::hex(0x2f7d3c),
                (false, false) => style::WARN,
                (false, true) => Rgba::hex(0x9a5a00),
            };
            spans.push(span(key.to_owned()).color(style::color(neutral::text())));
            spans.push(span("=").color(faint));
            spans.push(span(value.to_owned()).color(style::color(tint)));
            if !comment.is_empty() {
                spans.push(span(comment.to_owned()).color(faint));
            }
        } else {
            spans.push(span(line.to_owned()));
        }
        rich_text(spans)
            .size(12)
            .font(style::mono())
            .line_height(1.5)
            .into()
    });
    Column::with_children(lines).into()
}

pub(super) fn split_comment(rest: &str) -> (&str, &str) {
    let mut quoted = false;
    for (index, c) in rest.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return rest.split_at(index),
            _ => {}
        }
    }
    (rest, "")
}

pub(super) fn readable(palette: Palette) -> bool {
    let panel = neutral::panel().over(Rgba::hex(0x808080));
    palette.on_accent.contrast(palette.accent) >= 4.5 && palette.accent.contrast(panel) >= 4.5
}

pub(super) fn broken(err: &str) -> Element<'_, Message> {
    column![
        text("Your config needs a fix first")
            .size(26)
            .font(style::semibold()),
        text("Settings never rewrites a file it can't read, so nothing is lost.")
            .size(14)
            .color(style::color(style::faint())),
        container(
            text(err)
                .font(style::mono())
                .size(13)
                .color(style::color(style::WARN))
        )
        .padding(16)
        .width(Fill)
        .style(style::card),
        text("Fix it in the file — this window picks it up as soon as you save.").size(13),
    ]
    .spacing(16)
    .into()
}

// 87 % as 0.87, the way the file keeps fractions.
pub(super) fn hundredths(percent: u16) -> f64 {
    f64::from(percent) / 100.0
}

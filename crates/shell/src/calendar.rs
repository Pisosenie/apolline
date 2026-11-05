use crate::style::text;
use apolline_theme::{Palette, neutral};
use chrono::{DateTime, Datelike, Local, Months, NaiveDate};
use iced::widget::{Row, button, column, container, mouse_area, opaque, row, space};
use iced::{Alignment, Element, Fill, Length};

use crate::events::Agenda;
use crate::icon::{Icon, icon};
use crate::media::shorten;
use crate::shell::Message;
use crate::style;
use crate::weather::{self, Weather};

const CELL: f32 = 38.0;

pub fn month(now: NaiveDate, offset: i32) -> NaiveDate {
    let first = now.with_day(1).unwrap_or(now);
    if offset >= 0 {
        first.checked_add_months(Months::new(offset as u32))
    } else {
        first.checked_sub_months(Months::new(offset.unsigned_abs()))
    }
    .unwrap_or(first)
}

pub fn days(first: NaiveDate) -> Vec<NaiveDate> {
    let lead = first.weekday().num_days_from_monday();
    let start = first - chrono::Duration::days(i64::from(lead));
    (0..42).map(|i| start + chrono::Duration::days(i)).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    now: DateTime<Local>,
    offset: i32,
    picked: Option<NaiveDate>,
    agenda: &'a Agenda,
    weather: Option<&'a Weather>,
    palette: Palette,
    edge: apolline_config::Edge,
    gap: f32,
    calendars: usize,
) -> Element<'a, Message> {
    let today = now.date_naive();
    let first = month(today, offset);
    let arrow = |glyph: Icon, by: i32| {
        button(icon(glyph, 15.0, neutral::text()))
            .padding(7)
            .style(style::ghost(8.0))
            .on_press(Message::CalendarMonth(offset + by))
    };
    let mut title = row![
        text(
            first
                .format(&crate::i18n::names("%B %Y", &first))
                .to_string()
        )
        .size(16)
        .font(style::semibold())
        .width(Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(2);
    if offset != 0 {
        title = title.push(
            button(text("Today").size(12))
                .padding([5, 10])
                .style(style::ghost(8.0))
                .on_press(Message::CalendarMonth(0)),
        );
    }
    title = title
        .push(arrow(Icon::ChevronLeft, -1))
        .push(arrow(Icon::ChevronRight, 1));

    let names = Row::with_children(["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].map(|name| {
        container(text(name).size(11).color(style::color(style::faint())))
            .center_x(CELL)
            .into()
    }));
    let mut grid = column![names].spacing(2);
    for week in days(first).chunks(7) {
        grid = grid.push(Row::with_children(week.iter().map(|day| {
            let inside = day.month() == first.month();
            let is_today = *day == today;
            let label = text(day.day().to_string()).size(13);
            let label = if is_today {
                label
                    .font(style::semibold())
                    .color(style::color(palette.on_accent))
            } else if inside {
                label.color(style::color(neutral::text()))
            } else {
                label.color(style::color(neutral::text().alpha(0.3)))
            };
            let busy = agenda.busy(*day);
            let dot = container(space().width(4).height(4)).style(style::pill(match busy {
                true if is_today => palette.on_accent,
                true => palette.accent,
                false => neutral::text().alpha(0.0),
            }));
            let cell = container(column![label, dot].spacing(1).align_x(Alignment::Center))
                .center(CELL - 4.0);
            let cell = if is_today {
                cell.style(style::pill(palette.accent))
            } else if Some(*day) == picked {
                cell.style(style::pill(palette.accent_soft))
            } else {
                cell
            };
            button(cell)
                .padding(2)
                .style(style::ghost((CELL - 4.0) / 2.0))
                .on_press(Message::CalendarDay(*day))
                .into()
        })));
    }

    let mut top = column![
        text(now.format(&crate::i18n::names("%A", &now)).to_string())
            .size(12)
            .color(style::color(palette.accent)),
        text(now.format(&crate::i18n::names("%-d %B", &now)).to_string())
            .size(22)
            .font(style::semibold()),
    ]
    .spacing(2);
    if let Some(weather) = weather {
        top = top.push(forecast(weather, palette)).spacing(12);
    }
    let mut body = column![
        top,
        container(space())
            .width(Fill)
            .height(1)
            .style(|_| { iced::widget::container::background(style::color(neutral::hairline())) }),
        title,
        grid,
    ];
    if picked.is_some() || agenda.busy(today) {
        body = body.push(day_list(picked.unwrap_or(today), today, agenda, palette));
    }
    // With nothing set up yet, say what it can show and where that's set.
    let invites: Vec<Element<'a, Message>> = [
        (weather.is_none(), Icon::CloudSun, "Add weather"),
        (calendars == 0, Icon::Plus, "Add calendar"),
    ]
    .into_iter()
    .filter(|(missing, ..)| *missing)
    .map(|(_, glyph, label)| {
        button(
            row![
                icon(glyph, 14.0, neutral::muted()),
                text(label).size(12).color(style::color(neutral::muted())),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([6, 10])
        .style(style::ghost(8.0))
        .on_press(Message::SettingsPage("bar"))
        .into()
    })
    .collect();
    if !invites.is_empty() {
        body = body.push(Row::with_children(invites).spacing(4));
    }
    let card = container(body.spacing(14).width(Length::Fixed(CELL * 7.0)))
        .padding(18)
        .style(style::surface);

    mouse_area(style::beside_bar(
        opaque(card),
        edge,
        Alignment::Center,
        gap,
        0.0,
    ))
    .on_press(Message::Overlay(None))
    .into()
}

const LISTED: usize = 6;

// What's on a day: whole-day things first, then by time.
fn day_list<'a>(
    day: NaiveDate,
    today: NaiveDate,
    agenda: &'a Agenda,
    palette: Palette,
) -> Element<'a, Message> {
    let heading = if day == today {
        "Today".to_owned()
    } else {
        day.format(&crate::i18n::names("%A, %-d %B", &day))
            .to_string()
    };
    let mut list = column![
        text(
            crate::i18n::find(&heading)
                .unwrap_or(&heading)
                .to_uppercase()
        )
        .size(10)
        .font(style::semibold())
        .color(style::color(style::faint()))
    ]
    .spacing(8);
    let items = agenda.on(day);
    if items.is_empty() {
        return list
            .push(
                text("Nothing planned")
                    .size(12)
                    .color(style::color(style::faint())),
            )
            .into();
    }
    for item in items.iter().take(LISTED) {
        let when = match (item.start, item.end) {
            (Some(start), Some(end)) => {
                format!("{} – {}", start.format("%H:%M"), end.format("%H:%M"))
            }
            (Some(start), None) => start.format("%H:%M").to_string(),
            (None, Some(end)) => {
                crate::i18n::fill("Until {time}", &[("time", &end.format("%H:%M"))])
            }
            (None, None) => "All day".to_owned(),
        };
        let mut what = column![
            text(when).size(11).color(style::color(style::faint())),
            text(shorten(&item.summary, 34)).size(13),
        ]
        .spacing(1);
        if let Some(place) = &item.location {
            what = what.push(
                text(shorten(place, 36))
                    .size(11)
                    .color(style::color(style::faint())),
            );
        }
        let mut line = row![
            container(space().width(3).height(30)).style(style::pill(palette.accent)),
            what.width(Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if let Some(link) = &item.call {
            line = line.push(
                button(text("Join").size(12).font(style::medium()))
                    .padding([5, 12])
                    .style(style::primary(palette))
                    .on_press(Message::OpenLink(link.to_string())),
            );
        }
        list = list.push(line);
    }
    if items.len() > LISTED {
        list = list.push(
            text(crate::i18n::fill(
                "+{count} more",
                &[("count", &(items.len() - LISTED))],
            ))
            .size(11)
            .color(style::color(style::faint())),
        );
    }
    list.into()
}

fn forecast(weather: &Weather, palette: Palette) -> Element<'_, Message> {
    let (glyph, words) = weather::describe(weather.code);
    let today = row![
        icon(glyph, 24.0, palette.accent),
        text(format!("{:.0}°", weather.now))
            .size(22)
            .font(style::semibold()),
        column![
            text(words).size(13),
            row![
                icon(Icon::MapPin, 11.0, style::faint()),
                text(weather.place.as_str())
                    .size(11)
                    .color(style::color(style::faint())),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let days = Row::with_children(weather.days.iter().skip(1).take(3).map(|day| {
        let (glyph, _) = weather::describe(day.code);
        column![
            text(
                day.date
                    .format(&crate::i18n::names("%a", &day.date))
                    .to_string()
            )
            .size(11)
            .color(style::color(style::faint())),
            icon(glyph, 16.0, neutral::text()),
            text(format!("{:.0}° {:.0}°", day.high, day.low)).size(11),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .width(Fill)
        .into()
    }));
    column![today, days].spacing(12).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_month_page_starts_on_monday_and_has_six_weeks() {
        let october = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let page = days(month(october, 0));
        assert_eq!(page.len(), 42);
        assert_eq!(page[0], NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
        assert_eq!(page[3], NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        assert_eq!(
            month(october, 3),
            NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()
        );
        assert_eq!(
            month(october, -10),
            NaiveDate::from_ymd_opt(2025, 12, 1).unwrap()
        );
    }
}

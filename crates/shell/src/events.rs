use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{
    DateTime, Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday,
};
use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::shell::Message;

const EVERY: Duration = Duration::from_secs(10 * 60);
// How far from today occurrences are worked out, both ways.
const REACH: i64 = 400;
const MOST: usize = 5000;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occurrence {
    // None for a whole day.
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
    pub summary: String,
    pub location: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agenda {
    days: BTreeMap<NaiveDate, Vec<Occurrence>>,
}

impl Agenda {
    pub fn on(&self, day: NaiveDate) -> &[Occurrence] {
        self.days.get(&day).map_or(&[], Vec::as_slice)
    }

    pub fn busy(&self, day: NaiveDate) -> bool {
        self.days.contains_key(&day)
    }

    fn add(&mut self, day: NaiveDate, occurrence: Occurrence) {
        let list = self.days.entry(day).or_default();
        if !list.contains(&occurrence) {
            list.push(occurrence);
            list.sort();
        }
    }
}

pub fn subscription(sources: &[String]) -> Subscription<Message> {
    if sources.is_empty() {
        return Subscription::none();
    }
    Subscription::run_with(sources.to_vec(), watch)
}

#[expect(
    clippy::ptr_arg,
    reason = "Subscription::run_with hands the data over as &D"
)]
fn watch(sources: &Vec<String>) -> impl Stream<Item = Message> + use<> {
    let sources = sources.clone();
    iced::stream::channel(1, async move |mut output| {
        loop {
            let wanted = sources.clone();
            let agenda = tokio::task::spawn_blocking(move || {
                let texts: Vec<String> = wanted
                    .iter()
                    .filter_map(|source| {
                        read(source)
                            .map_err(|err| eprintln!("apolline: calendar {source}: {err}"))
                            .ok()
                    })
                    .collect();
                agenda(&texts, Local::now().date_naive())
            })
            .await
            .unwrap_or_default();
            if output
                .send(Message::Agenda(Arc::new(agenda)))
                .await
                .is_err()
            {
                return;
            }
            tokio::time::sleep(EVERY).await;
        }
    })
}

// A file, or a subscription link (webcal:// is https://). No accounts, no sign-in.
fn read(source: &str) -> Result<String, String> {
    let link = source
        .strip_prefix("webcal://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| source.to_owned());
    if link.starts_with("https://") {
        return ureq::get(&link)
            .call()
            .map_err(|err| err.to_string())?
            .body_mut()
            .with_config()
            .limit(8 * 1024 * 1024)
            .read_to_string()
            .map_err(|err| err.to_string());
    }
    if link.starts_with("http://") {
        return Err("only https links are read".into());
    }
    let path = match link.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| std::path::PathBuf::from(home).join(rest))
            .ok_or("no home folder")?,
        None => std::path::PathBuf::from(&link),
    };
    std::fs::read_to_string(&path).map_err(|err| err.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Zone {
    Floating,
    Utc,
    Named(chrono_tz::Tz),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Moment {
    at: NaiveDateTime,
    zone: Zone,
    whole_day: bool,
}

impl Moment {
    fn local(&self) -> NaiveDateTime {
        let local = match self.zone {
            Zone::Floating => return self.at,
            Zone::Utc => Utc.from_utc_datetime(&self.at).with_timezone(&Local),
            Zone::Named(tz) => match tz.from_local_datetime(&self.at).earliest() {
                Some(at) => at.with_timezone(&Local),
                None => return self.at,
            },
        };
        DateTime::naive_local(&local)
    }
}

#[derive(Clone, Debug, Default)]
struct Rule {
    freq: String,
    interval: u32,
    count: Option<usize>,
    until: Option<NaiveDate>,
    by_day: Vec<(i32, Weekday)>,
    by_month_day: Vec<i32>,
}

#[derive(Debug, Default)]
struct Event {
    uid: String,
    summary: String,
    location: Option<String>,
    start: Option<Moment>,
    end: Option<Moment>,
    rule: Option<Rule>,
    except: Vec<NaiveDate>,
    instead_of: Option<NaiveDate>,
    cancelled: bool,
}

pub fn agenda(texts: &[String], today: NaiveDate) -> Agenda {
    let mut events: Vec<Event> = texts.iter().flat_map(|text| parse(text)).collect();
    // A moved or cancelled meeting replaces that one day of its series.
    let moved: Vec<(String, NaiveDate)> = events
        .iter()
        .filter_map(|e| Some((e.uid.clone(), e.instead_of?)))
        .collect();
    for event in events.iter_mut().filter(|e| e.instead_of.is_none()) {
        event.except.extend(
            moved
                .iter()
                .filter(|(uid, _)| *uid == event.uid)
                .map(|(_, day)| *day),
        );
    }
    let from = today - chrono::Duration::days(REACH);
    let until = today + chrono::Duration::days(REACH);
    let mut agenda = Agenda::default();
    for event in events.iter().filter(|e| !e.cancelled) {
        let Some(start) = event.start else { continue };
        let length = event
            .end
            .map(|end| end.at - start.at)
            .filter(|length| *length > chrono::Duration::zero())
            .unwrap_or_else(|| {
                if start.whole_day {
                    chrono::Duration::days(1)
                } else {
                    chrono::Duration::zero()
                }
            });
        let days = match &event.rule {
            Some(rule) => expand(start.at.date(), rule, until),
            None => vec![start.at.date()],
        };
        for day in days.into_iter().filter(|d| !event.except.contains(d)) {
            let first = Moment {
                at: day.and_time(start.at.time()),
                ..start
            };
            if start.whole_day {
                let span = length.num_days().clamp(1, 60);
                for offset in 0..span {
                    let date = day + chrono::Duration::days(offset);
                    if (from..=until).contains(&date) {
                        agenda.add(date, occurrence(event, None, None));
                    }
                }
            } else {
                let begins = first.local();
                let ends = Moment {
                    at: first.at + length,
                    ..first
                }
                .local();
                if (from..=until).contains(&begins.date()) {
                    let end = (length > chrono::Duration::zero()).then(|| ends.time());
                    agenda.add(begins.date(), occurrence(event, Some(begins.time()), end));
                }
            }
        }
    }
    agenda
}

fn occurrence(event: &Event, start: Option<NaiveTime>, end: Option<NaiveTime>) -> Occurrence {
    Occurrence {
        start,
        end,
        summary: event.summary.clone(),
        location: event.location.clone(),
    }
}

// Lines folded onto the next one start with a space or a tab.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        match line.strip_prefix([' ', '\t']) {
            Some(rest) if !lines.is_empty() => {
                if let Some(last) = lines.last_mut() {
                    last.push_str(rest.trim_end_matches('\r'));
                }
            }
            _ => lines.push(line.trim_end_matches('\r').to_owned()),
        }
    }
    lines
}

// NAME;PARAM=VALUE;...:VALUE, with quoted parameter values allowed to hold colons.
type Params = Vec<(String, String)>;

fn property(line: &str) -> Option<(String, Params, &str)> {
    let mut quoted = false;
    let colon = line.char_indices().find_map(|(i, c)| match c {
        '"' => {
            quoted = !quoted;
            None
        }
        ':' if !quoted => Some(i),
        _ => None,
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|part| part.split_once('='))
        .map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_owned()))
        .collect();
    Some((name, params, value))
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.trim().to_owned()
}

fn moment(params: &[(String, String)], value: &str) -> Option<Moment> {
    let value = value.trim();
    let param = |key: &str| {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    if param("VALUE") == Some("DATE") || value.len() == 8 {
        let day = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some(Moment {
            at: day.and_time(NaiveTime::MIN),
            zone: Zone::Floating,
            whole_day: true,
        });
    }
    let (stamp, utc) = match value.strip_suffix('Z') {
        Some(stamp) => (stamp, true),
        None => (value, false),
    };
    let at = NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S").ok()?;
    let zone = if utc {
        Zone::Utc
    } else {
        param("TZID")
            .and_then(|name| name.trim_start_matches('/').parse().ok())
            .map_or(Zone::Floating, Zone::Named)
    };
    Some(Moment {
        at,
        zone,
        whole_day: false,
    })
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

fn rule(value: &str) -> Option<Rule> {
    let mut rule = Rule {
        interval: 1,
        ..Rule::default()
    };
    for part in value.split(';') {
        let (key, value) = part.split_once('=')?;
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => rule.freq = value.to_ascii_uppercase(),
            "INTERVAL" => rule.interval = value.parse().ok().filter(|i| *i > 0)?,
            "COUNT" => rule.count = value.parse().ok(),
            "UNTIL" => rule.until = NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d").ok(),
            "BYDAY" => {
                rule.by_day = value
                    .split(',')
                    .filter_map(|day| {
                        let split = day.len().checked_sub(2)?;
                        let (nth, code) = day.split_at(split);
                        Some((nth.parse().unwrap_or(0), weekday(code)?))
                    })
                    .collect();
            }
            "BYMONTHDAY" => {
                rule.by_month_day = value.split(',').filter_map(|d| d.parse().ok()).collect()
            }
            _ => {}
        }
    }
    matches!(
        rule.freq.as_str(),
        "DAILY" | "WEEKLY" | "MONTHLY" | "YEARLY"
    )
    .then_some(rule)
}

// The days a rule lands on, from the first one up to `limit`.
fn expand(start: NaiveDate, rule: &Rule, limit: NaiveDate) -> Vec<NaiveDate> {
    let end = rule.until.map_or(limit, |until| until.min(limit));
    let step = i64::from(rule.interval);
    let mut days = Vec::new();
    let mut counted = 0usize;
    let mut take = |day: NaiveDate, days: &mut Vec<NaiveDate>| -> bool {
        if day < start {
            return true;
        }
        if day > end || rule.count.is_some_and(|count| counted >= count) {
            return false;
        }
        counted += 1;
        days.push(day);
        days.len() < MOST
    };
    match rule.freq.as_str() {
        "DAILY" => {
            let mut day = start;
            while take(day, &mut days) {
                day += chrono::Duration::days(step);
            }
        }
        "WEEKLY" => {
            let mut week = start.week(Weekday::Mon).first_day();
            let mut wanted: Vec<Weekday> = rule.by_day.iter().map(|(_, d)| *d).collect();
            if wanted.is_empty() {
                wanted.push(start.weekday());
            }
            wanted.sort_by_key(Weekday::num_days_from_monday);
            'weeks: loop {
                for day in &wanted {
                    let date = week + chrono::Duration::days(i64::from(day.num_days_from_monday()));
                    if !take(date, &mut days) {
                        break 'weeks;
                    }
                }
                week += chrono::Duration::weeks(step);
            }
        }
        "MONTHLY" | "YEARLY" => {
            let months = if rule.freq == "YEARLY" { 12 } else { 1 } * rule.interval;
            let mut first = start.with_day(1).unwrap_or(start);
            'months: loop {
                let mut found: Vec<NaiveDate> = if !rule.by_day.is_empty() && rule.freq == "MONTHLY"
                {
                    rule.by_day
                        .iter()
                        .flat_map(|(nth, day)| nth_weekdays(first, *nth, *day))
                        .collect()
                } else if !rule.by_month_day.is_empty() {
                    rule.by_month_day
                        .iter()
                        .filter_map(|d| month_day(first, *d))
                        .collect()
                } else {
                    month_day(first, start.day() as i32).into_iter().collect()
                };
                found.sort();
                for date in found {
                    if !take(date, &mut days) {
                        break 'months;
                    }
                }
                match first.checked_add_months(Months::new(months)) {
                    Some(next) if next <= end => first = next,
                    _ => break,
                }
            }
        }
        _ => {}
    }
    days
}

// The nth weekday of a month (2TU is the second Tuesday, -1FR the last Friday); 0 means all of them.
fn nth_weekdays(first: NaiveDate, nth: i32, day: Weekday) -> Vec<NaiveDate> {
    let all: Vec<NaiveDate> = first
        .iter_days()
        .take_while(|d| d.month() == first.month())
        .filter(|d| d.weekday() == day)
        .collect();
    match nth {
        0 => all,
        n if n > 0 => all.get(n as usize - 1).copied().into_iter().collect(),
        n => all
            .len()
            .checked_sub(n.unsigned_abs() as usize)
            .and_then(|i| all.get(i).copied())
            .into_iter()
            .collect(),
    }
}

fn month_day(first: NaiveDate, day: i32) -> Option<NaiveDate> {
    if day > 0 {
        return first.with_day(day as u32);
    }
    let last = first.checked_add_months(Months::new(1))? - chrono::Duration::days(1);
    last.with_day((last.day() as i32 + day + 1).try_into().ok()?)
}

fn parse(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut current: Option<Event> = None;
    for line in unfold(text) {
        let Some((name, params, value)) = property(&line) else {
            continue;
        };
        match (name.as_str(), current.as_mut()) {
            ("BEGIN", None) if value.eq_ignore_ascii_case("VEVENT") => {
                current = Some(Event::default())
            }
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => {
                events.extend(current.take());
            }
            ("UID", Some(event)) => event.uid = value.to_owned(),
            ("SUMMARY", Some(event)) => event.summary = unescape(value),
            ("LOCATION", Some(event)) => {
                event.location = Some(unescape(value)).filter(|l| !l.is_empty())
            }
            ("DTSTART", Some(event)) => event.start = moment(&params, value),
            ("DTEND", Some(event)) => event.end = moment(&params, value),
            ("RRULE", Some(event)) => event.rule = rule(value),
            ("EXDATE", Some(event)) => event.except.extend(
                value
                    .split(',')
                    .filter_map(|v| moment(&params, v))
                    .map(|m| m.at.date()),
            ),
            ("RECURRENCE-ID", Some(event)) => {
                event.instead_of = moment(&params, value).map(|m| m.at.date())
            }
            ("STATUS", Some(event)) => event.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    const ICS: &str = "BEGIN:VCALENDAR\r
BEGIN:VEVENT\r
UID:standup\r
SUMMARY:Standup\\, team\r
DTSTART:20261005T100000\r
DTEND:20261005T101500\r
RRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=6\r
EXDATE:20261012T100000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:standup\r
RECURRENCE-ID:20261014T100000\r
SUMMARY:Standup\\, moved\r
DTSTART:20261015T120000\r
DTEND:20261015T121500\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:trip\r
SUMMARY:Trip to the\r
  mountains\r
DTSTART;VALUE=DATE:20261030\r
DTEND;VALUE=DATE:20261102\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:rent\r
SUMMARY:Rent\r
DTSTART;VALUE=DATE:20260130\r
RRULE:FREQ=MONTHLY;BYMONTHDAY=-1\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:club\r
SUMMARY:Book club\r
DTSTART:20261008T150000\r
RRULE:FREQ=MONTHLY;BYDAY=2TH\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:gone\r
SUMMARY:Cancelled thing\r
STATUS:CANCELLED\r
DTSTART;VALUE=DATE:20261031\r
END:VEVENT\r
END:VCALENDAR\r
";

    #[test]
    fn a_calendar_file_lands_on_the_right_days() {
        let agenda = agenda(&[ICS.to_owned()], day(2026, 10, 31));
        let titles = |d: NaiveDate| -> Vec<String> {
            agenda.on(d).iter().map(|o| o.summary.clone()).collect()
        };
        assert_eq!(titles(day(2026, 10, 5)), ["Standup, team"]);
        assert_eq!(titles(day(2026, 10, 7)), ["Standup, team"]);
        assert!(titles(day(2026, 10, 12)).is_empty(), "skipped that Monday");
        assert!(titles(day(2026, 10, 14)).is_empty(), "moved to Thursday");
        assert_eq!(titles(day(2026, 10, 15)), ["Standup, moved"]);
        assert_eq!(titles(day(2026, 10, 21)), ["Standup, team"]);
        assert!(titles(day(2026, 10, 26)).is_empty(), "six times, then over");
        assert_eq!(
            titles(day(2026, 10, 31)),
            ["Rent", "Trip to the mountains"].map(String::from)
        );
        assert_eq!(titles(day(2026, 11, 1)), ["Trip to the mountains"]);
        assert!(
            titles(day(2026, 11, 2)).is_empty(),
            "the end day isn't part of it"
        );
        assert_eq!(
            titles(day(2027, 2, 28)),
            ["Rent"],
            "the last day of a short month"
        );
        assert_eq!(titles(day(2026, 11, 12)), ["Book club"], "second Thursday");
        assert!(
            agenda
                .on(day(2026, 10, 31))
                .iter()
                .all(|o| o.start.is_none())
        );
        let standup = &agenda.on(day(2026, 10, 5))[0];
        assert_eq!(standup.start, NaiveTime::from_hms_opt(10, 0, 0));
        assert_eq!(standup.end, NaiveTime::from_hms_opt(10, 15, 0));
    }

    #[test]
    fn times_move_into_your_own_time_zone() {
        let ics = "BEGIN:VEVENT\nUID:a\nSUMMARY:Call\nDTSTART;TZID=Europe/Moscow:20261105T100000\nEND:VEVENT\nBEGIN:VEVENT\nUID:b\nSUMMARY:Launch\nDTSTART:20261105T220000Z\nEND:VEVENT\n";
        let agenda = agenda(&[ics.to_owned()], day(2026, 11, 1));
        let moscow = chrono_tz::Europe::Moscow
            .with_ymd_and_hms(2026, 11, 5, 10, 0, 0)
            .unwrap()
            .with_timezone(&Local);
        let utc = Utc
            .with_ymd_and_hms(2026, 11, 5, 22, 0, 0)
            .unwrap()
            .with_timezone(&Local);
        for (at, summary) in [(moscow, "Call"), (utc, "Launch")] {
            let found = agenda.on(at.date_naive());
            assert!(
                found
                    .iter()
                    .any(|o| o.summary == summary && o.start == Some(at.time())),
                "{summary} at {at}: {found:?}"
            );
        }
    }

    #[test]
    fn quoted_parameters_may_hold_colons() {
        let (name, params, value) =
            property("DTSTART;TZID=\"America/New_York:odd\":20261005T100000").unwrap();
        assert_eq!(name, "DTSTART");
        assert_eq!(params[0].1, "America/New_York:odd");
        assert_eq!(value, "20261005T100000");
    }
}

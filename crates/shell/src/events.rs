use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use chrono::{
    DateTime, Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone,
    Utc, Weekday,
};
use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::shell::Message;

const EVERY: Duration = Duration::from_secs(10 * 60);
const RETRY: Duration = Duration::from_secs(60);
// How far from today occurrences are worked out, both ways.
const REACH: i64 = 400;
// Limits against a calendar that would eat the machine.
const MOST: usize = 20_000;
const STEPS: usize = 200_000;
const LONGEST: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occurrence {
    // None on a whole day, and on the days a long event carries over into.
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
    pub summary: Arc<str>,
    pub location: Option<Arc<str>>,
    // When a timed event begins and ends, in local time.
    span: Option<(NaiveDateTime, NaiveDateTime)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agenda {
    days: BTreeMap<NaiveDate, Vec<Occurrence>>,
    count: usize,
}

impl Agenda {
    pub fn on(&self, day: NaiveDate) -> &[Occurrence] {
        self.days.get(&day).map_or(&[], Vec::as_slice)
    }

    pub fn busy(&self, day: NaiveDate) -> bool {
        self.days.contains_key(&day)
    }

    // What's on now, or what starts within half an hour, for the bar.
    pub fn soon(&self, now: NaiveDateTime) -> Option<(&Occurrence, Option<i64>)> {
        let tomorrow = now.date().succ_opt().unwrap_or(now.date());
        let timed = self
            .on(now.date())
            .iter()
            .chain(self.on(tomorrow))
            .filter_map(|o| Some((o, o.span?)));
        let mut upcoming: Option<(&Occurrence, i64)> = None;
        for (occurrence, (begins, ends)) in timed {
            if begins <= now && now < ends {
                return Some((occurrence, None));
            }
            // Whole minutes, rounded up: 9:50:30 to 10:00 is "in 10 min".
            let wait = ((begins - now).num_seconds() + 59) / 60;
            if begins > now && wait <= 30 && upcoming.is_none_or(|(_, w)| wait < w) {
                upcoming = Some((occurrence, wait));
            }
        }
        upcoming.map(|(occurrence, wait)| (occurrence, Some(wait)))
    }

    fn add(&mut self, day: NaiveDate, occurrence: Occurrence) {
        if self.count < MOST {
            self.count += 1;
            self.days.entry(day).or_default().push(occurrence);
        }
    }

    fn finish(mut self) -> Self {
        for list in self.days.values_mut() {
            list.sort();
            list.dedup();
        }
        self
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
        // A source that can't be read this time keeps what it said last time.
        let mut last: HashMap<String, String> = HashMap::new();
        loop {
            let wanted = sources.clone();
            let known = last.clone();
            let done = tokio::task::spawn_blocking(move || {
                let mut texts = known;
                let mut failed = false;
                for source in &wanted {
                    match read(source) {
                        Ok(text) => {
                            texts.insert(source.clone(), text);
                        }
                        Err(err) => {
                            failed = true;
                            eprintln!("apolline: calendar {}: {err}", shown(source));
                        }
                    }
                }
                texts.retain(|source, _| wanted.contains(source));
                let all: Vec<String> = texts.values().cloned().collect();
                (agenda(&all, Local::now().date_naive()), texts, failed)
            })
            .await;
            let wait = match done {
                Ok((agenda, texts, failed)) => {
                    last = texts;
                    if output
                        .send(Message::Agenda(Arc::new(agenda)))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    if failed { RETRY } else { EVERY }
                }
                Err(_) => EVERY,
            };
            tokio::time::sleep(wait).await;
        }
    })
}

// A link shows only its host: a calendar's secret address, or a login in it, is a password of sorts.
pub fn shown(source: &str) -> &str {
    match source.split_once("://") {
        Some((_, rest)) => {
            let place = rest.split(['/', '?', '#']).next().unwrap_or(rest);
            place.rsplit('@').next().unwrap_or(place)
        }
        None => source,
    }
}

// "Standup · in 10 min", or "· now" while it's on.
pub fn when(summary: &str, wait: Option<i64>) -> String {
    match wait {
        None => format!("{summary} · now"),
        Some(minutes) => format!("{summary} · in {minutes} min"),
    }
}

// A file, or a subscription link (webcal:// is https://). No accounts, no sign-in.
fn read(source: &str) -> Result<String, String> {
    let link = source
        .strip_prefix("webcal://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| source.to_owned());
    if link.starts_with("https://") {
        return ureq::get(&link)
            .config()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
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

impl Zone {
    fn instant(self, at: NaiveDateTime) -> Option<DateTime<Utc>> {
        Some(match self {
            Zone::Floating => Local
                .from_local_datetime(&at)
                .earliest()?
                .with_timezone(&Utc),
            Zone::Utc => Utc.from_utc_datetime(&at),
            Zone::Named(tz) => tz.from_local_datetime(&at).earliest()?.with_timezone(&Utc),
        })
    }

    fn wall(self, instant: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Zone::Floating => instant.with_timezone(&Local).naive_local(),
            Zone::Utc => instant.naive_utc(),
            Zone::Named(tz) => instant.with_timezone(&tz).naive_local(),
        }
    }

    // IANA names, Mozilla's "/mozilla.org/…/Europe/Berlin" and Outlook's Windows names.
    fn named(name: &str) -> Zone {
        let name = name.trim();
        let mut tail = name;
        loop {
            if let Ok(tz) = tail.trim_start_matches('/').parse() {
                return Zone::Named(tz);
            }
            match tail.split_once('/') {
                Some((_, rest)) => tail = rest,
                None => break,
            }
        }
        WINDOWS
            .iter()
            .find(|(windows, _)| windows.eq_ignore_ascii_case(name))
            .and_then(|(_, iana)| iana.parse().ok())
            .map_or(Zone::Floating, Zone::Named)
    }
}

const WINDOWS: [(&str, &str); 36] = [
    ("UTC", "UTC"),
    ("Coordinated Universal Time", "UTC"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Mountain Standard Time", "America/Denver"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Central Standard Time", "America/Chicago"),
    ("Eastern Standard Time", "America/New_York"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Central Standard Time (Mexico)", "America/Mexico_City"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("FLE Standard Time", "Europe/Kyiv"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("West Asia Standard Time", "Asia/Tashkent"),
    ("Ekaterinburg Standard Time", "Asia/Yekaterinburg"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Central Asia Standard Time", "Asia/Bishkek"),
    ("China Standard Time", "Asia/Shanghai"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Moment {
    at: NaiveDateTime,
    zone: Zone,
    whole_day: bool,
}

impl Moment {
    fn local(&self) -> NaiveDateTime {
        if self.whole_day {
            return self.at;
        }
        self.zone
            .instant(self.at)
            .map_or(self.at, |instant| Zone::Floating.wall(instant))
    }

    // The same moment as the wall clock of another zone shows it.
    fn in_zone(&self, zone: Zone) -> NaiveDateTime {
        if self.whole_day || self.zone == zone {
            return self.at;
        }
        self.zone
            .instant(self.at)
            .map_or(self.at, |instant| zone.wall(instant))
    }
}

#[derive(Clone, Debug, Default)]
struct Rule {
    freq: String,
    interval: u32,
    count: Option<usize>,
    until: Option<Moment>,
    by_day: Vec<(i32, Weekday)>,
    by_month_day: Vec<i32>,
    by_month: Vec<u32>,
    by_set_pos: Vec<i32>,
}

#[derive(Debug, Default)]
struct Event {
    uid: String,
    summary: Arc<str>,
    location: Option<Arc<str>>,
    start: Option<Moment>,
    end: Option<Moment>,
    rule: Option<Rule>,
    except: Vec<Moment>,
    instead_of: Option<Moment>,
    cancelled: bool,
}

pub fn agenda(texts: &[String], today: NaiveDate) -> Agenda {
    let mut events: Vec<Event> = texts.iter().flat_map(|text| parse(text)).collect();
    // A moved or cancelled meeting replaces that one day of its series.
    let moved: Vec<(String, Moment)> = events
        .iter()
        .filter_map(|e| Some((e.uid.clone(), e.instead_of?)))
        .collect();
    for event in events.iter_mut().filter(|e| e.instead_of.is_none()) {
        event.except.extend(
            moved
                .iter()
                .filter(|(uid, _)| *uid == event.uid)
                .map(|(_, at)| *at),
        );
    }
    let from = today - TimeDelta::days(REACH);
    let until = today + TimeDelta::days(REACH);
    let mut agenda = Agenda::default();
    for event in events.iter().filter(|e| !e.cancelled) {
        let Some(start) = event.start else { continue };
        let skipped: Vec<NaiveDate> = event
            .except
            .iter()
            .map(|at| at.in_zone(start.zone).date())
            .collect();
        let days = match &event.rule {
            Some(rule) => expand(start, rule, from - TimeDelta::days(60), until),
            None => vec![start.at.date()],
        };
        for day in days.into_iter().filter(|d| !skipped.contains(d)) {
            if start.whole_day {
                let length = event
                    .end
                    .map(|end| (end.at.date() - start.at.date()).num_days())
                    .unwrap_or(1)
                    .clamp(1, 60);
                for offset in 0..length {
                    let date = day + TimeDelta::days(offset);
                    if (from..=until).contains(&date) {
                        agenda.add(date, occurrence(event, None, None, None));
                    }
                }
                continue;
            }
            let begins = Moment {
                at: day.and_time(start.at.time()),
                ..start
            }
            .local();
            // Measured between the two as they really are, so a flight across zones keeps its length.
            let length = event
                .end
                .map(|end| end.local() - start.local())
                .filter(|length| *length > TimeDelta::zero())
                .unwrap_or_default()
                .min(TimeDelta::days(14));
            let ends = begins + length;
            let last = if ends.time() == NaiveTime::MIN && ends.date() > begins.date() {
                ends.date().pred_opt().unwrap_or(ends.date())
            } else {
                ends.date()
            };
            let mut date = begins.date();
            while date <= last {
                if (from..=until).contains(&date) {
                    let first = date == begins.date();
                    let end = (length > TimeDelta::zero() && (first || date == last))
                        .then(|| ends.time());
                    let start = first.then(|| begins.time());
                    agenda.add(date, occurrence(event, start, end, Some((begins, ends))));
                }
                let Some(next) = date.succ_opt() else { break };
                date = next;
            }
        }
    }
    agenda.finish()
}

fn occurrence(
    event: &Event,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    span: Option<(NaiveDateTime, NaiveDateTime)>,
) -> Occurrence {
    Occurrence {
        start,
        end,
        summary: event.summary.clone(),
        location: event.location.clone(),
        span,
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

fn unescape(value: &str) -> Arc<str> {
    let mut out = String::with_capacity(value.len().min(LONGEST * 4));
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if out.chars().count() >= LONGEST {
            break;
        }
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
    out.trim().into()
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
        param("TZID").map_or(Zone::Floating, Zone::named)
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

fn numbers<T: std::str::FromStr>(value: &str) -> Vec<T> {
    value
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

// None for a rule this reader doesn't follow; the event then shows once, on its first day.
fn rule(value: &str) -> Option<Rule> {
    let mut rule = Rule {
        interval: 1,
        ..Rule::default()
    };
    for part in value.split(';').filter(|part| !part.trim().is_empty()) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key.trim().to_ascii_uppercase().as_str() {
            "FREQ" => rule.freq = value.trim().to_ascii_uppercase(),
            "INTERVAL" => rule.interval = value.trim().parse().ok().filter(|i| *i > 0)?,
            "COUNT" => rule.count = value.trim().parse().ok(),
            "UNTIL" => rule.until = moment(&[], value),
            "BYDAY" => {
                rule.by_day = value
                    .split(',')
                    .filter_map(|day| {
                        let day = day.trim();
                        let split = day
                            .len()
                            .checked_sub(2)
                            .filter(|at| day.is_char_boundary(*at))?;
                        let (nth, code) = day.split_at(split);
                        let nth = match nth {
                            "" => 0,
                            nth => nth.parse().ok()?,
                        };
                        Some((nth, weekday(code)?))
                    })
                    .collect();
            }
            "BYMONTHDAY" => rule.by_month_day = numbers(value),
            "BYMONTH" => rule.by_month = numbers(value),
            "BYSETPOS" => rule.by_set_pos = numbers(value),
            "WKST" => {}
            _ => return None,
        }
    }
    let known = matches!(
        rule.freq.as_str(),
        "DAILY" | "WEEKLY" | "MONTHLY" | "YEARLY"
    );
    // Ordinals counted across a whole year aren't followed.
    let yearly_days = rule.freq == "YEARLY" && !rule.by_day.is_empty() && rule.by_month.is_empty();
    (known && !yearly_days).then_some(rule)
}

// The days a rule lands on, in the event's own zone, from `from` up to `limit`.
fn expand(start: Moment, rule: &Rule, from: NaiveDate, limit: NaiveDate) -> Vec<NaiveDate> {
    let first = start.at.date();
    let time = start.at.time();
    let until = rule.until.map(|until| {
        let at = until.in_zone(start.zone);
        if until.whole_day {
            at.date().and_hms_opt(23, 59, 59).unwrap_or(at)
        } else {
            at
        }
    });
    let mut days = Vec::new();
    let mut counted = 0usize;
    // Every day the rule lands on counts toward COUNT, even before the window.
    let mut take = |day: NaiveDate| -> bool {
        if day > limit || until.is_some_and(|until| day.and_time(time) > until) {
            return false;
        }
        if rule.count.is_some_and(|count| counted >= count) {
            return false;
        }
        counted += 1;
        if day >= from {
            days.push(day);
        }
        days.len() < MOST
    };
    // The first day is always one, as calendars treat it.
    if !take(first) {
        return days;
    }
    let weekdays: Vec<Weekday> = rule.by_day.iter().map(|(_, d)| *d).collect();
    let fits = |day: &NaiveDate| {
        (rule.by_month.is_empty() || rule.by_month.contains(&day.month()))
            && (rule.by_month_day.is_empty()
                || rule
                    .by_month_day
                    .iter()
                    .any(|d| month_day(*day, *d) == Some(*day)))
    };
    let mut period = match rule.freq.as_str() {
        "WEEKLY" => first.week(Weekday::Mon).first_day(),
        "MONTHLY" => first.with_day(1).unwrap_or(first),
        "YEARLY" => first.with_ordinal(1).unwrap_or(first),
        _ => first,
    };
    for _ in 0..STEPS {
        let mut found: Vec<NaiveDate> = match rule.freq.as_str() {
            "DAILY" => vec![period]
                .into_iter()
                .filter(|d| weekdays.is_empty() || weekdays.contains(&d.weekday()))
                .filter(fits)
                .collect(),
            "WEEKLY" => {
                let wanted = if weekdays.is_empty() {
                    vec![first.weekday()]
                } else {
                    weekdays.clone()
                };
                wanted
                    .iter()
                    .filter_map(|d| {
                        period.checked_add_signed(TimeDelta::days(i64::from(
                            d.num_days_from_monday(),
                        )))
                    })
                    .filter(fits)
                    .collect()
            }
            "MONTHLY" => in_month(period, rule, first)
                .into_iter()
                .filter(fits)
                .collect(),
            _ => {
                let months: Vec<u32> = if rule.by_month.is_empty() {
                    vec![first.month()]
                } else {
                    rule.by_month.clone()
                };
                months
                    .iter()
                    .filter_map(|m| period.with_month(*m))
                    .flat_map(|month| in_month(month, rule, first))
                    .collect()
            }
        };
        found.sort();
        found.dedup();
        if !rule.by_set_pos.is_empty() {
            let all = std::mem::take(&mut found);
            found = rule
                .by_set_pos
                .iter()
                .filter_map(|pos| match *pos {
                    p if p > 0 => all.get(p as usize - 1).copied(),
                    p if p < 0 => all
                        .len()
                        .checked_sub(p.unsigned_abs() as usize)
                        .and_then(|i| all.get(i).copied()),
                    _ => None,
                })
                .collect();
            found.sort();
        }
        for day in found.into_iter().filter(|day| *day > first) {
            if !take(day) {
                return days;
            }
        }
        let step = rule.interval;
        let next = match rule.freq.as_str() {
            "DAILY" => {
                TimeDelta::try_days(i64::from(step)).and_then(|d| period.checked_add_signed(d))
            }
            "WEEKLY" => {
                TimeDelta::try_weeks(i64::from(step)).and_then(|w| period.checked_add_signed(w))
            }
            "MONTHLY" => period.checked_add_months(Months::new(step)),
            _ => step
                .checked_mul(12)
                .and_then(|months| period.checked_add_months(Months::new(months))),
        };
        match next {
            Some(next) if next <= limit && until.is_none_or(|until| next <= until.date()) => {
                period = next
            }
            _ => break,
        }
    }
    days
}

// The days of one month a monthly rule names: by weekday, by day of the month, or the first day's date.
fn in_month(month: NaiveDate, rule: &Rule, first: NaiveDate) -> Vec<NaiveDate> {
    let month = month.with_day(1).unwrap_or(month);
    if !rule.by_day.is_empty() {
        rule.by_day
            .iter()
            .flat_map(|(nth, day)| nth_weekdays(month, *nth, *day))
            .collect()
    } else if !rule.by_month_day.is_empty() {
        rule.by_month_day
            .iter()
            .filter_map(|d| month_day(month, *d))
            .collect()
    } else {
        month_day(month, first.day() as i32).into_iter().collect()
    }
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

// Day `day` of the month `any` falls in; negative counts from the end.
fn month_day(any: NaiveDate, day: i32) -> Option<NaiveDate> {
    let first = any.with_day(1)?;
    if day > 0 {
        return first.with_day(day as u32);
    }
    let last = first.checked_add_months(Months::new(1))?.pred_opt()?;
    last.with_day((last.day() as i32 + day + 1).try_into().ok()?)
}

fn parse(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut current: Option<Event> = None;
    // Alarms and other parts nested in an event carry their own SUMMARY and UID.
    let mut inner = 0usize;
    for line in unfold(text) {
        let Some((name, params, value)) = property(&line) else {
            continue;
        };
        let component = value.trim();
        match (name.as_str(), current.as_mut()) {
            ("BEGIN", None) if component.eq_ignore_ascii_case("VEVENT") => {
                current = Some(Event::default());
                inner = 0;
            }
            ("BEGIN", Some(_)) => inner += 1,
            ("END", Some(_)) if inner > 0 => inner -= 1,
            ("END", Some(_)) if component.eq_ignore_ascii_case("VEVENT") => {
                events.extend(current.take());
            }
            (_, Some(_)) if inner > 0 => {}
            ("UID", Some(event)) => event.uid = value.to_owned(),
            ("SUMMARY", Some(event)) => event.summary = unescape(value),
            ("LOCATION", Some(event)) => {
                event.location = Some(unescape(value)).filter(|l| !l.is_empty())
            }
            ("DTSTART", Some(event)) => event.start = moment(&params, value),
            ("DTEND", Some(event)) => event.end = moment(&params, value),
            ("RRULE", Some(event)) => event.rule = rule(value),
            ("EXDATE", Some(event)) => event
                .except
                .extend(value.split(',').filter_map(|v| moment(&params, v))),
            ("RECURRENCE-ID", Some(event)) => event.instead_of = moment(&params, value),
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
            agenda.on(d).iter().map(|o| o.summary.to_string()).collect()
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
                    .any(|o| &*o.summary == summary && o.start == Some(at.time())),
                "{summary} at {at}: {found:?}"
            );
        }
    }

    #[test]
    fn the_bar_hears_of_what_is_on_or_coming() {
        let agenda = agenda(&[ICS.to_owned()], day(2026, 10, 31));
        let at = |h: u32, m: u32| day(2026, 10, 7).and_hms_opt(h, m, 0).unwrap();
        assert_eq!(agenda.soon(at(9, 0)), None, "an hour off is too far");
        let (standup, wait) = agenda.soon(at(9, 50)).unwrap();
        assert_eq!((&*standup.summary, wait), ("Standup, team", Some(10)));
        assert_eq!(agenda.soon(at(10, 5)).map(|(_, w)| w), Some(None), "on now");
        assert_eq!(agenda.soon(at(10, 20)), None, "over");
    }

    fn titles(agenda: &Agenda, d: NaiveDate) -> Vec<String> {
        agenda.on(d).iter().map(|o| o.summary.to_string()).collect()
    }

    fn event(lines: &str) -> String {
        format!(
            "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:x
SUMMARY:Thing
{lines}
END:VEVENT
END:VCALENDAR
"
        )
    }

    fn days_of(lines: &str, from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
        let agenda = agenda(&[event(lines)], from);
        from.iter_days()
            .take_while(|d| *d <= to)
            .filter(|d| agenda.busy(*d))
            .collect()
    }

    #[test]
    fn an_alarm_inside_an_event_is_not_the_event() {
        let ics = "BEGIN:VEVENT
UID:meeting
SUMMARY:Planning
DTSTART:20261102T100000
BEGIN:VALARM
UID:alarm-1
SUMMARY:Alarm notification
ACTION:EMAIL
END:VALARM
END:VEVENT
";
        let agenda = agenda(&[ics.to_owned()], day(2026, 11, 1));
        assert_eq!(titles(&agenda, day(2026, 11, 2)), ["Planning"]);
    }

    #[test]
    fn broken_or_hostile_rules_dont_take_the_calendar_down() {
        for rule in [
            "RRULE:FREQ=WEEKLY;BYDAY=€",
            "RRULE:FREQ=DAILY;INTERVAL=99999999",
            "RRULE:FREQ=YEARLY;INTERVAL=4294967295",
            "RRULE:FREQ=MONTHLY;BYSETPOS=-99;BYDAY=MO",
            "RRULE:",
        ] {
            let days = days_of(
                &format!(
                    "DTSTART:20261102T100000
{rule}"
                ),
                day(2026, 11, 1),
                day(2026, 11, 30),
            );
            assert!(
                days.contains(&day(2026, 11, 2)),
                "{rule}: the first day stays"
            );
        }
    }

    #[test]
    fn an_old_series_still_shows_today() {
        let days = days_of(
            "DTSTART:20120105T090000
RRULE:FREQ=DAILY",
            day(2026, 11, 1),
            day(2026, 11, 3),
        );
        assert_eq!(days.len(), 3);
        let weekly = days_of(
            "DTSTART:20060102T090000
RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
            day(2026, 11, 2),
            day(2026, 11, 8),
        );
        assert_eq!(weekly.len(), 5);
    }

    #[test]
    fn rules_from_outlook_thunderbird_and_google_land_right() {
        let last_weekday = days_of(
            "DTSTART:20260130T090000
RRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1",
            day(2026, 11, 1),
            day(2026, 12, 31),
        );
        assert_eq!(last_weekday, [day(2026, 11, 30), day(2026, 12, 31)]);
        let weekdays = days_of(
            "DTSTART:20261102T090000
RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR;",
            day(2026, 11, 2),
            day(2026, 11, 8),
        );
        assert_eq!(weekdays.len(), 5, "no weekend, and the stray ; is fine");
        let thanksgiving = days_of(
            "DTSTART;VALUE=DATE:20261126
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=4TH",
            day(2027, 11, 1),
            day(2027, 11, 30),
        );
        assert_eq!(thanksgiving, [day(2027, 11, 25)]);
        let hourly = days_of(
            "DTSTART:20261102T090000
RRULE:FREQ=DAILY;BYHOUR=9,17",
            day(2026, 11, 1),
            day(2026, 11, 10),
        );
        assert_eq!(
            hourly,
            [day(2026, 11, 2)],
            "a rule it can't follow shows only its first day"
        );
    }

    #[test]
    fn until_and_skipped_days_count_in_the_event_s_own_zone() {
        let tokyo = Moment {
            at: day(2026, 10, 5).and_hms_opt(8, 0, 0).unwrap(),
            zone: Zone::named("Asia/Tokyo"),
            whole_day: false,
        };
        let weekly = rule("FREQ=WEEKLY;UNTIL=20261018T230000Z").unwrap();
        assert_eq!(
            expand(tokyo, &weekly, day(2026, 10, 1), day(2026, 12, 1)),
            [day(2026, 10, 5), day(2026, 10, 12), day(2026, 10, 19)]
        );
        let ics = event(
            "DTSTART;TZID=America/Los_Angeles:20261005T180000
RRULE:FREQ=WEEKLY;COUNT=3
EXDATE:20261013T010000Z",
        );
        let agenda = agenda(&[ics], day(2026, 10, 1));
        assert_eq!(agenda.count, 2, "the skipped evening is gone");
        assert_eq!(
            Zone::named("Pacific Standard Time"),
            Zone::named("America/Los_Angeles")
        );
        assert_eq!(
            Zone::named("/mozilla.org/20070129_1/Europe/Berlin"),
            Zone::named("Europe/Berlin")
        );
    }

    #[test]
    fn a_night_shift_runs_past_midnight() {
        let agenda = agenda(
            &[event(
                "DTSTART:20261101T220000
DTEND:20261102T060000",
            )],
            day(2026, 11, 1),
        );
        let first = &agenda.on(day(2026, 11, 1))[0];
        assert_eq!(
            (first.start, first.end),
            (
                NaiveTime::from_hms_opt(22, 0, 0),
                NaiveTime::from_hms_opt(6, 0, 0)
            )
        );
        let second = &agenda.on(day(2026, 11, 2))[0];
        assert_eq!(
            (second.start, second.end),
            (None, NaiveTime::from_hms_opt(6, 0, 0))
        );
        let at = |d: u32, h: u32| day(2026, 11, d).and_hms_opt(h, 0, 0).unwrap();
        assert_eq!(agenda.soon(at(1, 23)).map(|(_, w)| w), Some(None));
        assert_eq!(agenda.soon(at(2, 1)).map(|(_, w)| w), Some(None));
        assert_eq!(agenda.soon(at(2, 7)), None);
    }

    #[test]
    fn long_titles_are_cut() {
        let long = "x".repeat(100_000);
        let agenda = agenda(
            &[event(&format!(
                "SUMMARY:{long}
DTSTART:20261102T100000"
            ))],
            day(2026, 11, 1),
        );
        assert_eq!(
            agenda.on(day(2026, 11, 2))[0].summary.chars().count(),
            LONGEST
        );
    }

    #[test]
    fn links_show_only_their_host() {
        assert_eq!(
            shown("https://calendar.google.com/calendar/ical/secret/basic.ics"),
            "calendar.google.com"
        );
        assert_eq!(
            shown("webcal://me:hunter2@dav.example.org/cal.ics"),
            "dav.example.org"
        );
        assert_eq!(shown("~/calendars/home.ics"), "~/calendars/home.ics");
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

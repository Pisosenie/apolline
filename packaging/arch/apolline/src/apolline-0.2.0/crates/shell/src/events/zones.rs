use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Zone {
    Floating,
    Utc,
    Named(chrono_tz::Tz),
}

impl Zone {
    // A time the clocks skipped in spring is read as the hour after.
    pub(super) fn instant(self, at: NaiveDateTime) -> Option<DateTime<Utc>> {
        let later = at + TimeDelta::hours(1);
        Some(match self {
            Zone::Floating => Local
                .from_local_datetime(&at)
                .earliest()
                .or_else(|| Local.from_local_datetime(&later).earliest())?
                .with_timezone(&Utc),
            Zone::Utc => Utc.from_utc_datetime(&at),
            Zone::Named(tz) => tz
                .from_local_datetime(&at)
                .earliest()
                .or_else(|| tz.from_local_datetime(&later).earliest())?
                .with_timezone(&Utc),
        })
    }

    pub(super) fn wall(self, instant: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Zone::Floating => instant.with_timezone(&Local).naive_local(),
            Zone::Utc => instant.naive_utc(),
            Zone::Named(tz) => instant.with_timezone(&tz).naive_local(),
        }
    }

    // IANA names, Mozilla's "/mozilla.org/…/Europe/Berlin" and Outlook's Windows names.
    pub(super) fn named(name: &str) -> Zone {
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

pub(super) const WINDOWS: [(&str, &str); 36] = [
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

// One STANDARD or DAYLIGHT part of a VTIMEZONE: from when, and the offsets either side of it.
pub(super) struct Observance {
    pub(super) start: NaiveDateTime,
    pub(super) from: i32,
    pub(super) to: i32,
    pub(super) rule: Option<Rule>,
}

// Outlook names its zones as it likes ("Customized Time Zone") and describes them in a VTIMEZONE.
// Each such zone is matched to the IANA zone that keeps the same offsets through the year.
pub(super) fn zones(text: &str, year: i32) -> Vec<(String, chrono_tz::Tz)> {
    let mut found = Vec::new();
    let mut tzid: Option<String> = None;
    let mut parts: Vec<Observance> = Vec::new();
    let mut part: Option<Observance> = None;
    for line in unfold(text) {
        let Some((name, _, value)) = property(&line) else {
            continue;
        };
        let value = value.trim();
        let observance = ["STANDARD", "DAYLIGHT"]
            .iter()
            .any(|kind| value.eq_ignore_ascii_case(kind));
        match (name.as_str(), part.as_mut()) {
            ("BEGIN", _) if value.eq_ignore_ascii_case("VTIMEZONE") => {
                tzid = None;
                parts.clear();
            }
            ("BEGIN", None) if observance => {
                part = Some(Observance {
                    start: NaiveDateTime::MIN,
                    from: 0,
                    to: 0,
                    rule: None,
                });
            }
            ("END", Some(_)) if observance => parts.extend(part.take()),
            ("END", None) if value.eq_ignore_ascii_case("VTIMEZONE") => {
                if let Some(id) = tzid.take()
                    && Zone::named(&id) == Zone::Floating
                    && let Some(tz) = matching(&parts, year)
                {
                    found.push((id, tz));
                }
            }
            ("TZID", None) => tzid = Some(value.to_owned()),
            ("DTSTART", Some(part)) => {
                if let Ok(start) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
                    part.start = start;
                }
            }
            ("TZOFFSETFROM", Some(part)) => part.from = utc_offset(value).unwrap_or(part.from),
            ("TZOFFSETTO", Some(part)) => part.to = utc_offset(value).unwrap_or(part.to),
            ("RRULE", Some(part)) => part.rule = rule(value),
            _ => {}
        }
    }
    found
}

// "+0100", "-0530", "+013045".
pub(super) fn utc_offset(value: &str) -> Option<i32> {
    let (sign, digits) = match value.split_at_checked(1)? {
        ("+", digits) => (1, digits),
        ("-", digits) => (-1, digits),
        _ => return None,
    };
    if !matches!(digits.len(), 4 | 6) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let field = |at: usize| digits.get(at..at + 2).and_then(|d| d.parse::<i32>().ok());
    Some(sign * (field(0)? * 3600 + field(2)? * 60 + field(4).unwrap_or(0)))
}

pub(super) fn matching(parts: &[Observance], year: i32) -> Option<chrono_tz::Tz> {
    let days: Vec<NaiveDateTime> = NaiveDate::from_ymd_opt(year, 1, 1)?
        .iter_days()
        .take_while(|day| day.year() == year)
        .filter_map(|day| day.and_hms_opt(12, 0, 0))
        .collect();
    let ours: Vec<i32> = days
        .iter()
        .map(|at| offset_at(parts, *at))
        .collect::<Option<_>>()?;
    let theirs = |tz: &chrono_tz::Tz, at: &NaiveDateTime| {
        tz.offset_from_utc_datetime(at).fix().local_minus_utc()
    };
    chrono_tz::TZ_VARIANTS
        .iter()
        .filter(|tz| theirs(tz, &days[14]) == ours[14] && theirs(tz, &days[195]) == ours[195])
        .find(|tz| {
            days.iter()
                .zip(&ours)
                .all(|(at, ours)| theirs(tz, at) == *ours)
        })
        .copied()
}

// The offset in force at a UTC time: the latest onset before it wins.
pub(super) fn offset_at(parts: &[Observance], at: NaiveDateTime) -> Option<i32> {
    let mut latest: Option<(NaiveDateTime, i32)> = None;
    for part in parts {
        let onsets: Vec<NaiveDateTime> = match &part.rule {
            Some(rule) => (at.year() - 1..=at.year())
                .filter_map(|year| onset(part, rule, year))
                .collect(),
            None => vec![part.start],
        };
        for local in onsets {
            let onset = local - TimeDelta::seconds(part.from.into());
            if onset <= at && latest.is_none_or(|(seen, _)| onset > seen) {
                latest = Some((onset, part.to));
            }
        }
    }
    latest
        .map(|(_, to)| to)
        .or_else(|| parts.iter().min_by_key(|p| p.start).map(|p| p.from))
}

// When a yearly change happens in a given year: "the last Sunday of October at 03:00".
pub(super) fn onset(part: &Observance, rule: &Rule, year: i32) -> Option<NaiveDateTime> {
    if year < part.start.year() {
        return None;
    }
    let month = rule.by_month.first().copied().unwrap_or(part.start.month());
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let day = match (rule.by_day.first(), rule.by_month_day.is_empty()) {
        (Some((0, weekday)), false) => rule
            .by_month_day
            .iter()
            .filter_map(|d| month_day(first, *d))
            .find(|d| d.weekday() == *weekday)?,
        (Some((nth, weekday)), _) => *nth_weekdays(first, *nth, *weekday).first()?,
        (None, false) => month_day(first, rule.by_month_day[0])?,
        (None, true) => first.with_day(part.start.day())?,
    };
    Some(day.and_time(part.start.time()))
}

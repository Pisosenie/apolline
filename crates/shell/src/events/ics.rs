use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Moment {
    pub(super) at: NaiveDateTime,
    pub(super) zone: Zone,
    pub(super) whole_day: bool,
}

impl Moment {
    pub(super) fn local(&self) -> NaiveDateTime {
        if self.whole_day {
            return self.at;
        }
        self.zone
            .instant(self.at)
            .map_or(self.at, |instant| Zone::Floating.wall(instant))
    }

    // The same moment as the wall clock of another zone shows it.
    pub(super) fn in_zone(&self, zone: Zone) -> NaiveDateTime {
        if self.whole_day || self.zone == zone {
            return self.at;
        }
        self.zone
            .instant(self.at)
            .map_or(self.at, |instant| zone.wall(instant))
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Rule {
    pub(super) freq: String,
    pub(super) interval: u32,
    pub(super) count: Option<usize>,
    pub(super) until: Option<Moment>,
    pub(super) by_day: Vec<(i32, Weekday)>,
    pub(super) by_month_day: Vec<i32>,
    pub(super) by_month: Vec<u32>,
    pub(super) by_set_pos: Vec<i32>,
    pub(super) week_start: Option<Weekday>,
}

#[derive(Debug, Default)]
pub(super) struct Event {
    pub(super) uid: String,
    pub(super) summary: Arc<str>,
    pub(super) location: Option<Arc<str>>,
    pub(super) call: Option<Arc<str>>,
    pub(super) alarms: Vec<TimeDelta>,
    pub(super) start: Option<Moment>,
    pub(super) end: Option<Moment>,
    // How long it lasts, for events that give a length instead of an end.
    pub(super) duration: Option<TimeDelta>,
    pub(super) rule: Option<Rule>,
    pub(super) except: Vec<Moment>,
    pub(super) instead_of: Option<Moment>,
    pub(super) cancelled: bool,
}

pub fn agenda(texts: &[String], today: NaiveDate) -> Agenda {
    let mut events: Vec<Event> = texts
        .iter()
        .flat_map(|text| parse(text, today.year()))
        .collect();
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
                    .or_else(|| event.duration.map(|d| d.num_days()))
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
                .or(event.duration)
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

pub(super) fn occurrence(
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
        call: event.call.clone(),
        span,
        alarms: event.alarms.clone(),
    }
}

// Lines folded onto the next one start with a space or a tab.
pub(super) fn unfold(text: &str) -> Vec<String> {
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
pub(super) type Params = Vec<(String, String)>;

pub(super) fn property(line: &str) -> Option<(String, Params, &str)> {
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

pub(super) fn unescape(value: &str) -> Arc<str> {
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

pub(super) fn moment(params: &[(String, String)], value: &str) -> Option<Moment> {
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

// How long before the start an alarm's trigger goes off: "-PT15M" is fifteen minutes. Triggers
// at a set time, after the start or from the end aren't reminders of a start.
pub(super) fn lead(params: &[(String, String)], value: &str) -> Option<TimeDelta> {
    let param = |key: &str| {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    if param("VALUE") == Some("DATE-TIME") || param("RELATED") == Some("END") {
        return None;
    }
    let value = value.trim();
    let lead = match value.strip_prefix('-') {
        Some(before) => duration(before)?,
        None => duration(value).filter(|d| d.is_zero())?,
    };
    (lead <= LONGEST_LEAD).then_some(lead)
}

// "PT1H30M", "P3D", "P1W": a length as RFC 5545 writes it.
pub(super) fn duration(value: &str) -> Option<TimeDelta> {
    let value = value.trim().trim_start_matches('+');
    let rest = value.strip_prefix('P')?;
    let mut total = TimeDelta::zero();
    let mut number = String::new();
    let mut time = false;
    for c in rest.chars() {
        match c {
            'T' => time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                let part = match (unit, time) {
                    ('W', false) => TimeDelta::try_weeks(n)?,
                    ('D', false) => TimeDelta::try_days(n)?,
                    ('H', true) => TimeDelta::try_hours(n)?,
                    ('M', true) => TimeDelta::try_minutes(n)?,
                    ('S', true) => TimeDelta::try_seconds(n)?,
                    _ => return None,
                };
                total = total.checked_add(&part)?;
            }
        }
    }
    number.is_empty().then_some(total)
}

pub(super) fn weekday(code: &str) -> Option<Weekday> {
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

pub(super) fn numbers<T: std::str::FromStr>(value: &str) -> Vec<T> {
    value
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

// None for a rule this reader doesn't follow; the event then shows once, on its first day.
pub(super) fn rule(value: &str) -> Option<Rule> {
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
                        let day = day.trim().to_ascii_uppercase();
                        let day = day.as_str();
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
            "WKST" => rule.week_start = Some(weekday(&value.trim().to_ascii_uppercase())?),
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
pub(super) fn expand(
    start: Moment,
    rule: &Rule,
    from: NaiveDate,
    limit: NaiveDate,
) -> Vec<NaiveDate> {
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
    let week_start = rule.week_start.unwrap_or(Weekday::Mon);
    let mut period = match rule.freq.as_str() {
        "WEEKLY" => first.week(week_start).first_day(),
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
                        let offset =
                            (d.num_days_from_monday() + 7 - week_start.num_days_from_monday()) % 7;
                        period.checked_add_signed(TimeDelta::days(i64::from(offset)))
                    })
                    .filter(fits)
                    .collect()
            }
            "MONTHLY" => in_month(period, rule, first)
                .into_iter()
                .filter(fits)
                .collect(),
            _ => {
                // Days of the month without months mean every month of the year.
                let months: Vec<u32> =
                    match (rule.by_month.is_empty(), rule.by_month_day.is_empty()) {
                        (false, _) => rule.by_month.clone(),
                        (true, false) => (1..=12).collect(),
                        (true, true) => vec![first.month()],
                    };
                months
                    .iter()
                    .filter_map(|m| period.with_month(*m))
                    .flat_map(|month| in_month(month, rule, first))
                    .filter(fits)
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
pub(super) fn in_month(month: NaiveDate, rule: &Rule, first: NaiveDate) -> Vec<NaiveDate> {
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
pub(super) fn nth_weekdays(first: NaiveDate, nth: i32, day: Weekday) -> Vec<NaiveDate> {
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
pub(super) fn month_day(any: NaiveDate, day: i32) -> Option<NaiveDate> {
    let first = any.with_day(1)?;
    if day > 0 {
        return first.with_day(day as u32);
    }
    let last = first.checked_add_months(Months::new(1))?.pred_opt()?;
    last.with_day((last.day() as i32 + day + 1).try_into().ok()?)
}

pub(super) fn parse(text: &str, year: i32) -> Vec<Event> {
    let zones = zones(text, year);
    let mut events = Vec::new();
    let mut current: Option<Event> = None;
    // Alarms and other parts nested in an event carry their own SUMMARY and UID.
    let mut inner = 0usize;
    let mut in_alarm = false;
    for line in unfold(text) {
        let Some((name, mut params, value)) = property(&line) else {
            continue;
        };
        for (key, value) in &mut params {
            if key == "TZID"
                && let Some((_, tz)) = zones.iter().find(|(id, _)| id == value)
            {
                *value = tz.name().to_owned();
            }
        }
        let component = value.trim();
        match (name.as_str(), current.as_mut()) {
            ("BEGIN", None) if component.eq_ignore_ascii_case("VEVENT") => {
                current = Some(Event::default());
                inner = 0;
            }
            ("BEGIN", Some(_)) => {
                in_alarm = inner == 0 && component.eq_ignore_ascii_case("VALARM");
                inner += 1;
            }
            ("END", Some(_)) if inner > 0 => {
                inner -= 1;
                in_alarm = in_alarm && inner > 0;
            }
            ("TRIGGER", Some(event)) if in_alarm && inner == 1 => {
                event.alarms.extend(lead(&params, value));
            }
            ("END", Some(_)) if component.eq_ignore_ascii_case("VEVENT") => {
                events.extend(current.take());
            }
            (_, Some(_)) if inner > 0 => {}
            ("UID", Some(event)) => event.uid = value.to_owned(),
            ("SUMMARY", Some(event)) => event.summary = unescape(value),
            ("LOCATION", Some(event)) => {
                let link = call(value);
                // A place that is only the call's link shows as the Join button instead.
                event.location = Some(unescape(value))
                    .filter(|l| !l.is_empty() && link.as_deref() != Some(l.trim()));
                event.call = event.call.take().or_else(|| link.map(Arc::from));
            }
            (
                "URL"
                | "DESCRIPTION"
                | "CONFERENCE"
                | "X-GOOGLE-CONFERENCE"
                | "X-MICROSOFT-SKYPETEAMSMEETINGURL",
                Some(event),
            ) => event.call = event.call.take().or_else(|| call(value).map(Arc::from)),
            ("DTSTART", Some(event)) => event.start = moment(&params, value),
            ("DTEND", Some(event)) => event.end = moment(&params, value),
            ("DURATION", Some(event)) => event.duration = duration(value),
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

use std::collections::{BTreeMap, HashMap};
mod ics;
#[cfg(test)]
mod tests;
mod zones;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use apolline_config::Calendar;
use chrono::{
    DateTime, Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeDelta,
    TimeZone, Utc, Weekday,
};
use iced::Subscription;
use iced::futures::{SinkExt, Stream};

use crate::shell::Message;
use ics::*;
// CalDAV's tests read calendars the same way.
#[cfg(test)]
pub use ics::agenda;
use zones::*;

const EVERY: Duration = Duration::from_secs(10 * 60);
const RETRY: Duration = Duration::from_secs(60);
// How far from today occurrences are worked out, both ways.
pub const REACH: i64 = 400;
// Limits against a calendar that would eat the machine.
const MOST: usize = 60_000;
const PER_DAY: usize = 60;
const STEPS: usize = 200_000;
const LONGEST: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occurrence {
    // None on a whole day, and on the days a long event carries over into.
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
    pub summary: Arc<str>,
    pub location: Option<Arc<str>>,
    // Where its video call is joined.
    pub call: Option<Arc<str>>,
    // When a timed event begins and ends, in local time.
    span: Option<(NaiveDateTime, NaiveDateTime)>,
    // How long before it starts each of its own alarms goes off.
    alarms: Vec<TimeDelta>,
}

// A meeting with a call and no alarm of its own is still worth a word this long before.
const MEETING_LEAD: TimeDelta = TimeDelta::minutes(5);
// Alarms further ahead than this are the calendar's business, not a card's.
const LONGEST_LEAD: TimeDelta = TimeDelta::hours(2);

impl Occurrence {
    pub fn begins(&self) -> Option<NaiveDateTime> {
        self.span.map(|(begins, _)| begins)
    }
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

    // What to remind about, with the alarm that's due: its own alarms each in turn, or a few
    // minutes before a meeting with a call. A minute's grace after the start lets an alarm at the
    // start itself be seen by a clock that ticks once a minute.
    pub fn due(&self, now: NaiveDateTime) -> impl Iterator<Item = (&Occurrence, TimeDelta)> {
        let tomorrow = now.date().succ_opt().unwrap_or(now.date());
        self.on(now.date())
            .iter()
            .chain(self.on(tomorrow))
            .filter_map(move |o| {
                let begins = o.begins()?;
                let fallback = (o.alarms.is_empty() && o.call.is_some()).then_some(MEETING_LEAD);
                let latest = o
                    .alarms
                    .iter()
                    .copied()
                    .chain(fallback)
                    .filter(|lead| begins - *lead <= now)
                    .min()?;
                (now < begins + TimeDelta::minutes(1)).then_some((o, latest))
            })
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

    // Capped per day as well as in all, so a crowded feed can't crowd out today.
    fn add(&mut self, day: NaiveDate, occurrence: Occurrence) {
        let list = self.days.entry(day).or_default();
        if self.count < MOST && list.len() < PER_DAY {
            self.count += 1;
            list.push(occurrence);
        }
    }

    fn finish(mut self) -> Self {
        self.days.retain(|_, list| !list.is_empty());
        for list in self.days.values_mut() {
            list.sort();
            list.dedup();
        }
        self
    }
}

pub fn subscription(sources: &[Calendar]) -> Subscription<Message> {
    if sources.is_empty() {
        return Subscription::none();
    }
    Subscription::run_with(sources.to_vec(), watch)
}

#[expect(
    clippy::ptr_arg,
    reason = "Subscription::run_with hands the data over as &D"
)]
fn watch(sources: &Vec<Calendar>) -> impl Stream<Item = Message> + use<> {
    let sources = sources.clone();
    iced::stream::channel(1, async move |mut output| {
        // A source that can't be read keeps what it said last time, and only it is asked
        // again soon, waiting longer after each failure.
        let mut texts: HashMap<Calendar, String> = HashMap::new();
        let mut tags: HashMap<Calendar, Tag> = HashMap::new();
        let mut heard: HashMap<Calendar, Heard> = HashMap::new();
        let mut built: Option<NaiveDate> = None;
        let start = Instant::now();
        let mut due: HashMap<Calendar, (Instant, Duration)> = sources
            .iter()
            .map(|source| (source.clone(), (start, RETRY)))
            .collect();
        loop {
            let now = Instant::now();
            let ready: Vec<Calendar> = due
                .iter()
                .filter(|(_, (at, _))| *at <= now)
                .map(|(source, _)| source.clone())
                .collect();
            if !ready.is_empty() {
                // Pushed back first, so even a reader that dies can't spin this loop.
                for source in &ready {
                    if let Some(next) = due.get_mut(source) {
                        next.0 = now + next.1;
                    }
                }
                // All at once, and none longer than a minute: a keyring that asks or a server that
                // hangs mustn't hold up the rest.
                let fetched = iced::futures::future::join_all(ready.into_iter().map(|source| {
                    let tag = tags.get(&source).cloned().unwrap_or_default();
                    async {
                        let fetch = fetch(source.clone(), tag);
                        let result = tokio::time::timeout(Duration::from_secs(60), fetch)
                            .await
                            .unwrap_or_else(|_| Err("it didn't answer within a minute".into()));
                        (source, result)
                    }
                }))
                .await;
                let mut changed = false;
                for (source, result) in fetched {
                    let Some(next) = due.get_mut(&source) else {
                        continue;
                    };
                    let state = heard.entry(source.clone()).or_default();
                    match result {
                        Ok(fetched) => {
                            if let Fetched::New(text, tag) = fetched {
                                changed |= texts.get(&source) != Some(&text);
                                texts.insert(source.clone(), text);
                                tags.insert(source, tag);
                            }
                            *next = (Instant::now() + EVERY, RETRY);
                            *state = Heard {
                                read: Some(Utc::now().timestamp()),
                                error: None,
                            };
                        }
                        Err(err) => {
                            eprintln!("apolline: calendar {}: {err}", name(&source));
                            next.1 = (next.1 * 2).min(EVERY);
                            state.error = Some(err);
                        }
                    }
                }
                tell(&heard);
                // Worked out and sent only when a calendar says something new, or a day passed:
                // a new agenda redraws every window.
                let today = Local::now().date_naive();
                if changed || built != Some(today) {
                    built = Some(today);
                    let all: Vec<String> = texts.values().cloned().collect();
                    let agenda = tokio::task::spawn_blocking(move || agenda(&all, today)).await;
                    if let Ok(agenda) = agenda
                        && output
                            .send(Message::Agenda(Arc::new(agenda)))
                            .await
                            .is_err()
                    {
                        return;
                    }
                }
            }
            let next = due.values().map(|(at, _)| *at).min().unwrap_or(now + EVERY);
            tokio::time::sleep_until(tokio::time::Instant::from_std(next)).await;
        }
    })
}

// What a server said of the calendar it last sent, to ask next time whether it changed since.
#[derive(Clone, Debug, Default, PartialEq)]
struct Tag {
    etag: Option<String>,
    modified: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Fetched {
    Same,
    New(String, Tag),
}

async fn fetch(source: Calendar, tag: Tag) -> Result<Fetched, String> {
    match source {
        Calendar::Ics(link) => tokio::task::spawn_blocking(move || read(&link, &tag))
            .await
            .map_err(|err| err.to_string())?,
        Calendar::CalDav { url, user } => crate::caldav::read(url, user)
            .await
            .map(|text| Fetched::New(text, Tag::default())),
    }
}

// What the watcher last heard from a source, for Settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heard {
    // Seconds since 1970.
    pub read: Option<i64>,
    pub error: Option<String>,
}

// Sources go by a hash in this file: a calendar's secret link shouldn't sit where others may read.
pub fn key(source: &Calendar) -> u64 {
    let text = match source {
        Calendar::Ics(link) => link.clone(),
        Calendar::CalDav { url, user } => format!("{user} {url}"),
    };
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn heard_path() -> PathBuf {
    crate::control::runtime_dir().join("apolline.calendars")
}

fn tell(heard: &HashMap<Calendar, Heard>) {
    let text: String = heard
        .iter()
        .map(|(source, heard)| {
            let error = heard.error.as_deref().unwrap_or_default();
            format!(
                "{:016x} {} {}\n",
                key(source),
                heard.read.unwrap_or(0),
                error.replace('\n', " ")
            )
        })
        .collect();
    let path = heard_path();
    let staged = path.with_extension("calendars.new");
    if std::fs::write(&staged, text)
        .and_then(|()| std::fs::rename(&staged, &path))
        .is_err()
    {
        eprintln!("apolline: cannot write {}", path.display());
    }
}

pub fn heard() -> HashMap<u64, Heard> {
    let text = std::fs::read_to_string(heard_path()).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let key = u64::from_str_radix(parts.next()?, 16).ok()?;
            let read = parts.next()?.parse().ok().filter(|at| *at > 0);
            let error = parts.next().filter(|e| !e.is_empty()).map(str::to_owned);
            Some((key, Heard { read, error }))
        })
        .collect()
}

// "~/home.ics", "calendar.google.com", or "work · me at dav.example.org".
pub fn name(source: &Calendar) -> String {
    match source {
        Calendar::Ics(link) => shown(link).to_owned(),
        Calendar::CalDav { url, user } => {
            let host = shown(url);
            match url.trim_end_matches('/').rsplit('/').next() {
                Some(collection) if collection != host && !collection.is_empty() => {
                    format!("{collection} · {user} at {host}")
                }
                _ => format!("{user} at {host}"),
            }
        }
    }
}

// Where video calls are joined: a host (or one under it) and how the path starts.
const CALLS: [(&str, &str); 9] = [
    ("meet.google.com", "/"),
    ("zoom.us", "/j/"),
    ("zoom.us", "/my/"),
    ("zoom.us", "/w/"),
    ("teams.microsoft.com", "/l/meetup-join"),
    ("teams.live.com", "/meet"),
    ("meet.jit.si", "/"),
    ("webex.com", "/"),
    ("whereby.com", "/"),
];

// The first video call link in some calendar text, escaped or not.
fn call(text: &str) -> Option<String> {
    text.match_indices("https://").find_map(|(at, _)| {
        let link: String = text[at..]
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | '\\' | ')'))
            .collect();
        let rest = &link["https://".len()..];
        // The host ends at the first of these, so "evil.example?.zoom.us" isn't Zoom.
        let (authority, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
        if authority.contains('@') {
            return None;
        }
        let host = authority
            .split(':')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        CALLS
            .iter()
            .any(|(service, start)| {
                (host == *service || host.ends_with(&format!(".{service}")))
                    && path.starts_with(start)
                    && path.len() > 1
            })
            .then_some(link)
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
fn read(source: &str, tag: &Tag) -> Result<Fetched, String> {
    let link = source
        .strip_prefix("webcal://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| source.to_owned());
    let (bytes, tag) = if link.starts_with("https://") {
        match download(&link, tag)? {
            Some(got) => got,
            None => return Ok(Fetched::Same),
        }
    } else if link.starts_with("http://") {
        return Err("only https links are read".into());
    } else {
        let path = match link.strip_prefix("~/") {
            Some(rest) => std::env::var_os("HOME")
                .map(|home| std::path::PathBuf::from(home).join(rest))
                .ok_or("no home folder")?,
            None => std::path::PathBuf::from(&link),
        };
        let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
        (bytes, Tag::default())
    };
    let text = unfold_bytes(&bytes);
    // A sign-in page from a café's Wi-Fi is no calendar, and shouldn't replace the last one.
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("this isn't a calendar".into());
    }
    Ok(Fetched::New(text, tag))
}

// The calendar, unless the server says it hasn't changed since it was last sent: most feeds
// change far less often than every ten minutes, and some are megabytes.
fn download(link: &str, tag: &Tag) -> Result<Option<(Vec<u8>, Tag)>, String> {
    let mut request = ureq::get(link)
        .config()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build();
    if let Some(etag) = &tag.etag {
        request = request.header("If-None-Match", etag);
    }
    if let Some(modified) = &tag.modified {
        request = request.header("If-Modified-Since", modified);
    }
    let mut response = request.call().map_err(|err| err.to_string())?;
    match response.status().as_u16() {
        304 => return Ok(None),
        200 => {}
        status => return Err(format!("the server said {status}")),
    }
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let tag = Tag {
        etag: header("etag"),
        modified: header("last-modified"),
    };
    let bytes = response
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_vec()
        .map_err(|err| err.to_string())?;
    Ok(Some((bytes, tag)))
}

// Folds are undone before the bytes become text: a fold may split a letter in two.
pub fn unfold_bytes(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match &bytes[at..] {
            [b'\r', b'\n', b' ' | b'\t', ..] => at += 3,
            [b'\n', b' ' | b'\t', ..] => at += 2,
            [byte, ..] => {
                out.push(*byte);
                at += 1;
            }
            [] => break,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

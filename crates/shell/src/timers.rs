// Countdowns started from the launcher or `apolline timer`, on the bar until they ring.

use std::time::{Duration, Instant, SystemTime};

use crate::notify::{Heard, Notification, Urgency};

const LONGEST: Duration = Duration::from_secs(24 * 3600);
const LABEL: usize = 40;
const RINGS: u32 = 100;
pub const TIMER_CARD: u32 = crate::notify::REMINDER_CARD + 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    pub label: Option<String>,
    pub length: Duration,
    pub ends: Instant,
}

impl Timer {
    pub fn start(length: Duration, label: Option<String>) -> Self {
        Self {
            label,
            length,
            ends: Instant::now() + length,
        }
    }

    pub fn left(&self) -> Duration {
        self.ends.saturating_duration_since(Instant::now())
    }

    pub fn card(&self, count: u32) -> Notification {
        Notification {
            id: TIMER_CARD + count % RINGS,
            app: crate::i18n::t("Timer").to_owned(),
            desktop: None,
            actions: Vec::new(),
            image: None,
            progress: None,
            transient: false,
            resident: false,
            reply: None,
            summary: self
                .label
                .clone()
                .unwrap_or_else(|| crate::i18n::t("Time's up").to_owned()),
            body: crate::i18n::fill("{length} went by", &[("length", &length(self.length))]),
            // You set it yourself, so it comes through do not disturb, as an alarm does.
            urgency: Urgency::Critical,
            timeout: None,
            received: SystemTime::now(),
            sound: Heard::Quiet,
        }
    }
}

// "timer 25m tea", "timer 1h30m", "таймер 10 мин чай": how long, and what for.
pub fn parse(text: &str) -> Option<(Duration, Option<String>)> {
    let mut words = text.split_whitespace();
    let first = words.next()?.to_lowercase();
    if first != "timer" && first != "таймер" {
        return None;
    }
    let words: Vec<&str> = words.collect();
    let mut total = Duration::ZERO;
    let mut used = 0;
    while used < words.len() {
        let next = words.get(used + 1).copied();
        match part(words[used], next) {
            Some((length, took)) => {
                total += length;
                used += took;
            }
            None => break,
        }
    }
    if total.is_zero() || total > LONGEST {
        return None;
    }
    let label = words[used..].join(" ");
    let label: String = label.chars().take(LABEL).collect();
    Some((total, (!label.is_empty()).then_some(label)))
}

// One piece of a length: "25m", "1h30m", "90" (minutes), or "10" followed by "min".
fn part(word: &str, next: Option<&str>) -> Option<(Duration, usize)> {
    let word = word.to_lowercase();
    if let Ok(number) = word.parse::<u64>() {
        return match next.and_then(|n| unit(&n.to_lowercase())) {
            Some(unit) => Some((Duration::from_secs(number * unit), 2)),
            None => Some((Duration::from_secs(number * 60), 1)),
        };
    }
    let mut total = 0;
    let mut rest = word.as_str();
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let number: u64 = rest[..digits].parse().ok()?;
        let after = &rest[digits..];
        let ends = after
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(after.len());
        total += number * unit(&after[..ends])?;
        rest = &after[ends..];
    }
    Some((Duration::from_secs(total), 1))
}

fn unit(word: &str) -> Option<u64> {
    let word = word.trim_end_matches('.');
    match word {
        "s" | "sec" | "secs" | "second" | "seconds" | "с" | "сек" | "секунд" | "секунды"
        | "секунда" => Some(1),
        "m" | "min" | "mins" | "minute" | "minutes" | "м" | "мин" | "минут" | "минуты"
        | "минута" | "минуту" => Some(60),
        "h" | "hr" | "hrs" | "hour" | "hours" | "ч" | "час" | "часа" | "часов" => {
            Some(3600)
        }
        _ => None,
    }
}

// "1 h 30 min", "10 min", "45 s".
pub fn length(length: Duration) -> String {
    let secs = length.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(crate::i18n::fill("{n} h", &[("n", &h)]));
    }
    if m > 0 {
        parts.push(crate::i18n::fill("{n} min", &[("n", &m)]));
    }
    if s > 0 || parts.is_empty() {
        parts.push(crate::i18n::fill("{n} s", &[("n", &s)]));
    }
    parts.join(" ")
}

// What's left, as a clock: 9:59, or 1:05:00.
pub fn left(left: Duration) -> String {
    let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
    match secs / 3600 {
        0 => format!("{}:{:02}", secs / 60, secs % 60),
        h => format!("{h}:{:02}:{:02}", secs / 60 % 60, secs % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(text: &str) -> Option<(u64, Option<String>)> {
        parse(text).map(|(length, label)| (length.as_secs(), label))
    }

    #[test]
    fn lengths_read_in_either_language_with_what_follows_as_the_label() {
        assert_eq!(secs("timer 25m tea"), Some((1500, Some("tea".into()))));
        assert_eq!(secs("Timer 1h30m"), Some((5400, None)));
        assert_eq!(secs("timer 1h 30m"), Some((5400, None)));
        assert_eq!(secs("таймер 10 мин чай"), Some((600, Some("чай".into()))));
        assert_eq!(secs("timer 5"), Some((300, None)));
        assert_eq!(secs("timer 90 s pasta"), Some((90, Some("pasta".into()))));
        assert_eq!(secs("timer tea"), None);
        assert_eq!(secs("timer 25 hours"), None);
        assert_eq!(secs("timers 5m"), None);
        assert_eq!(secs("timer 10x"), None);
    }

    #[test]
    fn what_is_left_reads_as_a_clock() {
        assert_eq!(left(Duration::from_millis(599_400)), "10:00");
        assert_eq!(left(Duration::from_secs(3900)), "1:05:00");
        assert_eq!(length(Duration::from_secs(5400)), "1 h 30 min");
        assert_eq!(length(Duration::from_secs(45)), "45 s");
    }
}

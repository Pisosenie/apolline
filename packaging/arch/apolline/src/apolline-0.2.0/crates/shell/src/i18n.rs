use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

// Keys are the English text as written in the code, so anything missing stays English.
const RUSSIAN: &str = include_str!("../../../assets/locales/ru.toml");

enum Entry {
    Text(&'static str),
    // One, few and many: 1 файл, 2 файла, 5 файлов.
    Count([&'static str; 3]),
}

#[cfg(not(test))]
static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(test))]
fn speak(russian: bool) {
    ON.store(russian, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(test))]
pub fn russian() -> bool {
    ON.load(std::sync::atomic::Ordering::Relaxed)
}

// Each test thread speaks its own language, English unless it says otherwise.
#[cfg(test)]
thread_local! {
    static ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn speak(russian: bool) {
    ON.set(russian);
}

#[cfg(test)]
pub fn russian() -> bool {
    ON.get()
}

fn table() -> &'static HashMap<String, Entry> {
    static TABLE: OnceLock<HashMap<String, Entry>> = OnceLock::new();
    TABLE.get_or_init(|| parse(RUSSIAN))
}

fn parse(text: &str) -> HashMap<String, Entry> {
    let keep = |s: &str| -> &'static str { Box::leak(s.to_owned().into_boxed_str()) };
    let Ok(table) = text.parse::<toml::Table>() else {
        eprintln!("apolline: the Russian translation doesn't parse");
        return HashMap::new();
    };
    table
        .into_iter()
        .filter_map(|(english, value)| {
            let entry = match value {
                toml::Value::String(s) => Entry::Text(keep(&s)),
                toml::Value::Array(forms) => {
                    let forms: Vec<&'static str> =
                        forms.iter().filter_map(|f| f.as_str()).map(keep).collect();
                    Entry::Count(forms.try_into().ok()?)
                }
                _ => return None,
            };
            Some((english, entry))
        })
        .collect()
}

// `language` from the config: "ru", "en", or "auto" and nothing for what the system speaks.
pub fn choose(language: Option<&str>) {
    let system = || {
        // Tests don't take the language of the machine they run on.
        if cfg!(test) {
            return String::new();
        }
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|v| v.to_string_lossy().into_owned())
            .find(|v| !v.is_empty())
            .unwrap_or_default()
    };
    let wanted = match language {
        Some(code) if code != "auto" => code.to_owned(),
        _ => system(),
    };
    speak(wanted.starts_with("ru"));
}

pub fn find(english: &str) -> Option<&'static str> {
    if !russian() {
        return None;
    }
    match table().get(english)? {
        Entry::Text(text) => Some(text),
        Entry::Count(_) => None,
    }
}

pub fn t(english: &'static str) -> &'static str {
    find(english).unwrap_or(english)
}

// A line with {names} in it, translated before they're filled in.
pub fn fill(template: &'static str, values: &[(&str, &dyn Display)]) -> String {
    values
        .iter()
        .fold(t(template).to_owned(), |line, (name, value)| {
            line.replace(&format!("{{{name}}}"), &value.to_string())
        })
}

// "1 result" or "{n} results", and in Russian whichever of its three forms fits n.
pub fn count(n: usize, one: &'static str, many: &'static str) -> String {
    let form = match (russian(), table().get(many)) {
        (true, Some(Entry::Count(forms))) => forms[russian_form(n)],
        _ if n == 1 => one,
        _ => many,
    };
    form.replace("{n}", &n.to_string())
}

const DAYS: [&str; 7] = [
    "понедельник",
    "вторник",
    "среда",
    "четверг",
    "пятница",
    "суббота",
    "воскресенье",
];
const SHORT_DAYS: [&str; 7] = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];
const MONTHS: [&str; 12] = [
    "январь",
    "февраль",
    "март",
    "апрель",
    "май",
    "июнь",
    "июль",
    "август",
    "сентябрь",
    "октябрь",
    "ноябрь",
    "декабрь",
];
// After a day's number: 4 ноября.
const OF_MONTHS: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
const SHORT_MONTHS: [&str; 12] = [
    "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
];

// A strftime pattern with day and month names already in place, for `format` to finish.
pub fn names(pattern: &str, day: &impl chrono::Datelike) -> String {
    if !russian() {
        return pattern.to_owned();
    }
    let weekday = day.weekday().num_days_from_monday() as usize;
    let month = day.month0() as usize;
    let capital = |word: &str, first: bool| -> String {
        if !first {
            return word.to_owned();
        }
        let mut letters = word.chars();
        letters
            .next()
            .map(|c| c.to_uppercase().chain(letters).collect())
            .unwrap_or_default()
    };
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(at) = rest.find('%') {
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        let first = out.trim().is_empty();
        let (word, taken) = match after.as_bytes().get(1) {
            Some(b'A') => (capital(DAYS[weekday], first), 2),
            Some(b'a') => (capital(SHORT_DAYS[weekday], first), 2),
            Some(b'B') if out.ends_with("%-d ") || out.ends_with("%d ") => {
                (OF_MONTHS[month].to_owned(), 2)
            }
            Some(b'B') => (capital(MONTHS[month], first), 2),
            Some(b'b') => (SHORT_MONTHS[month].to_owned(), 2),
            _ => {
                let end = after.char_indices().nth(2).map_or(after.len(), |(i, _)| i);
                (after[..end].to_owned(), end)
            }
        };
        out.push_str(&word);
        rest = &after[taken..];
    }
    out.push_str(rest);
    out
}

fn russian_form(n: usize) -> usize {
    match (n % 10, n % 100) {
        (1, h) if h != 11 => 0,
        (2..=4, h) if !(12..=14).contains(&h) => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_counts_pick_the_right_form() {
        let forms: Vec<usize> = [1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 101, 111]
            .into_iter()
            .map(russian_form)
            .collect();
        assert_eq!(forms, [0, 1, 1, 2, 2, 2, 2, 0, 1, 2, 0, 2]);
    }

    #[test]
    fn the_translation_parses_and_keeps_its_placeholders() {
        let table = parse(RUSSIAN);
        assert!(!table.is_empty());
        for (english, entry) in &table {
            let names: Vec<&str> = english
                .split('{')
                .skip(1)
                .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
                .collect();
            let forms = match entry {
                Entry::Text(text) => vec![*text],
                Entry::Count(forms) => forms.to_vec(),
            };
            for form in forms {
                for name in &names {
                    assert!(
                        form.contains(&format!("{{{name}}}")),
                        "{english:?} lost {{{name}}} in {form:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn days_and_months_take_their_russian_names() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        speak(true);
        let shown = |pattern: &str| day.format(&names(pattern, &day)).to_string();
        assert_eq!(shown("%A, %-d %B"), "Среда, 4 ноября");
        assert_eq!(shown("%B %Y"), "Ноябрь 2026");
        assert_eq!(shown("%a %-d %b"), "Ср 4 ноя");
        assert_eq!(shown("%-d %B"), "4 ноября");
        speak(false);
        assert_eq!(shown("%A, %-d %B"), "Wednesday, 4 November");
    }

    #[test]
    fn english_stays_english_and_counts_still_count() {
        speak(false);
        assert_eq!(t("Do not disturb"), "Do not disturb");
        assert_eq!(count(1, "1 result", "{n} results"), "1 result");
        assert_eq!(count(3, "1 result", "{n} results"), "3 results");
        assert_eq!(
            fill("Pair with {device}?", &[("device", &"WH-1000XM5")]),
            "Pair with WH-1000XM5?"
        );
    }
}

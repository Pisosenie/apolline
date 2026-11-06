// The words of what's playing, from lrclib.net: synced lines, each with the moment it's sung.
// Asked only when [panel] lyrics is on, since it tells lrclib the title and the artist.

use std::time::Duration;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Lyrics {
    lines: Vec<(Duration, String)>,
}

impl Lyrics {
    // The line being sung and the one after it; nothing before the first or in a pause.
    pub fn at(&self, at: Duration) -> Option<(&str, Option<&str>)> {
        let now = self.lines.iter().rposition(|(start, _)| *start <= at)?;
        let line = self.lines[now].1.as_str();
        if line.is_empty() {
            return None;
        }
        let next = self
            .lines
            .get(now + 1)
            .map(|(_, l)| l.as_str())
            .filter(|l| !l.is_empty());
        Some((line, next))
    }
}

// What a track is known by, so the same one isn't asked for twice.
pub fn key(track: &crate::media::Track) -> String {
    format!(
        "{}\n{}",
        track.artist.as_deref().unwrap_or_default(),
        track.title
    )
}

// [mm:ss.xx] lines; a line may carry more than one time, and other tags are skipped.
fn parse(lrc: &str) -> Option<Lyrics> {
    let mut lines = Vec::new();
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut times = Vec::new();
        while let Some(tag) = rest.strip_prefix('[') {
            let Some((stamp, after)) = tag.split_once(']') else {
                break;
            };
            if let Some(time) = stamp_time(stamp) {
                times.push(time);
            }
            rest = after;
        }
        lines.extend(times.into_iter().map(|time| (time, rest.trim().to_owned())));
    }
    lines.sort_by_key(|(time, _)| *time);
    (!lines.is_empty()).then_some(Lyrics { lines })
}

fn stamp_time(stamp: &str) -> Option<Duration> {
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes: u64 = minutes.parse().ok()?;
    let seconds: f64 = seconds.parse().ok()?;
    Some(Duration::from_secs(minutes * 60) + Duration::from_secs_f64(seconds))
}

// The first synced lyrics lrclib has for the track; None when it has none or can't be reached.
pub async fn find(artist: Option<String>, title: String) -> Option<Lyrics> {
    tokio::task::spawn_blocking(move || {
        let mut request = ureq::get("https://lrclib.net/api/search").query("track_name", &title);
        if let Some(artist) = &artist {
            request = request.query("artist_name", artist);
        }
        let body = request
            .header(
                "User-Agent",
                concat!("Apolline/", env!("CARGO_PKG_VERSION")),
            )
            .config()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .call()
            .ok()?
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .ok()?;
        let Ok(Value::Array(hits)) = serde_json::from_str(&body) else {
            return None;
        };
        hits.iter()
            .find_map(|hit| hit["syncedLyrics"].as_str())
            .and_then(parse)
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_come_in_time_and_pauses_are_quiet() {
        let lyrics = parse("[ar:Someone]\n[00:01.50]First\n[00:04.00][01:00.00]Again\n[00:06.25]\n[00:08.00]Last\n").unwrap();
        let at = |secs: f64| lyrics.at(Duration::from_secs_f64(secs));
        assert_eq!(at(0.5), None, "before the first line");
        assert_eq!(at(2.0), Some(("First", Some("Again"))));
        assert_eq!(at(7.0), None, "a pause");
        assert_eq!(at(9.0), Some(("Last", Some("Again"))));
        assert_eq!(at(61.0), Some(("Again", None)), "a line sung twice");
        assert!(parse("no times here").is_none());
    }
}

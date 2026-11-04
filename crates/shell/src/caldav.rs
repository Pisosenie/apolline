use std::time::Duration;

use apolline_config::Calendar;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use chrono::{Local, NaiveDate, TimeDelta};
use ureq::http;

use crate::events::{REACH, shown, unfold_bytes};

const LARGEST: u64 = 16 * 1024 * 1024;

// What Settings does to add one: a read to check the address and the password, then the
// password goes to the keyring. Nothing is kept if either fails.
pub async fn add(url: String, user: String, password: String) -> Result<Calendar, String> {
    let check = (url.clone(), user.clone(), password.clone());
    let today = Local::now().date_naive();
    tokio::task::spawn_blocking(move || fetch(&check.0, &check.1, &check.2, today))
        .await
        .map_err(|err| err.to_string())??;
    crate::secrets::store(&url, &user, &password).await?;
    Ok(Calendar::CalDav { url, user })
}

pub async fn read(url: String, user: String) -> Result<String, String> {
    secure(&url)?;
    let password = crate::secrets::find(&url, &user)
        .await?
        .ok_or("The keyring has no password for it; remove it and add it again")?;
    let today = Local::now().date_naive();
    tokio::task::spawn_blocking(move || fetch(&url, &user, &password, today))
        .await
        .map_err(|err| err.to_string())?
}

// The events around today as VCALENDAR text, the way an ICS file would give them.
fn fetch(url: &str, user: &str, password: &str, today: NaiveDate) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .allow_non_standard_methods(true)
        // A redirect would either drop the password or carry it somewhere else.
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut response = agent
        .run(request(url, user, password, today)?)
        .map_err(|err| {
            crate::i18n::fill(
                "Couldn't reach {url}: {err}",
                &[("url", &shown(url)), ("err", &err)],
            )
        })?;
    let status = response.status().as_u16();
    if !matches!(status, 200 | 207) {
        return Err(refused(status));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(LARGEST)
        .read_to_vec()
        .map_err(|err| err.to_string())?;
    calendar(&body)
}

// Basic auth sends the password as it is, so it goes over https, or plain http to this machine.
pub fn secure(url: &str) -> Result<(), String> {
    let (scheme, rest) = url.split_once("://").ok_or("This isn't a web address")?;
    let place = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if place.contains('@') {
        return Err("The user name goes in its own field, not in the address".into());
    }
    let host = match place.strip_prefix('[') {
        Some(v6) => v6.split(']').next(),
        None => place.split(':').next(),
    };
    match (scheme.to_ascii_lowercase().as_str(), host) {
        ("https", _) | ("http", Some("localhost" | "127.0.0.1" | "::1")) => Ok(()),
        _ => Err("CalDAV is read over https only".into()),
    }
}

// A calendar-query for every event that touches the days the agenda works out.
fn request(
    url: &str,
    user: &str,
    password: &str,
    today: NaiveDate,
) -> Result<http::Request<String>, String> {
    secure(url)?;
    let day = |offset: i64| (today + TimeDelta::days(offset)).format("%Y%m%dT000000Z");
    let query = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop><C:calendar-data/></D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="{}" end="{}"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>
"#,
        day(-REACH),
        day(REACH + 1)
    );
    let login = BASE64_STANDARD.encode(format!("{user}:{password}"));
    http::Request::builder()
        .method("REPORT")
        .uri(url)
        .header("Authorization", format!("Basic {login}"))
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(query)
        .map_err(|_| "This isn't a web address".into())
}

fn refused(status: u16) -> String {
    match status {
        401 => "The server turned down the user name or password".into(),
        403 => "This account may not read that calendar".into(),
        404 => "There's no calendar at this address".into(),
        300..=399 => "This address points elsewhere; use the calendar's own address".into(),
        _ => crate::i18n::fill("The server answered {status}", &[("status", &status)]),
    }
}

// Every calendar-data in a multistatus, one after another; the parser doesn't mind.
fn calendar(xml: &[u8]) -> Result<String, String> {
    if elements(xml, b"multistatus").is_empty() {
        return Err("The server's answer isn't a calendar".into());
    }
    Ok(elements(xml, b"calendar-data")
        .iter()
        .filter(|data| !data.is_empty())
        .map(|data| unfold_bytes(data))
        .collect::<Vec<_>>()
        .join("\r\n"))
}

// The text of each element with this local name, whatever prefix its namespace goes by.
// Bytes, not text: a fold may split a letter, and it's undone only once entities are.
fn elements(xml: &[u8], name: &[u8]) -> Vec<Vec<u8>> {
    const SKIPPED: [(&[u8], &[u8]); 3] =
        [(b"<!--", b"-->"), (b"<![CDATA[", b"]]>"), (b"<?", b"?>")];
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(open) = find(xml, at, b"<") {
        let rest = &xml[open..];
        if let Some((_, end)) = SKIPPED.iter().find(|(start, _)| rest.starts_with(start)) {
            at = find(xml, open, end).map_or(xml.len(), |e| e + end.len());
            continue;
        }
        let Some(close) = tag_end(xml, open) else {
            break;
        };
        let tag = &xml[open + 1..close];
        at = close + 1;
        if local(tag) != name {
            continue;
        }
        if tag.ends_with(b"/") {
            found.push(Vec::new());
            continue;
        }
        let (text, next) = text(xml, at);
        found.push(text);
        at = next;
    }
    found
}

// "C:calendar-data content-type=…" is calendar-data; a closing tag has no name here.
fn local(tag: &[u8]) -> &[u8] {
    let name = tag
        .split(|b| b.is_ascii_whitespace() || *b == b'/')
        .next()
        .unwrap_or_default();
    name.rsplit(|b| *b == b':').next().unwrap_or_default()
}

fn tag_end(xml: &[u8], open: usize) -> Option<usize> {
    let mut quote = None;
    (open..xml.len()).find(|&at| {
        match (quote, xml[at]) {
            (None, b'"' | b'\'') => quote = Some(xml[at]),
            (Some(q), b) if q == b => quote = None,
            (None, b'>') => return true,
            _ => {}
        }
        false
    })
}

// Character data up to the next tag, with CDATA taken as it is.
fn text(xml: &[u8], mut at: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    while let Some(rest) = xml.get(at..).filter(|rest| !rest.is_empty()) {
        if let Some(inner) = rest.strip_prefix(b"<![CDATA[") {
            let end = find(inner, 0, b"]]>").unwrap_or(inner.len());
            out.extend_from_slice(&inner[..end]);
            at += b"<![CDATA[".len() + end + b"]]>".len();
        } else if rest[0] == b'<' {
            break;
        } else if let Some((c, length)) = reference(rest) {
            out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            at += length;
        } else {
            out.push(rest[0]);
            at += 1;
        }
    }
    (out, at)
}

// "&lt;", "&#13;", "&#x0D;": the character, and how many bytes it took.
fn reference(rest: &[u8]) -> Option<(char, usize)> {
    let rest = rest.strip_prefix(b"&")?;
    let end = rest.iter().take(10).position(|b| *b == b';')?;
    let name = std::str::from_utf8(&rest[..end]).ok()?;
    let c = match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let code = match name.strip_prefix("#x") {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => name.strip_prefix('#')?.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some((c, end + 2))
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn calendar_data_comes_out_whatever_its_prefix() {
        let nextcloud = br#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:cal="urn:ietf:params:xml:ns:caldav">
 <d:response><d:href>/remote.php/dav/calendars/me/home/a.ics</d:href>
  <d:propstat><d:prop><cal:calendar-data>BEGIN:VCALENDAR&#13;
BEGIN:VEVENT&#13;
SUMMARY:Tom &amp; Jerry &lt;3&#13;
END:VEVENT&#13;
END:VCALENDAR&#13;
</cal:calendar-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"#;
        assert_eq!(
            calendar(nextcloud).unwrap(),
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Tom & Jerry <3\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        );

        let apple = br#"<multistatus xmlns="DAV:">
<!-- <calendar-data>not this</calendar-data> -->
<response><propstat><prop>
<calendar-data xmlns="urn:ietf:params:xml:ns:caldav" content-type='text/calendar' note="a > b"><![CDATA[BEGIN:VCALENDAR
SUMMARY:<b>bold</b> & raw
END:VCALENDAR]]></calendar-data>
</prop></propstat></response>
<response><propstat><prop><C:calendar-data xmlns:C="urn:ietf:params:xml:ns:caldav"/></prop>
<status>HTTP/1.1 404 Not Found</status></propstat></response>
</multistatus>"#;
        assert_eq!(
            calendar(apple).unwrap(),
            "BEGIN:VCALENDAR\nSUMMARY:<b>bold</b> & raw\nEND:VCALENDAR"
        );

        let none = br#"<D:multistatus xmlns:D="DAV:"/>"#;
        assert_eq!(calendar(none).unwrap(), "", "no events is still a calendar");
        assert!(calendar(b"<html>Sign in to the Wi-Fi</html>").is_err());
        assert!(calendar(b"<C:calendar-data>cut short").is_err());
    }

    #[test]
    fn a_fold_through_a_letter_comes_back_whole() {
        let event = "SUMMARY:Встреча".as_bytes();
        let cut = event.len() - 1;
        let xml = [
            b"<multistatus><calendar-data>BEGIN:VCALENDAR&#13;\n".as_slice(),
            &event[..cut],
            b"&#13;\n ",
            &event[cut..],
            b"&#xD;\nEND:VCALENDAR</calendar-data></multistatus>",
        ]
        .concat();
        assert!(calendar(&xml).unwrap().contains("SUMMARY:Встреча\r\n"));
    }

    #[test]
    fn what_a_server_sends_lands_in_the_agenda() {
        let xml = br#"<multistatus xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
<response><propstat><prop><C:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:standup
SUMMARY:Standup
DTSTART:20261102T100000
DTEND:20261102T101500
RRULE:FREQ=DAILY;COUNT=3
END:VEVENT
END:VCALENDAR
</C:calendar-data></prop></propstat></response>
<response><propstat><prop><C:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:review
SUMMARY:Review
DTSTART;VALUE=DATE:20261103
END:VEVENT
END:VCALENDAR
</C:calendar-data></prop></propstat></response>
</multistatus>"#;
        let agenda = crate::events::agenda(&[calendar(xml).unwrap()], day(2026, 11, 2));
        let titles =
            |d| -> Vec<String> { agenda.on(d).iter().map(|o| o.summary.to_string()).collect() };
        assert_eq!(titles(day(2026, 11, 2)), ["Standup"]);
        assert_eq!(titles(day(2026, 11, 3)), ["Review", "Standup"]);
        assert_eq!(titles(day(2026, 11, 4)), ["Standup"]);
        assert!(titles(day(2026, 11, 5)).is_empty());
    }

    #[test]
    fn the_request_asks_for_the_agenda_s_reach() {
        let request = request(
            "https://dav.example.org/calendars/me/home/",
            "Aladdin",
            "open sesame",
            day(2026, 11, 2),
        )
        .unwrap();
        assert_eq!(request.method().as_str(), "REPORT");
        let header = |name| request.headers()[name].to_str().unwrap();
        assert_eq!(
            header("Authorization"),
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert_eq!(header("Depth"), "1");
        assert!(header("Content-Type").starts_with("application/xml"));
        let body = request.body();
        assert!(
            body.contains(r#"<C:time-range start="20250928T000000Z" end="20271208T000000Z"/>"#),
            "{body}"
        );
        assert!(body.contains(r#"<C:comp-filter name="VEVENT">"#));
        assert!(body.contains("<C:calendar-data/>"));
        assert_eq!(elements(body.as_bytes(), b"calendar-query").len(), 1);
    }

    #[test]
    fn passwords_go_over_https_or_stay_on_this_machine() {
        for good in [
            "https://dav.example.org/me/",
            "HTTPS://dav.example.org",
            "http://localhost:5232/me/home/",
            "http://127.0.0.1:5232/me/",
            "http://[::1]:5232/me/",
        ] {
            assert_eq!(secure(good), Ok(()), "{good}");
        }
        for bad in [
            "http://dav.example.org/me/",
            "http://localhost.example.org/",
            "http://127.0.0.1.example.org/",
            "ftp://localhost/",
            "dav.example.org/me/",
        ] {
            assert_eq!(
                secure(bad).unwrap_err(),
                if bad.contains("://") {
                    "CalDAV is read over https only"
                } else {
                    "This isn't a web address"
                },
                "{bad}"
            );
        }
        assert!(secure("https://me:hunter2@dav.example.org/").is_err());
        assert!(request("https://dav example/", "me", "x", day(2026, 1, 1)).is_err());
        assert_eq!(
            refused(401),
            "The server turned down the user name or password"
        );
    }

    fn live() -> (String, String, String) {
        let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        (
            var("CALDAV_URL"),
            var("CALDAV_USER"),
            var("CALDAV_PASSWORD"),
        )
    }

    // CALDAV_URL=… CALDAV_USER=… CALDAV_PASSWORD=… cargo test -p apolline-shell live -- --ignored --nocapture
    #[test]
    #[ignore = "needs a CalDAV server"]
    fn live_server() {
        let (url, user, password) = live();
        let today = Local::now().date_naive();
        let text = fetch(&url, &user, &password, today).unwrap();
        let agenda = crate::events::agenda(&[text], today);
        let mut busy = 0;
        for d in today.iter_days().take(14) {
            for o in agenda.on(d) {
                println!("{d} {:?}-{:?} {}", o.start, o.end, o.summary);
                busy += 1;
            }
        }
        assert!(busy > 0, "nothing in the next two weeks");
        assert_eq!(
            fetch(&url, &user, "wrong", today).unwrap_err(),
            refused(401)
        );
    }

    // The same, as Settings adds it and the bar reads it: needs a session bus with a Secret Service.
    #[test]
    #[ignore = "needs a CalDAV server and a keyring"]
    fn live_server_through_the_keyring() {
        let (url, user, password) = live();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            crate::secrets::here().await.unwrap();
            let added = add(url.clone(), user.clone(), password.clone()).await;
            assert_eq!(
                added,
                Ok(Calendar::CalDav {
                    url: url.clone(),
                    user: user.clone()
                })
            );
            let found = crate::secrets::find(&url, &user).await.unwrap();
            assert_eq!(found.as_deref(), Some(password.as_str()));
            let text = read(url.clone(), user.clone()).await.unwrap();
            assert!(text.contains("BEGIN:VEVENT"));
            assert!(
                add(url.clone(), user.clone(), "wrong".into())
                    .await
                    .is_err()
            );
            assert_eq!(
                crate::secrets::find(&url, &user).await.unwrap().as_deref(),
                Some(password.as_str()),
                "a password that didn't work replaces nothing"
            );
            crate::secrets::forget(&url, &user).await.unwrap();
            assert_eq!(crate::secrets::find(&url, &user).await, Ok(None));
            assert!(read(url, user).await.unwrap_err().contains("no password"));
            println!("added, read, and forgotten through the keyring");
        });
    }
}

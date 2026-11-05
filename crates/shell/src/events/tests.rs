use super::*;

#[test]
fn reminders_come_at_the_alarm_or_just_before_a_call() {
    assert_eq!(lead(&[], "-PT15M"), Some(TimeDelta::minutes(15)));
    assert_eq!(lead(&[], "PT0S"), Some(TimeDelta::zero()), "at the start");
    assert_eq!(lead(&[], "PT5M"), None, "after it started");
    assert_eq!(lead(&[], "-P1D"), None, "a day ahead is the calendar's");
    let at = [("VALUE".to_owned(), "DATE-TIME".to_owned())];
    assert_eq!(lead(&at, "20261102T090000Z"), None);

    let ics = "BEGIN:VEVENT\r
UID:dentist\r
SUMMARY:Dentist\r
DTSTART:20261102T100000\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
TRIGGER:-PT30M\r
END:VALARM\r
BEGIN:VALARM\r
TRIGGER:-PT10M\r
END:VALARM\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:review\r
SUMMARY:Design review\r
DTSTART:20261102T110000\r
LOCATION:https://meet.google.com/abc-defg-hij\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:lunch\r
SUMMARY:Lunch\r
DTSTART:20261102T120000\r
END:VEVENT\r
";
    let agenda = agenda(&[ics.to_owned()], day(2026, 11, 2));
    let due = |h, m| -> Vec<(String, i64)> {
        let now = day(2026, 11, 2).and_hms_opt(h, m, 0).unwrap();
        agenda
            .due(now)
            .map(|(o, lead)| (o.summary.to_string(), lead.num_minutes()))
            .collect()
    };
    let one = |what: &str, lead| vec![(what.to_owned(), lead)];
    assert!(due(9, 29).is_empty());
    assert_eq!(due(9, 30), one("Dentist", 30), "each alarm in turn");
    assert_eq!(due(9, 50), one("Dentist", 10));
    assert_eq!(due(10, 0), one("Dentist", 10), "a minute's grace");
    assert!(due(10, 1).is_empty(), "begun");
    assert!(due(10, 54).is_empty());
    assert_eq!(
        due(10, 55),
        one("Design review", 5),
        "five minutes before a call"
    );
    assert!(due(11, 58).is_empty(), "no alarm, no call, no card");
}

#[test]
fn an_alarm_at_the_start_still_rings() {
    let ics = "BEGIN:VEVENT\r
UID:call\r
SUMMARY:Call\r
DTSTART:20261102T100000\r
BEGIN:VALARM\r
TRIGGER:PT0S\r
END:VALARM\r
END:VEVENT\r
";
    let agenda = agenda(&[ics.to_owned()], day(2026, 11, 2));
    let at = |h, m| day(2026, 11, 2).and_hms_opt(h, m, 0).unwrap();
    assert_eq!(agenda.due(at(9, 59)).count(), 0);
    assert_eq!(agenda.due(at(10, 0)).count(), 1);
}

#[test]
fn a_video_call_is_found_where_calendars_put_it() {
    assert_eq!(
        call("https://meet.google.com/abc-defg-hij").as_deref(),
        Some("https://meet.google.com/abc-defg-hij")
    );
    assert_eq!(
        call("Join: https://us02web.zoom.us/j/8812345678?pwd=xyz\\nMeeting ID: 881").as_deref(),
        Some("https://us02web.zoom.us/j/8812345678?pwd=xyz"),
        "ends where the escaped line break begins"
    );
    assert!(
        call("<https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/0?context=y>")
            .is_some_and(|link| link.ends_with("context=y"))
    );
    assert_eq!(call("https://zoom.us/pricing"), None, "not a meeting");
    assert_eq!(call("https://evil.example/meet.google.com/abc"), None);
    assert_eq!(call("https://meet.google.com.evil.example/abc"), None);
    assert_eq!(
        call("https://evil.example?.zoom.us/j/1"),
        None,
        "a query isn't a host"
    );
    assert_eq!(call("https://evil.example#.zoom.us/j/1"), None);
    assert_eq!(
        call("https://zoom.us@evil.example/j/1"),
        None,
        "nor a login"
    );
    assert_eq!(call("http://meet.google.com/abc"), None, "https only");

    let ics = "BEGIN:VEVENT\r
UID:review\r
SUMMARY:Design review\r
DTSTART:20261102T163000\r
LOCATION:https://meet.google.com/abc-defg-hij\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:sync\r
SUMMARY:Sync\r
DTSTART:20261102T170000\r
LOCATION:Room 4\r
DESCRIPTION:Teams meeting\\n________________\\nJoin: https://teams.microsoft.com/l/meetup-join/19%3a\r
 meeting_abc/0\\nMeeting ID: 123\r
END:VEVENT\r
";
    let agenda = agenda(&[ics.to_owned()], day(2026, 11, 2));
    let items = agenda.on(day(2026, 11, 2));
    assert_eq!(
        items[0].location, None,
        "the link is the Join button instead"
    );
    assert_eq!(
        items[0].call.as_deref(),
        Some("https://meet.google.com/abc-defg-hij")
    );
    assert_eq!(items[1].location.as_deref(), Some("Room 4"));
    assert_eq!(
        items[1].call.as_deref(),
        Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc/0")
    );
}

#[test]
fn an_outlook_zone_reads_as_the_zone_it_describes() {
    let ics = "BEGIN:VCALENDAR\r
BEGIN:VTIMEZONE\r
TZID:Customized Time Zone\r
BEGIN:STANDARD\r
DTSTART:16010101T030000\r
TZOFFSETFROM:+0200\r
TZOFFSETTO:+0100\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:16010101T020000\r
TZOFFSETFROM:+0100\r
TZOFFSETTO:+0200\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VTIMEZONE\r
TZID:(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi\r
BEGIN:STANDARD\r
DTSTART:16010101T000000\r
TZOFFSETFROM:+0530\r
TZOFFSETTO:+0530\r
END:STANDARD\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
UID:summer\r
DTSTART;TZID=Customized Time Zone:20260715T100000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:winter\r
DTSTART;TZID=Customized Time Zone:20261215T100000\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:delhi\r
DTSTART;TZID=\"(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi\":20261105T150000\r
END:VEVENT\r
END:VCALENDAR\r
";
    let utc = |uid: &str| {
        parse(ics, 2026)
            .into_iter()
            .find(|e| e.uid == uid)
            .and_then(|e| e.start)
            .map(|start| start.in_zone(Zone::Utc).format("%H:%M").to_string())
    };
    assert_eq!(utc("summer").as_deref(), Some("08:00"), "summer time");
    assert_eq!(utc("winter").as_deref(), Some("09:00"));
    assert_eq!(utc("delhi").as_deref(), Some("09:30"));

    let zone = zones(ics, 2026)[0].1;
    let berlin: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
    for day in day(2026, 1, 1).iter_days().take(365) {
        let noon = day.and_hms_opt(12, 0, 0).unwrap();
        assert_eq!(
            zone.offset_from_utc_datetime(&noon).fix(),
            berlin.offset_from_utc_datetime(&noon).fix(),
            "changes on the same days as Berlin: {day}"
        );
    }
    assert_eq!(utc_offset("-0330"), Some(-12600));
    assert_eq!(utc_offset("+013045"), Some(5445));
    assert_eq!(utc_offset("0100"), None);
}

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
fn what_review_found_in_real_calendars() {
    // A fold that splits a Cyrillic letter in half.
    let bytes = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Встреча\r\nEND:VEVENT\r\n".as_bytes();
    let cut = bytes.windows(2).position(|w| w == "ч".as_bytes()).unwrap() + 1;
    let folded = [&bytes[..cut], b"\r\n ", &bytes[cut..]].concat();
    assert!(unfold_bytes(&folded).contains("SUMMARY:Встреча"));

    // Google and Apple write WKST=SU.
    let days = days_of(
        "DTSTART:20261101T100000\nRRULE:FREQ=WEEKLY;INTERVAL=2;WKST=SU;BYDAY=SU,WE",
        day(2026, 11, 1),
        day(2026, 11, 30),
    );
    assert_eq!(
        days,
        [
            day(2026, 11, 1),
            day(2026, 11, 4),
            day(2026, 11, 15),
            day(2026, 11, 18),
            day(2026, 11, 29)
        ]
    );

    // A length instead of an end.
    let agenda = agenda(
        &[event("DTSTART:20261102T100000\nDURATION:PT1H30M")],
        day(2026, 11, 1),
    );
    assert_eq!(
        agenda.on(day(2026, 11, 2))[0].end,
        NaiveTime::from_hms_opt(11, 30, 0)
    );
    let trip = days_of(
        "DTSTART;VALUE=DATE:20261102\nDURATION:P3D",
        day(2026, 11, 1),
        day(2026, 11, 10),
    );
    assert_eq!(trip.len(), 3);
    assert_eq!(
        duration("P1W2D"),
        TimeDelta::try_days(9),
        "lenient, as calendars are"
    );
    assert_eq!(duration("PT"), Some(TimeDelta::zero()));
    assert_eq!(duration("P5"), None);
    assert_eq!(duration("PT15M"), TimeDelta::try_minutes(15));

    // The RFC's election day: the Tuesday after the first Monday of November.
    let election = days_of(
        "DTSTART;VALUE=DATE:20241105\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=TU;BYMONTHDAY=2,3,4,5,6,7,8",
        day(2026, 11, 1),
        day(2026, 11, 30),
    );
    assert_eq!(election, [day(2026, 11, 3)]);
    let firsts = days_of(
        "DTSTART;VALUE=DATE:20260101\nRRULE:FREQ=YEARLY;BYMONTHDAY=1",
        day(2026, 11, 1),
        day(2026, 12, 31),
    );
    assert_eq!(firsts, [day(2026, 11, 1), day(2026, 12, 1)]);

    // Lowercase is still a rule.
    let lower = days_of(
        "DTSTART:20261102T100000\nRRULE:freq=weekly;byday=mo,we",
        day(2026, 11, 2),
        day(2026, 11, 8),
    );
    assert_eq!(lower, [day(2026, 11, 2), day(2026, 11, 4)]);

    // A time the clocks skip is read as the hour after, not left as it was.
    let gap = Moment {
        at: day(2027, 3, 14).and_hms_opt(2, 30, 0).unwrap(),
        zone: Zone::named("America/New_York"),
        whole_day: false,
    };
    assert_eq!(
        gap.in_zone(Zone::Utc),
        day(2027, 3, 14).and_hms_opt(7, 30, 0).unwrap()
    );
}

#[test]
fn a_page_that_isnt_a_calendar_is_refused() {
    let file = std::env::temp_dir().join(format!("apolline-not-ics-{}", std::process::id()));
    std::fs::write(&file, "<html>Sign in to the Wi-Fi</html>").unwrap();
    assert!(read(file.to_str().unwrap(), &Tag::default()).is_err());
    std::fs::write(&file, b"BEGIN:VCALENDAR\r\nX:\xff\r\nEND:VCALENDAR\r\n").unwrap();
    assert!(
        read(file.to_str().unwrap(), &Tag::default()).is_ok(),
        "a bad byte isn't the end of it"
    );
    std::fs::remove_file(file).unwrap();
}

#[test]
fn a_crowded_day_keeps_room_for_the_rest() {
    let busy: String = (0..200)
        .map(|i| {
            format!(
                "BEGIN:VEVENT\nUID:{i}\nSUMMARY:Item {i}\nDTSTART:20261102T{:02}0000\nEND:VEVENT\n",
                i % 24
            )
        })
        .collect();
    let agenda = agenda(&[busy, event("DTSTART:20261103T100000")], day(2026, 11, 1));
    assert!(agenda.on(day(2026, 11, 2)).len() <= PER_DAY);
    assert_eq!(agenda.on(day(2026, 11, 3)).len(), 1);
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
fn calendars_are_named_without_their_secrets() {
    let work = Calendar::CalDav {
        url: "https://dav.example.org/calendars/me/work/".into(),
        user: "me".into(),
    };
    assert_eq!(name(&work), "work · me at dav.example.org");
    let bare = Calendar::CalDav {
        url: "https://dav.example.org".into(),
        user: "me".into(),
    };
    assert_eq!(name(&bare), "me at dav.example.org");
    let secret = Calendar::Ics("https://calendar.google.com/calendar/ical/secret/basic.ics".into());
    assert_eq!(name(&secret), "calendar.google.com");
    let theirs = Calendar::CalDav {
        url: "https://dav.example.org/calendars/me/work/".into(),
        user: "them".into(),
    };
    assert_ne!(key(&work), key(&theirs));
    assert_eq!(
        key(&Calendar::Ics("a".into())),
        0xaf63_dc4c_8601_ec8c,
        "the same in every build, so Settings and a bar started earlier agree"
    );
}

#[test]
fn quoted_parameters_may_hold_colons() {
    let (name, params, value) =
        property("DTSTART;TZID=\"America/New_York:odd\":20261005T100000").unwrap();
    assert_eq!(name, "DTSTART");
    assert_eq!(params[0].1, "America/New_York:odd");
    assert_eq!(value, "20261005T100000");
}

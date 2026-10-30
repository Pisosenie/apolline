use std::time::Duration;

use chrono::NaiveDate;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use serde_json::Value;

use crate::icon::Icon;
use crate::shell::Message;

#[derive(Clone, Debug, PartialEq)]
pub struct Weather {
    pub place: String,
    pub now: f32,
    pub code: u8,
    pub days: Vec<Day>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub high: f32,
    pub low: f32,
    pub code: u8,
}

const EVERY: Duration = Duration::from_secs(30 * 60);

pub fn subscription(place: Option<String>) -> Subscription<Message> {
    match place {
        Some(place) => Subscription::run_with(place, watch),
        None => Subscription::none(),
    }
}

fn watch(place: &String) -> impl Stream<Item = Message> + use<> {
    let place = place.clone();
    iced::stream::channel(1, async move |mut output| {
        loop {
            let wanted = place.clone();
            let weather = tokio::task::spawn_blocking(move || fetch(&wanted))
                .await
                .ok()
                .and_then(|result| {
                    result
                        .map_err(|err| eprintln!("apolline: weather: {err}"))
                        .ok()
                });
            let retry = if weather.is_some() {
                EVERY
            } else {
                Duration::from_secs(5 * 60)
            };
            if output.send(Message::Weather(weather)).await.is_err() {
                return;
            }
            tokio::time::sleep(retry).await;
        }
    })
}

fn fetch(place: &str) -> Result<Weather, String> {
    let (latitude, longitude, name) = locate(place)?;
    let body = get(ureq::get("https://api.open-meteo.com/v1/forecast")
        .query("latitude", latitude.to_string())
        .query("longitude", longitude.to_string())
        .query("current", "temperature_2m,weather_code")
        .query(
            "daily",
            "weather_code,temperature_2m_max,temperature_2m_min",
        )
        .query("timezone", "auto")
        .query("forecast_days", "4"))?;
    parse(&body, name)
}

fn locate(place: &str) -> Result<(f64, f64, String), String> {
    if let Some((lat, lon)) = place.split_once(',')
        && let (Ok(lat), Ok(lon)) = (lat.trim().parse(), lon.trim().parse())
    {
        return Ok((lat, lon, place.to_owned()));
    }
    let body = get(ureq::get("https://geocoding-api.open-meteo.com/v1/search")
        .query("name", place)
        .query("count", "1"))?;
    let json: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    let hit = json["results"]
        .get(0)
        .ok_or(format!("no place called {place}"))?;
    Ok((
        hit["latitude"].as_f64().ok_or("no latitude")?,
        hit["longitude"].as_f64().ok_or("no longitude")?,
        hit["name"].as_str().unwrap_or(place).to_owned(),
    ))
}

fn get(request: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Result<String, String> {
    request
        .call()
        .map_err(|err| err.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|err| err.to_string())
}

fn parse(body: &str, place: String) -> Result<Weather, String> {
    let json: Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    let number = |v: &Value| v.as_f64().map(|n| n as f32);
    let current = &json["current"];
    let daily = &json["daily"];
    let list = |key: &str| daily[key].as_array().cloned().unwrap_or_default();
    let (dates, highs, lows, codes) = (
        list("time"),
        list("temperature_2m_max"),
        list("temperature_2m_min"),
        list("weather_code"),
    );
    let days = dates
        .iter()
        .zip(highs.iter().zip(lows.iter().zip(codes.iter())))
        .filter_map(|(date, (high, (low, code)))| {
            Some(Day {
                date: NaiveDate::parse_from_str(date.as_str()?, "%Y-%m-%d").ok()?,
                high: number(high)?,
                low: number(low)?,
                code: code.as_u64()? as u8,
            })
        })
        .collect();
    Ok(Weather {
        place,
        now: number(&current["temperature_2m"]).ok_or("no temperature")?,
        code: current["weather_code"].as_u64().ok_or("no weather code")? as u8,
        days,
    })
}

pub fn describe(code: u8) -> (Icon, &'static str) {
    match code {
        0 => (Icon::Sun, "Clear"),
        1 | 2 => (Icon::CloudSun, "Partly cloudy"),
        3 => (Icon::Cloud, "Cloudy"),
        45 | 48 => (Icon::CloudFog, "Fog"),
        51..=57 => (Icon::CloudDrizzle, "Drizzle"),
        61..=67 | 80..=82 => (Icon::CloudRain, "Rain"),
        71..=77 | 85 | 86 => (Icon::CloudSnow, "Snow"),
        95..=99 => (Icon::CloudLightning, "Thunderstorm"),
        _ => (Icon::Cloud, "Cloudy"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forecast_reply_becomes_today_and_the_next_days() {
        let body = r#"{"current":{"time":"2026-10-30T18:00","temperature_2m":11.4,"weather_code":2},
            "daily":{"time":["2026-10-30","2026-10-31"],"weather_code":[2,61],
            "temperature_2m_max":[14.1,12.0],"temperature_2m_min":[7.2,6.5]}}"#;
        let weather = parse(body, "Lisbon".into()).unwrap();
        assert_eq!(weather.now, 11.4);
        assert_eq!(describe(weather.code).1, "Partly cloudy");
        assert_eq!(weather.days.len(), 2);
        assert_eq!(
            weather.days[1].date,
            NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
        );
        assert_eq!(describe(weather.days[1].code).1, "Rain");
        assert!(parse("{}", "x".into()).is_err());
    }
}

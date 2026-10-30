use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// Rates are asked for only when you type a sum of money, then kept for half a day.
const FRESH: Duration = Duration::from_secs(12 * 3600);
const SOURCE: &str = "https://open.er-api.com/v6/latest/USD";

#[derive(Clone, Debug, PartialEq)]
pub struct Rates {
    // How much of each currency one US dollar buys.
    pub per_dollar: HashMap<String, f64>,
    pub day: String,
}

impl Rates {
    pub fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        let from = self.per_dollar.get(from)?;
        let to = self.per_dollar.get(to)?;
        (*from > 0.0).then(|| amount / from * to)
    }

    pub fn knows(&self, code: &str) -> bool {
        self.per_dollar.contains_key(code)
    }
}

pub async fn load() -> Result<Arc<Rates>, String> {
    tokio::task::spawn_blocking(|| {
        if let Some(rates) = cached() {
            return Ok(Arc::new(rates));
        }
        let body = ureq::get(SOURCE)
            .call()
            .map_err(|err| err.to_string())?
            .body_mut()
            .with_config()
            .limit(512 * 1024)
            .read_to_string()
            .map_err(|err| err.to_string())?;
        let rates = parse(&body)?;
        if let Some(file) = cache_file() {
            let _ = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&file, &body));
        }
        Ok(Arc::new(rates))
    })
    .await
    .map_err(|err| err.to_string())?
}

fn cache_file() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(cache.join("apolline").join("rates.json"))
}

fn cached() -> Option<Rates> {
    let file = cache_file()?;
    let age = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|at| SystemTime::now().duration_since(at).ok())?;
    if age > FRESH {
        return None;
    }
    parse(&std::fs::read_to_string(file).ok()?).ok()
}

fn parse(body: &str) -> Result<Rates, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    if value["result"] != "success" {
        return Err("the exchange rates didn't come".into());
    }
    let per_dollar: HashMap<String, f64> = value["rates"]
        .as_object()
        .ok_or("no rates in the answer")?
        .iter()
        .filter_map(|(code, rate)| Some((code.clone(), rate.as_f64()?)))
        .collect();
    let day = value["time_last_update_utc"]
        .as_str()
        .and_then(|stamp| stamp.get(5..16))
        .unwrap_or_default()
        .trim()
        .to_owned();
    Ok(Rates { per_dollar, day })
}

#[cfg(test)]
pub fn sample() -> Rates {
    Rates {
        per_dollar: [("USD", 1.0), ("EUR", 0.92), ("RUB", 81.5), ("KZT", 498.0)]
            .into_iter()
            .map(|(code, rate)| (code.to_owned(), rate))
            .collect(),
        day: "30 Oct 2026".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_reads_into_rates() {
        let body = r#"{"result":"success","time_last_update_utc":"Fri, 30 Oct 2026 00:02:31 +0000","base_code":"USD","rates":{"USD":1,"EUR":0.92,"RUB":81.5}}"#;
        let rates = parse(body).unwrap();
        assert_eq!(rates.day, "30 Oct 2026");
        assert_eq!(
            rates.convert(100.0, "EUR", "RUB").map(|v| v.round()),
            Some(8859.0)
        );
        assert_eq!(rates.convert(1.0, "EUR", "XXX"), None);
        assert!(parse(r#"{"result":"error"}"#).is_err());
    }
}

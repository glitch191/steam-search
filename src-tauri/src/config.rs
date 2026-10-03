//! `config.json` in `%APPDATA%\steam-search\`, plus a small append-only log file.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use tauri_plugin_global_shortcut::Shortcut;

pub const APP_NAME: &str = "steam-search";
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Insert";
pub const DEFAULT_MAX_RESULTS: usize = 50;
const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub hotkey: String,
    pub max_results: usize,
    pub steam_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { hotkey: DEFAULT_HOTKEY.into(), max_results: DEFAULT_MAX_RESULTS, steam_path: String::new() }
    }
}

pub fn app_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(APP_NAME)
}

pub fn config_path() -> PathBuf {
    app_dir().join("config.json")
}

/// Loads the config file, creating it with defaults when missing.
/// Invalid values fall back to their default and are reported in the log.
pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        let config = Config::default();
        let _ = fs::create_dir_all(app_dir());
        match fs::write(&path, serde_json::to_string_pretty(&config).unwrap() + "\n") {
            Ok(()) => log("created config.json with default values"),
            Err(e) => log(&format!("cannot create config.json: {e}")),
        }
        return config;
    }
    match fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(e) => {
            log(&format!("cannot read config.json, using defaults: {e}"));
            Config::default()
        }
    }
}

pub fn parse(text: &str) -> Config {
    let mut config = Config::default();
    // Some editors save UTF-8 with a byte order mark, which serde_json rejects.
    let text = text.trim_start_matches('\u{feff}');
    let json: Value = match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => {
            log("config.json is not a valid JSON object, using defaults");
            return config;
        }
    };

    match json.get("hotkey") {
        None => {}
        Some(Value::String(s)) if parse_hotkey(s).is_some() => config.hotkey = s.trim().to_string(),
        Some(v) => log(&format!("invalid hotkey {v}, using {DEFAULT_HOTKEY}")),
    }
    match json.get("maxResults") {
        None => {}
        Some(v) => match v.as_u64() {
            Some(n @ 1..=500) => config.max_results = n as usize,
            _ => log(&format!("invalid maxResults {v} (expected 1-500), using {DEFAULT_MAX_RESULTS}")),
        },
    }
    match json.get("steamPath") {
        None => {}
        Some(Value::String(s)) => config.steam_path = s.trim().to_string(),
        Some(v) => log(&format!("invalid steamPath {v}, ignoring it")),
    }
    config
}

/// Parses a hotkey such as "Ctrl+Shift+Insert". Accepts "Ins" for Insert and "Win" for the Windows key.
pub fn parse_hotkey(text: &str) -> Option<Shortcut> {
    let normalized: Vec<String> = text
        .split('+')
        .map(|part| {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ins" => "Insert".to_string(),
                "win" | "windows" => "Super".to_string(),
                _ => part.to_string(),
            }
        })
        .collect();
    if normalized.iter().any(|p| p.is_empty()) {
        return None;
    }
    Shortcut::from_str(&normalized.join("+")).ok()
}

/// Appends a timestamped line to `%APPDATA%\steam-search\steam-search.log`.
pub fn log(message: &str) {
    if cfg!(test) {
        return;
    }
    let dir = app_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{APP_NAME}.log"));
    if fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
        let _ = fs::remove_file(&path);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{} {message}", timestamp());
    }
}

/// UTC time as "YYYY-MM-DD HH:MM:SS", without a date crate.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil date from day count (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkey_and_insert_aliases_parse() {
        assert!(parse_hotkey(DEFAULT_HOTKEY).is_some());
        assert_eq!(parse_hotkey("Ctrl+Shift+Ins"), parse_hotkey("ctrl + shift + insert"));
        assert!(parse_hotkey("Alt+Space").is_some());
        assert!(parse_hotkey("Ctrl++").is_none());
        assert!(parse_hotkey("Ctrl+Nonsense").is_none());
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let c = parse(r#"{ "hotkey": "Ctrl+Bogus", "maxResults": -3, "steamPath": 12 }"#);
        assert_eq!(c, Config::default());
        assert_eq!(parse("not json"), Config::default());
    }

    #[test]
    fn valid_values_are_kept() {
        let c = parse(r#"{ "hotkey": "Alt+F2", "maxResults": 20, "steamPath": " D:\\Steam " }"#);
        assert_eq!(c.hotkey, "Alt+F2");
        assert_eq!(c.max_results, 20);
        assert_eq!(c.steam_path, r"D:\Steam");
    }

    #[test]
    fn byte_order_mark_is_ignored() {
        assert_eq!(parse("\u{feff}{ \"maxResults\": 9 }").max_results, 9);
    }

    #[test]
    fn timestamp_has_expected_shape() {
        assert_eq!(timestamp().len(), "2026-01-01 00:00:00 UTC".len());
    }
}

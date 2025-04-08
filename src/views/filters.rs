use std::sync::OnceLock;

use chrono::NaiveDateTime;

static TZ: OnceLock<chrono_tz::Tz> = OnceLock::new();

pub fn set_timezone(tz: chrono_tz::Tz) {
    TZ.set(tz).unwrap();
}

pub fn format_datetime(date: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
    // For some reason `and_local_timezone` is fallible whereas `and_utc -> `with_timezone` is not
    let local = date.and_utc().with_timezone(TZ.get().expect("Uninitialized timezone value"));
    let formatted = local.format(format).to_string();
    Ok(formatted)
}

use std::sync::OnceLock;

use chrono::NaiveDateTime;

/// App timezone for usage
pub static CONFIG: OnceLock<crate::Config> = OnceLock::new();

/// Askama requires a `filters` module to be in scope to provide extra functions
/// to templates. Any such functions should be added here.
pub mod filters {
    use super::*;

    /// Format a datetime with a `strftime` format string.
    pub fn format_datetime(dt: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
        let tz = CONFIG.get().unwrap().app.tz;

        let fmt = dt.and_utc().with_timezone(&tz).format(format);
        Ok(fmt.to_string())
    }
}

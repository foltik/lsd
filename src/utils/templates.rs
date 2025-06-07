use std::sync::OnceLock;

use chrono::NaiveDateTime;

/// Global app config for reference from askama filters, set once at startup.
pub static CONFIG: OnceLock<crate::Config> = OnceLock::new();

/// Askama implicitly looks for a `filters` module to be in the same scope as
/// the `#[derive(Template)]` to provide extra functions to templates.
///
/// We export this along with `Template` in `crate::prelude`, so it should always properly be in scope.
pub mod filters {
    use super::*;

    /// Format a datetime with a `strftime` format string.
    pub fn format_datetime(dt: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
        let tz = CONFIG.get().unwrap().app.tz;

        let fmt = dt.and_utc().with_timezone(&tz).format(format);
        Ok(fmt.to_string())
    }

    /// Convert Option<String> to empty string if None
    pub fn unwrap_or_empty(value: &Option<String>) -> askama::Result<String> {
        Ok(value.as_deref().unwrap_or("").to_string())
    }

    /// Check if Option<String> has a value (not None and not empty)
    pub fn has_value(value: &Option<String>) -> askama::Result<bool> {
        Ok(value.as_ref().map_or(false, |s| !s.is_empty()))
    }
}

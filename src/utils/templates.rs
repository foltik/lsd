use std::sync::OnceLock;

use chrono::NaiveDateTime;

use crate::prelude::*;

/// Global app config for reference from askama filters, set once at startup.
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// Askama implicitly looks for a `filters` module to be in the same scope as
/// the `#[derive(Template)]` to provide extra functions to templates.
///
/// We export this along with `Template` in `crate::prelude`, so it should always properly be in scope.
pub mod filters {
    use std::fmt::Display;

    use super::*;

    /// Format a datetime with a `strftime` format string.
    pub fn format_datetime(dt: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
        let tz = CONFIG.get().unwrap().app.tz;

        let fmt = dt.and_utc().with_timezone(&tz).format(format);
        Ok(fmt.to_string())
    }

    /// Format an optional datetime with a `strftime` format string where None maps to the empty string.
    /// Useful for `<input value="...">` where everything is a string and "" is null.
    pub fn format_optional_datetime(
        dt: &Option<NaiveDateTime>,
        format: &str,
    ) -> Result<String, askama::Error> {
        match dt {
            Some(dt) => format_datetime(dt, format),
            None => Ok("".into()),
        }
    }

    /// Turn an `Option<T>` into a `String`, where None maps to the empty string.
    /// Useful for `<input value="...">` where everything is a string and "" is null.
    pub fn unwrap_or_empty<T: Display>(value: &Option<T>) -> Result<String, askama::Error> {
        Ok(match value {
            Some(v) => v.to_string(),
            None => "".into(),
        })
    }
}

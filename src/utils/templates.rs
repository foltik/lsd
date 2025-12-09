use chrono::NaiveDateTime;

use crate::prelude::*;

/// Askama implicitly looks for a `filters` module to be in the same scope as
/// the `#[derive(Template)]` to provide extra functions to templates.
///
/// We export this along with `Template` in `crate::prelude`, so it should always properly be in scope.
pub mod filters {
    use std::fmt::Display;

    use super::*;

    /// Check if a user is logged in and has a role.
    pub fn has_role(user: &Option<User>, role: &str) -> Result<bool, askama::Error> {
        Ok(user.as_ref().is_some_and(|u| u.has_role(role)))
    }

    /// Check if a user is logged in and has a staff role.
    pub fn has_staff_role(user: &Option<User>) -> Result<bool, askama::Error> {
        Ok(user.as_ref().is_some_and(|u| u.has_staff_role()))
    }

    /// Format a datetime with a `strftime` format string.
    pub fn format_datetime(dt: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
        let tz = config().app.tz;

        let fmt = dt.and_utc().with_timezone(&tz).format(format);
        Ok(fmt.to_string())
    }

    /// Format an optional datetime with a `strftime` format string where None maps to the empty string.
    /// Useful for `<input value="...">` where everything is a string and "" is null.
    pub fn format_optional_datetime(
        dt: &Option<NaiveDateTime>, format: &str,
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

    /// Returns the site URL
    pub fn url(_dummy: &str) -> Result<String, askama::Error> {
        Ok(config().app.url.clone())
    }

    /// Returns the site URL
    pub fn mailto(_dummy: &str) -> Result<String, askama::Error> {
        let email = config().email.from.email.to_string();
        Ok(format!("mailto:{email}"))
    }

    /// Livereload script enabled on debug builds.
    /// Askama doesn't support plain global functions, so we have to take a dummy argument.
    #[cfg(debug_assertions)]
    pub fn livereload(_dummy: &str) -> Result<String, askama::Error> {
        // Parse app url, split off port, switch to HTTP
        let url = &config().app.url;
        let url = match url.rsplit_once(":") {
            None => url,
            Some((url, _port)) => url,
        };
        let url = url.replace("https", "http");

        Ok(format!(r#"<script src="{url}:35729/livereload.js"></script>"#))
    }
    #[cfg(not(debug_assertions))]
    pub fn livereload(_dummy: &str) -> Result<String, askama::Error> {
        Ok("".into())
    }
}

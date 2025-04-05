use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::collections::HashMap;
use tera::{Tera, Value};

use crate::Config;

/// Initialize the [`Tera`] template engine, including our custom filter functions.
pub fn templates(config: &Config) -> Result<Tera> {
    let mut tera = Tera::new("frontend/templates/*")?;

    // Format a datetime with a [`strftime`] format string.
    // Also converts from UTC to the app's local timezone.
    //
    // Usage: `{{ date | format_datetime(format="%m.%d.%Y") }}`
    //
    // [`strftime`]: https://devhints.io/strftime
    let tz = config.app.tz;
    register_filter(
        &mut tera,
        "format_datetime",
        move |date: &Value, args: &HashMap<String, Value>| {
            let format = args.get("format").context("missing arg=`format`")?;
            let format = format.as_str().context("arg=`format` must be a string")?;

            let date: &str = date.as_str().with_context(|| format!("value={date:?} must be a string"))?;
            let date: NaiveDateTime = date.parse().context("parsing date")?;
            let utc = date.and_utc();
            let local = utc.with_timezone(&tz);

            let formatted = local.format(format).to_string();
            Ok(Value::String(formatted))
        },
    );

    Ok(tera)
}

/// Register a tera filter function.
///
/// On top of the regular `register_filter`, this function adds the filter name
/// as context to any errors, and handles conversion from `anyhow::Error` to
/// `tera::Error`.
fn register_filter<F>(tera: &mut Tera, name: &str, func: F)
where
    F: Fn(&Value, &HashMap<String, Value>) -> Result<Value> + Send + Sync + 'static,
{
    let name_ = name.to_string();
    tera.register_filter(
        name,
        move |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            func(value, args)
                .with_context(|| format!("{}()", &name_))
                .map_err(|err| tera::Error::msg(err.to_string()))
        },
    );
}

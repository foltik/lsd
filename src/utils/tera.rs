use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use tera::{Tera, Value};

/// Initialize the [`Tera`] template engine, including our custom filter functions.
pub fn templates() -> Result<Tera> {
    let mut tera = Tera::new("templates/*")?;
    register_filter(&mut tera, "format_datetime", format_datetime);
    Ok(tera)
}

/// Format a datetime with a [`strftime`] format string.
///
/// Usage: `{{ date | format_datetime(format="%m.%d.%Y") }}`
///
/// [`strftime`]: https://devhints.io/strftime
fn format_datetime(date: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let format = args.get("format").context("missing arg=`format`")?;
    let format = format.as_str().context("arg=`format` must be a string")?;

    let date: &str = date.as_str().with_context(|| format!("value={date:?} must be a string"))?;
    let date: DateTime<Local> = date.parse().context("parsing date")?;

    let formatted = date.format(format).to_string();
    Ok(Value::String(formatted))
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

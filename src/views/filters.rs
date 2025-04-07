use chrono::NaiveDateTime;

pub fn format_datetime(date: &NaiveDateTime, format: &str) -> Result<String, askama::Error> {
    let utc = date.and_utc();

    // TODO how to access config from here?
    // let local = utc.with_timezone(&tz);

    let formatted = utc.format(format).to_string();
    tracing::info!("Formatted to {}", formatted);
    Ok(formatted)
}

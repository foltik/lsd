use std::panic;

use crate::utils::config::{AlertsTelegramConfig, config};

/// Install a panic hook that alerts before running the default hook.
pub fn init() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let (file, line) = info.location().map_or(("unknown", 0), |l| (l.file(), l.line()));
        alert(format!("panic: {info}"), file, line);
        default_hook(info);
    }));
}

/// Log an alert and send to any configured backends.
#[macro_export]
macro_rules! alert {
    ( $($arg:tt)* ) => {
        $crate::utils::alerts::alert(format!($($arg)*), file!(), line!())
    };
}
pub fn alert(message: String, file: &str, line: u32) {
    let href = format!("https://github.com/foltik/lsd/blob/main/{file}#L{line}");
    send("Alert", &message, &format!("{file}:{line}"), &href);
}
pub fn alert_frontend(message: String, url: &str, line: u32) {
    send("Alert from frontend", &message, &format!("{url}:{line}"), url);
}

fn send(title: &str, message: &str, location: &str, href: &str) {
    tracing::error!("{title}: {message} at {location}");
    if let Some(telegram) = config().alerts.as_ref().and_then(|a| a.telegram.as_ref()) {
        send_telegram(telegram, title, message, location, href);
    }
}
fn send_telegram(config: &AlertsTelegramConfig, title: &str, message: &str, location: &str, href: &str) {
    // The panic hook can run on a thread with no runtime.
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };

    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };

    // Telegram rejects messages over 4096 chars
    let message: String = escape(&message.chars().take(4000).collect::<String>());
    let location = escape(location);
    let href = escape(href);

    let text = format!("<b>{title}:</b> {message}\nat <a href=\"{href}\">{location}</a>",);

    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.api_key);
    let chat_id = config.chat_id.clone();
    handle.spawn(async move {
        let res = reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "chat_id": chat_id, "text": text, "parse_mode": "HTML" }))
            .send()
            .await;

        if let Err(e) = res {
            tracing::error!("Failed to send Telegram alert: {e}");
        }
    });
}

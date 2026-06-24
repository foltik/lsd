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

/// Log an alert and send it to any configured backends. Prefer the `alert!` macro, which fills in
/// the call site.
#[macro_export]
macro_rules! alert {
    ( $($arg:tt)* ) => {
        $crate::utils::alerts::alert(format!($($arg)*), file!(), line!())
    };
}
pub fn alert(message: String, file: &str, line: u32) {
    tracing::error!("Alert: {message} at {file}:{line}");
    if let Some(telegram) = config().alerts.as_ref().and_then(|a| a.telegram.as_ref()) {
        send_telegram(telegram, &message, file, line);
    }
}

fn send_telegram(config: &AlertsTelegramConfig, message: &str, file: &str, line: u32) {
    // The panic hook can run on a thread with no runtime.
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };

    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let message = format!(
        "<b>Alert: {}</b>\nat <a href=\"https://github.com/foltik/lsd/blob/main/{file}#L{line}\">{file}:{line}</a>",
        escape(message)
    );

    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.api_key);
    let chat_id = config.chat_id.clone();
    handle.spawn(async move {
        let res = reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "chat_id": chat_id, "text": message, "parse_mode": "HTML" }))
            .send()
            .await;

        if let Err(e) = res {
            tracing::error!("Failed to send Telegram alert: {e}");
        }
    });
}

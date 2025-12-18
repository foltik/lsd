use backtrace::Backtrace;
use sentry::Level;
use sentry::integrations::backtrace::backtrace_to_stacktrace;
use sentry::integrations::tracing::EventFilter;
use sentry::protocol::Event;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

use crate::utils::config::SentryConfig;

pub fn init(config: &SentryConfig) {
    let guard = sentry::init((
        config.dsn.as_str(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            // Send request payloads, headers, etc.
            send_default_pii: true,
            ..Default::default()
        },
    ));
    std::mem::forget(guard);

    // static OnceCell<
}

pub fn layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    sentry::integrations::tracing::layer()
        .event_filter(|e| match *e.level() {
            tracing::Level::ERROR | tracing::Level::WARN => EventFilter::Event,
            _ => EventFilter::Ignore,
        })
        .span_filter(|e| matches!(*e.level(), tracing::Level::ERROR | tracing::Level::WARN))
}

pub fn report(message: String) {
    sentry::capture_event(Event { level: Level::Error, message: Some(message), ..Default::default() });
}

pub fn report_trace(message: String, backtrace: &Backtrace) {
    let stacktrace = backtrace_to_stacktrace(backtrace);

    let mut event = Event { level: Level::Error, message: Some(message.clone()), ..Default::default() };
    event.exception.values = vec![sentry::protocol::Exception {
        ty: "Error".into(),
        value: Some(message),
        stacktrace,
        ..Default::default()
    }];

    sentry::capture_event(event);
}

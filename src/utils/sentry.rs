use sentry::integrations::tracing::EventFilter;
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

pub fn layer<S: Subscriber>() -> impl Layer<S>
where
    S: for<'a> LookupSpan<'a>,
{
    sentry::integrations::tracing::layer()
        .event_filter(|e| match *e.level() {
            tracing::Level::ERROR | tracing::Level::WARN => EventFilter::Event,
            _ => EventFilter::Ignore,
        })
        .span_filter(|e| match *e.level() {
            tracing::Level::ERROR | tracing::Level::WARN => true,
            _ => false,
        })
}

use std::time::Duration;

use axum::{extract::MatchedPath, http::Request, response::Response};
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::utils::types::AppRouter;

/// Register debug tracing middleware.
pub fn register(router: AppRouter) -> AppRouter {
    // Add a middleware that logs all incoming requests and responses, including latency and status.
    router.layer(
        TraceLayer::new_for_http()
            // Start a `tracing::span` for each request.
            .make_span_with(|req: &Request<_>| {
                let path = match req.extensions().get::<MatchedPath>() {
                    Some(path) => path.as_str(),
                    None => req.uri().path(),
                };
                // Fields populated later must be initialized as `tracing::field::Empty`.
                tracing::info_span!("request", method = ?req.method(), path, status = tracing::field::Empty)
            })
            // Add some extra fields once the response is generated.
            .on_response(|res: &Response, latency: Duration, span: &Span| {
                span.record("status", res.status().as_u16());
                tracing::info!("handled in {latency:?}");
            }),
    )
}

use axum::http::Request;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt as _;
use tower_http::request_id::{MakeRequestId, RequestId};
use tower_http::trace::{DefaultOnResponse, MakeSpan, TraceLayer};
use tracing::Span;
use uuid::Uuid;

use crate::prelude::*;

#[derive(Clone, Copy)]
pub struct MakeRequestUuidV7;
impl MakeRequestId for MakeRequestUuidV7 {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        // Use UUIDv7 so that request ID can be sorted by time
        let request_id = Uuid::now_v7();
        Some(RequestId::new(request_id.to_string().parse().unwrap()))
    }
}

#[derive(Clone, Copy)]
pub struct LoggingMakeSpan;
impl<B> MakeSpan<B> for LoggingMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let method = request.method();
        let path = request.uri().path();
        tracing::info!("{method} {path:?}");
        tracing::span!(tracing::Level::DEBUG, "request", %method, %path)
    }
}

/// Register tracing-related middleware into the router.
pub fn add_middleware(router: AxumRouter) -> AxumRouter {
    router.layer(
        ServiceBuilder::new()
            .set_x_request_id(MakeRequestUuidV7)
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(LoggingMakeSpan)
                    .on_response(DefaultOnResponse::new()),
            )
            .propagate_x_request_id(),
    )
}

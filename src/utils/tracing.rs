use axum::http::Request;
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestId, RequestId},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    ServiceBuilderExt as _,
};
use uuid::Uuid;

use crate::utils::types::AppRouter;

#[derive(Clone, Copy)]
pub struct MakeRequestUuidV7;
impl MakeRequestId for MakeRequestUuidV7 {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        // Use UUIDv7 so that request ID can be sorted by time
        let request_id = Uuid::now_v7();
        Some(RequestId::new(request_id.to_string().parse().unwrap()))
    }
}

pub trait WithTracingLayer {
    fn with_tracing_layer(self) -> AppRouter;
}

impl WithTracingLayer for AppRouter {
    fn with_tracing_layer(self) -> AppRouter {
        self.layer(
            ServiceBuilder::new()
                .set_x_request_id(MakeRequestUuidV7)
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().include_headers(true))
                        .on_response(DefaultOnResponse::new().include_headers(true)),
                )
                .propagate_x_request_id(),
        )
    }
}

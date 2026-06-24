use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::{ConnectInfo, Request};
use axum::http::Request as HttpRequest;
use axum::middleware::Next;
use axum::response::Response;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt as _;
use tower_http::request_id::{MakeRequestId, RequestId};
use uuid::Uuid;

use crate::prelude::*;

#[derive(Clone, Copy)]
pub struct MakeRequestUuidV7;
impl MakeRequestId for MakeRequestUuidV7 {
    fn make_request_id<B>(&mut self, _request: &HttpRequest<B>) -> Option<RequestId> {
        // Use UUIDv7 so that request ID can be sorted by time
        let request_id = Uuid::now_v7();
        Some(RequestId::new(request_id.to_string().parse().unwrap()))
    }
}

#[rustfmt::skip]
async fn log_middleware(request: Request, next: Next) -> Response {
    let method = request.method().as_str().to_owned();
    let path = request.uri().path_and_query().map_or_else(|| request.uri().path(), |pq| pq.as_str()).to_owned();
    let version = request.version();
    let ip = request.extensions().get::<ConnectInfo<SocketAddr>>().unwrap().ip().to_canonical().to_string();

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_micros() as f64 / 1000.0;
    let status = response.status().as_u16();

    tracing::info!("{elapsed:>7.3}ms {ip:<15} {status} {method:<4} {version:?}  {path}");
    response
}

/// Register tracing-related middleware into the router.
pub fn add_middleware(router: AxumRouter) -> AxumRouter {
    router.layer(
        ServiceBuilder::new()
            .set_x_request_id(MakeRequestUuidV7)
            .layer(axum::middleware::from_fn(log_middleware))
            .propagate_x_request_id(),
    )
}

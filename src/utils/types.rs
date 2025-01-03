use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

/// The global shared application state.
pub use crate::app::AppState;
pub type SharedAppState = Arc<AppState>;

/// The global router type, with our shared application state.
pub type AppRouter = axum::Router<SharedAppState>;

/// App-wide result type which automatically handles conversion to an HTTP response.
pub struct AppError(anyhow::Error);
pub type AppResult<T> = Result<T, AppError>;

/// Convert an [`AppError`] into an HTTP response.
///
/// This allows us to return `AppResult from `axum::Handler` functions, and
/// tells the framework how to deal with errors.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // TODO: add a `dev` mode to `config.app`, and:
        // * when enabled, respond with a stack trace
        // * when disabled, respond with a generic error message that doesn't leak any details
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", self.0)).into_response()
    }
}
/// Allow converting anything that can be converted to an `anyhow::Result`
/// into an `AppResult` with the `?` operator.
impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

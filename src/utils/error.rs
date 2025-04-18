use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::views;

/// App-wide result type which automatically handles conversion to an HTTP response.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("not authorized")]
    NotAuthorized,

    #[error(transparent)]
    Email(#[from] lettre::error::Error),

    #[error(transparent)]
    Smtp(#[from] lettre::transport::smtp::Error),

    #[error(transparent)]
    Render(#[from] askama::Error),

    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

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
        match self {
            AppError::NotAuthorized => serve_401().into_response(),
            AppError::NotFound => serve_404().into_response(),
            AppError::Database(e) => {
                tracing::error!("{e}");
                serve_500().into_response()
            }
            AppError::Smtp(e) => {
                tracing::error!("{e}");
                serve_500().into_response()
            }
            AppError::Email(e) => {
                tracing::error!("{e}");
                serve_500().into_response()
            }
            AppError::Render(e) => {
                tracing::error!("{e}");
                serve_500().into_response()
            }
        }
    }
}

fn serve_401() -> impl IntoResponse {
    (
        StatusCode::UNAUTHORIZED,
        views::index::ErrorTemplate { message: "You do not have permission to view this page".into() },
    )
}

pub fn serve_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        views::index::ErrorTemplate { message: "Page not found".into() },
    )
}

fn serve_500() -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        views::index::ErrorTemplate { message: "Something went wrong :( Please try again".into() },
    )
}

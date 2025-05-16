use crate::prelude::*;

/// App-wide result type which automatically handles conversion to an HTTP response.
pub type AppResult<T> = Result<T, AppError>;

#[derive(thiserror::Error, Debug)]
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

/// Convert an [`AppError`] into an HTTP response.
///
/// This allows us to return `AppResult from `axum::Handler` functions, and
/// tells the framework how to deal with errors.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("{self:#}");

        let error_401 = || (StatusCode::UNAUTHORIZED, "You do not have permission to view this page");
        let error_404 = || (StatusCode::NOT_FOUND, "Page not found");
        let error_500 = || (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong :( Please try again");

        let (status, message) = match self {
            AppError::NotAuthorized => error_401(),
            AppError::NotFound => error_404(),
            AppError::Database(_) => error_500(),
            AppError::Smtp(_) => error_500(),
            AppError::Email(_) => error_500(),
            AppError::Render(_) => error_500(),
        };

        // TODO: add a `dev` mode to `config.app`, and:
        // * when enabled, respond with a stack trace
        // * when disabled, respond with a generic error message that doesn't leak any details
        #[derive(Template, WebTemplate)]
        #[template(path = "error.html")]
        struct Html {
            message: String,
        }
        let html = Html { message: message.to_string() };

        (status, html).into_response()
    }
}

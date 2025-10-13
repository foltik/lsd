use crate::prelude::*;

/// App-wide result type which automatically handles conversion to an HTTP response.
pub type AppResult<T> = Result<T, AppError>;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bad request")]
    BadRequest,
    #[error(transparent)]
    BadMultipart(#[from] axum::extract::multipart::MultipartError),
    #[error("not authorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,

    #[error(transparent)]
    Stripe(#[from] crate::utils::stripe::StripeError),
    #[error(transparent)]
    Email(#[from] lettre::error::Error),
    #[error(transparent)]
    Smtp(#[from] lettre::transport::smtp::Error),
    #[error(transparent)]
    Render(#[from] askama::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

/// Convert an [`AppError`] into an HTTP response.
///
/// This allows us to return `AppResult from `axum::Handler` functions, and
/// tells the framework how to deal with errors.
impl IntoResponse for AppError {
    #[rustfmt::skip]
    fn into_response(self) -> Response {
        tracing::error!("{self:#}");

        let error_400 = || (StatusCode::BAD_REQUEST, "Invalid request.");
        let error_401 = || (StatusCode::UNAUTHORIZED, "You do not have permission to view this page.");
        let error_404 = || (StatusCode::NOT_FOUND, "Page not found.");
        let error_500 = || (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong on our end. Please try again, or contact us if the issue persists."
        );

        let (status, message) = match self {
            AppError::BadRequest => error_400(),
            AppError::BadMultipart(e) => {
                tracing::error!("multipart error: {e}");
                error_400()
            },
            AppError::Unauthorized => error_401(),
            AppError::NotFound => error_404(),
            AppError::Database(_) => error_500(),
            AppError::Smtp(_) => error_500(),
            AppError::Email(_) => error_500(),
            AppError::Render(_) => error_500(),
            AppError::Reqwest(_) => error_500(),
            AppError::Stripe(_) => error_500(),
        };

        // TODO: add a `dev` mode to `config.app`, and:
        // * when enabled, respond with a stack trace
        // * when disabled, respond with a generic error message that doesn't leak any details
        #[derive(Template, WebTemplate)]
        #[template(path = "error.html")]
        struct Html {
            user: Option<User>,
            message: String,
        }
        let html = Html { user: None, message: message.to_string() };

        (status, html).into_response()
    }
}

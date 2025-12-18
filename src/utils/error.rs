use std::error::Error;

use backtrace::Backtrace;

use crate::prelude::*;

pub type Result<T, E = AnyError> = std::result::Result<T, E>;

#[derive(Debug)]
pub struct AnyError {
    message: String,
    backtrace: Backtrace,
}
impl AnyError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), backtrace: Backtrace::new() }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

// Wrapper for Into<Box<dyn Error>> since we can't impl Error directly on would conflict with the blanket From<E>
#[derive(Debug)]
pub struct AnyErrorWrapper(AnyError);
impl std::fmt::Display for AnyErrorWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.message.fmt(f)
    }
}
impl std::error::Error for AnyErrorWrapper {}
impl From<AnyError> for Box<dyn Error + Send + Sync> {
    fn from(e: AnyError) -> Self {
        Box::new(AnyErrorWrapper(e))
    }
}

impl<E: Error + Send + Sync + 'static> From<E> for AnyError {
    #[track_caller]
    fn from(e: E) -> Self {
        let backtrace = Backtrace::new();

        let mut message = format!("{e}");
        let mut curr: &dyn Error = &e;
        while let Some(prev) = curr.source() {
            write!(message, ": {prev}").unwrap();
            curr = prev;
        }

        Self { message, backtrace }
    }
}

#[macro_export]
macro_rules! bail {
    ( $fmt:expr ) => {
        return Err(AnyError::new($fmt));
    };
    ( $fmt:expr, $($arg:expr),* $(,)?) => {
        return Err(AnyError::new(format!("{}", format_args!($fmt, $($arg),*))));
    }
}
pub use bail;

#[macro_export]
macro_rules! any {
    ( $fmt:expr ) => {
        AnyError::new($fmt)
    };
    ( $fmt:expr, $($arg:expr),* $(,)?) => {
        AnyError::new(format!("{}", format_args!($fmt, $($arg),*)))
    }
}
pub use any;

/// Semantic app error.
/// * For HTML responses, gets templated into a nice error page.
/// * For JSON responses, gets treated like a normal error.
pub enum AppError {
    NotFound(Backtrace),
    Unauthorized(Backtrace),
    Invalid(Backtrace),
}
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn message(&self) -> &'static str {
        match self {
            AppError::NotFound { .. } => "Page not found.",
            AppError::Unauthorized { .. } => "Unauthorized.",
            AppError::Invalid { .. } => "Invalid request.",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Invalid(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn backtrace(&self) -> &Backtrace {
        match self {
            AppError::NotFound(bt) | AppError::Unauthorized(bt) | AppError::Invalid(bt) => bt,
        }
    }
}

/// API-only JSON handler return type.
/// Returns either T as JSON, or {error: "message"}
pub type JsonResult<T> = Result<Json<T>, JsonError>;
pub enum JsonError {
    App(AppError),
    Any(AnyError),
}
impl From<AppError> for JsonError {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}
macro_rules! impl_json_from {
    ( $from:ty ) => {
        impl From<$from> for JsonError {
            fn from(e: $from) -> Self {
                Self::Any(AnyError::from(e))
            }
        }
    };
}

/// User-visible HTML handler return type.
pub type HtmlResult = Result<Response, HtmlError>;
pub enum HtmlError {
    App(AppError),
    Any(AnyError),
}
impl From<AppError> for HtmlError {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}
macro_rules! impl_html_from {
    ( $from:ty ) => {
        impl From<$from> for HtmlError {
            fn from(e: $from) -> Self {
                Self::Any(AnyError::from(e))
            }
        }
    };
}

#[derive(Template, WebTemplate)]
#[template(path = "error.html")]
pub struct ErrorHtml {
    pub user: Option<User>,
    pub title: String,
    pub message: String,
    pub context: Option<String>,
    pub backtrace: Option<String>,
}

impl IntoResponse for HtmlError {
    #[rustfmt::skip]
    fn into_response(self) -> Response {
        if let HtmlError::Any(e) = &self {
            tracing::error!("{}", e.message());
            crate::utils::sentry::report_trace(e.message().into(), e.backtrace());
        }

        #[cfg(debug_assertions)]
        let backtrace = match &self {
            HtmlError::App(e) => Some(format!("{:?}", e.backtrace())),
            HtmlError::Any(e) => Some(format!("{:?}", e.backtrace())),
        };
        #[cfg(not(debug_assertions))]
        let backtrace = None;

        #[cfg(debug_assertions)]
        let context = match &self {
            HtmlError::App(_) => None,
            HtmlError::Any(e) => Some(e.message.clone()),
        };
        #[cfg(not(debug_assertions))]
        let context = None;

        let (status, html) = match &self {
            HtmlError::App(e) => (e.status(), ErrorHtml {
                user: None,
                title: "Error".into(),
                message: e.message().into(),
                context,
                backtrace,
            }),
            HtmlError::Any(_) => (StatusCode::INTERNAL_SERVER_ERROR, {
                let contact_to = config().email.contact_to.as_ref();
                let from = &config().email.from;
                let email = contact_to.unwrap_or(from).to_string();
                let mailto = format!(r#"<a href="mailto:{email}">{email}</a>"#);
                ErrorHtml {
                    user: None,
                    title: "We encountered an unexpected error".into(),
                    message: format!("The team has been alerted that there is an issue. If you need assistance, please contact {mailto}."),
                    context,
                    backtrace,
                }
            })
        };
        (status, html).into_response()
    }
}

impl IntoResponse for JsonError {
    fn into_response(self) -> Response {
        if let JsonError::Any(e) = &self {
            tracing::error!("{}", e.message());
            crate::utils::sentry::report_trace(e.message().into(), e.backtrace());
        }

        let message = match self {
            JsonError::App(e) => e.message(),
            JsonError::Any(_) => "Internal server error.",
        };

        Json(json!({"error": message.to_string()})).into_response()
    }
}

// Conversions from any and all other error types to our app error types
macro_rules! impl_from {
    ( $($from:ty),* ) => {
        $(
            impl_html_from!($from);
            impl_json_from!($from);
        )*
    }
}
impl_from! {
    axum::extract::multipart::MultipartError,
    lettre::error::Error,
    lettre::transport::smtp::Error,
    askama::Error,
    sqlx::Error,
    reqwest::Error
}
impl From<AnyError> for JsonError {
    fn from(e: AnyError) -> Self {
        Self::Any(e)
    }
}
impl From<AnyError> for HtmlError {
    fn from(e: AnyError) -> Self {
        Self::Any(e)
    }
}

// Helpers
#[track_caller]
pub fn not_found() -> AppError {
    AppError::NotFound(Backtrace::new())
}
#[macro_export]
macro_rules! bail_not_found {
    () => {
        return Err(not_found().into())
    };
}
pub use bail_not_found;

#[track_caller]
pub fn unauthorized() -> AppError {
    AppError::Unauthorized(Backtrace::new())
}
#[macro_export]
macro_rules! bail_unauthorized {
    () => {
        return Err(unauthorized().into())
    };
}
pub use bail_unauthorized;

#[track_caller]
pub fn invalid() -> AppError {
    AppError::Invalid(Backtrace::new())
}
#[macro_export]
macro_rules! bail_invalid {
    () => {
        return Err(invalid().into())
    };
}
pub use bail_invalid;

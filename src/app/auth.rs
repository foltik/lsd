//! A simple passwordless authentication flow using one-time links sent via email.
//!
//! TODO: Switch to one-time codes (123-456) instead of links:
//! * More robust against clients and intermediaries that auto-open URLs
//! * Easier to transfer across devices than a magic link
//!
//! We choose this scheme instead of one with usernames/passwords to reduce
//! friction and simplify onboarding.
//!
//! # High-level flow
//!
//! 1. **Email input**: User enters their email and submits a login form.
//! 2. **Token generated**: Server creates a short-lived link with a login token and emails it to the user.
//!    - If the user is already registered, the link points to `/login?token=...`.
//!    - If the user is not registered, the link points to `/register?token=...`.
//! 3. **Link clicked**: User clicks the link, passing the token back to the server.
//!    - `/login`: The user gets a new session cookie and is redirected home.
//!    - `/register`: The user is prompted to enter their first/last name.
//!      Upon submission, the user gets a new session cookie and is redirected home.

use lettre::message::Mailbox;
use lettre::message::header::ContentType;

use crate::db::token::{LoginToken, SessionToken};
use crate::prelude::*;

/// Add all `auth` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/login", post(login_form).get(login_link))
    })
}

/// Add all `auth` middleware to the router.
pub fn add_middleware(router: AxumRouter, state: SharedAppState) -> AxumRouter {
    /// Middleware layer to lookup add a `User` to the request if a session token is present.
    pub async fn session_middleware(
        State(state): State<SharedAppState>, mut cookies: CookieJar, mut request: Request, next: Next,
    ) -> AppResult<(CookieJar, Response)> {
        if let Some(token) = cookies.get("session") {
            match User::lookup_by_session_token(&state.db, token.value()).await? {
                Some(user) => {
                    request.extensions_mut().insert(user);
                }
                None => cookies = cookies.remove("session"),
            }
        }
        let response = next.run(request).await;
        Ok((cookies, response))
    }
    router.layer(axum::middleware::from_fn_with_state(state, session_middleware))
}
/// Enable extracting an `Option<User>` in a handler.
impl<S: Send + Sync> axum::extract::OptionalFromRequestParts<S> for User {
    type Rejection = Infallible;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Option<Self>, Self::Rejection> {
        Ok(parts.extensions.get::<User>().cloned())
    }
}
/// Enable extracting a `User` in a handler, returning UNAUTHORIZED if not logged in.
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for User {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<User>().cloned() {
            Some(user) => Ok(user),
            None => Err(AppError::Unauthorized),
        }
    }
}

/// Process a login form and send either a login or registration link via email.
async fn login_form(
    State(state): State<SharedAppState>, Form(form): Form<LoginForm>,
) -> AppResult<impl IntoResponse> {
    let email = form.email.email.to_string();
    let Some(user) = User::lookup_by_email(&state.db, &email).await? else {
        return Err(AppError::Unauthorized);
    };

    let email_id = Email::create_login(&state.db, &user).await?;

    let login_token = LoginToken::create(&state.db, &user).await?;
    let base_url = &state.config.app.url;
    let url = match form.redirect {
        Some(redirect) => format!("{base_url}/login?token={login_token}&redirect={redirect}"),
        None => format!("{base_url}/login?token={login_token}"),
    };
    let domain = &state.config.app.domain;

    let msg = state
        .mailer
        .builder()
        .header(ContentType::TEXT_PLAIN)
        .to(form.email)
        .subject(format!("Login to {domain}"))
        .body(format!("Click here to login: {url}"))?;

    match state.mailer.send(&msg).await {
        Ok(_) => {
            Email::mark_sent(&state.db, email_id).await?;
            Ok("Check your email!")
        }
        Err(e) => {
            Email::mark_error(&state.db, email_id, &e.to_string()).await?;
            Err(e)
        }
    }
}
#[derive(serde::Deserialize)]
struct LoginForm {
    email: Mailbox,
    redirect: Option<String>,
}

/// Show the login page or handle a login link
#[derive(serde::Deserialize)]
struct LoginQuery {
    redirect: Option<String>,
    token: Option<String>,
}
impl LoginQuery {
    fn redirect(&self) -> Redirect {
        Redirect::to(match &self.redirect {
            Some(url) => url,
            None => "/",
        })
    }
}
async fn login_link(
    user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<LoginQuery>,
) -> AppResult<Response> {
    // If user is already logged in for some reason, just follow the redirect
    if user.is_some() {
        return Ok(query.redirect().into_response());
    }

    // If there's no token, just show the login page.
    let Some(token) = &query.token else {
        #[derive(Template, WebTemplate)]
        #[template(path = "auth/login.html")]
        struct Html {
            user: Option<User>,
            #[allow(unused)]
            redirect: Option<String>,
        };
        return Ok(Html { user, redirect: query.redirect }.into_response());
    };

    // Otherwise we're handling a login link. Valdiate the login token and create a new session.
    let Some(user) = User::lookup_by_login_token(&state.db, token).await? else {
        return Err(AppError::Unauthorized);
    };
    let token = SessionToken::create(&state.db, &user).await?;
    let cookie = session_cookie(&state.config, token);

    let headers = [(header::SET_COOKIE, cookie)];
    Ok((headers, query.redirect()).into_response())
}

fn session_cookie(config: &Config, token: String) -> String {
    Cookie::build(("session", token))
        .secure(config.acme.is_some())
        .http_only(true)
        .same_site(cookie::SameSite::Strict)
        .domain(&config.app.domain)
        .max_age(cookie::time::Duration::days(config.app.session_expiry_days as i64))
        .to_string()
}

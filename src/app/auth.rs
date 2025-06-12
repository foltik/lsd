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

use axum_extra::extract::CookieJar;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;

use crate::db::token::{LoginToken, SessionToken};
use crate::db::user::UpdateUser;
use crate::prelude::*;
use crate::utils::templates::CONFIG;

fn build_cookie(token: &str) -> (axum::http::HeaderName, String) {
    let config = CONFIG.get().unwrap();
    (
        header::SET_COOKIE,
        cookie::Cookie::build(("session", token))
            .secure(if cfg!(debug_assertions) {
                // Safari won't allow secure cookies
                // coming from localhost in debug mode
                false
            } else {
                // Secure cookies in release mode
                true
            })
            .http_only(true)
            .same_site(cookie::SameSite::Lax)
            .domain(config.app.domain.as_str())
            .max_age(cookie::time::Duration::days(config.app.session_cookie_max_age_days))
            .to_string(),
    )
}

/// Add all `auth` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/login", post(login_form).get(login_link))
         .route("/register", post(register_form).get(register_link))
    })
}

/// Add all `auth` middleware to the router.
pub fn add_middleware(router: AxumRouter, state: SharedAppState) -> AxumRouter {
    router.layer(axum::middleware::from_fn_with_state(state, auth_middleware))
}

/// Middleware layer to lookup add a `User` to the request if a session token is present.
pub async fn auth_middleware(
    State(state): State<SharedAppState>,
    mut cookies: CookieJar,
    mut request: Request,
    next: Next,
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

/// Process a login form and send either a login or registration link via email.
async fn login_form(
    State(state): State<SharedAppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<impl IntoResponse> {
    let email = form.email.email.to_string();
    let login_token = LoginToken::create(&state.db, &email).await?;

    let email_id = Email::create_login(&state.db, &email).await?;

    let domain = &state.config.app.domain;
    let base_url = &state.config.app.url;
    let msg = state.mailer.builder().header(ContentType::TEXT_PLAIN).to(form.email);

    let msg = match User::lookup_by_email(&state.db, &email).await? {
        Some(_) => {
            let url = format!("{base_url}/login?token={login_token}");
            msg.subject(format!("Login to {domain}"))
                .body(format!("Click here to login: {url}"))?
        }
        None => {
            let url = format!("{base_url}/register?token={login_token}");
            msg.subject(format!("Register at {domain}"))
                .body(format!("Click here to complete your registration: {url}"))?
        }
    };

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
}

/// Login from a link containing a token, creating a new sesssion.
async fn login_link(
    State(state): State<SharedAppState>,
    Query(query): Query<LoginQuery>,
) -> AppResult<Response> {
    #[derive(Template, WebTemplate)]
    #[template(path = "auth/login.html")]
    pub struct Html;

    match query.token {
        Some(token) => {
            let user = User::lookup_by_login_token(&state.db, &token)
                .await?
                .ok_or(AppError::NotAuthorized)?;

            let token = SessionToken::create(&state.db, user.id).await?;
            let headers = (
                // TODO: expiration date
                [build_cookie(&token)],
                Redirect::to(&state.config.app.url),
            );
            Ok(headers.into_response())
        }
        None => Ok(Html.into_response()),
    }
}
#[derive(serde::Deserialize)]
struct LoginQuery {
    token: Option<String>,
}

/// Display the registration page.
async fn register_link(Query(query): Query<RegisterQuery>) -> impl IntoResponse {
    #[derive(Template, WebTemplate)]
    #[template(path = "auth/register.html")]
    pub struct Html {
        pub token: String,
    }
    Html { token: query.token }
}
#[derive(serde::Deserialize)]
struct RegisterQuery {
    token: String,
}

/// Process the registration form and create a new user.
async fn register_form(
    State(state): State<SharedAppState>,
    Form(form): Form<RegisterForm>,
) -> AppResult<Response> {
    let Some(email) = LoginToken::lookup_email(&state.db, &form.token).await? else {
        return Err(AppError::NotAuthorized);
    };

    let user_id = User::create(
        &state.db,
        &UpdateUser { first_name: form.first_name, last_name: form.last_name, email },
    )
    .await?;

    // TODO: Expiration date on the cookie
    let session_token = SessionToken::create(&state.db, user_id).await?;
    let headers = ([build_cookie(&session_token)], Redirect::to(&state.config.app.url));
    Ok(headers.into_response())
}
#[derive(serde::Deserialize)]
struct RegisterForm {
    token: String,
    first_name: String,
    last_name: String,
}

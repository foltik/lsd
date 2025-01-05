//! A simple passwordless authentication flow using one-time links sent via email.
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

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::post,
    Form,
};
use lettre::message::Mailbox;

use crate::db::token::{LoginToken, SessionToken};
use crate::db::user::{UpdateUser, User};
use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all auth routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/login", post(login_form).get(login_link))
        .route("/register", post(register_form).get(register_link))
}

/// Process a login form and send either a login or registration link via email.
async fn login_form(
    State(state): State<SharedAppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<impl IntoResponse> {
    let email = form.email.email.to_string();

    let login_token = LoginToken::create(&state.db, &email).await?;

    let url = &state.config.app.url;
    let url = match User::lookup_by_email(&state.db, &email).await? {
        Some(_) => format!("{url}/login?token={login_token}"),
        None => format!("{url}/register?token={login_token}"),
    };

    let msg = state.mail.builder().to(form.email).body(url)?;
    state.mail.send(msg).await?;

    Ok("Check your email!")
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
    let Some(user) = User::lookup_by_login_token(&state.db, &query.token).await? else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    let token = SessionToken::create(&state.db, user.id).await?;
    let headers = (
        // TODO: expiration date
        [(header::SET_COOKIE, format!("session={token}; Secure; Secure"))],
        Redirect::to(&state.config.app.url),
    );
    Ok(headers.into_response())
}
#[derive(serde::Deserialize)]
struct LoginQuery {
    token: String,
}

/// Display the registration page.
async fn register_link(
    State(state): State<SharedAppState>,
    Query(query): Query<RegisterQuery>,
) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("token", &query.token);

    let html = state.templates.render("register.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    let user_id = User::create(
        &state.db,
        &UpdateUser { first_name: form.first_name, last_name: form.last_name, email },
    )
    .await?;

    // TODO: Expiration date on the cookie
    let session_token = SessionToken::create(&state.db, user_id).await?;
    let headers = (
        [(header::SET_COOKIE, format!("session={session_token}; Secure"))],
        Redirect::to(&state.config.app.url),
    );
    Ok(headers.into_response())
}
#[derive(serde::Deserialize)]
struct RegisterForm {
    token: String,
    first_name: String,
    last_name: String,
}

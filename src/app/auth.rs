//! A simple passwordless authentication flow using one-time links sent via email.
//!
//! We choose this scheme instead of one with usernames/passwords to reduce
//! friction and simplify onboarding.
//!
//! # High-level flow
//!
//! 1. **Email input**: User enters their email and submits the login form.
//! 2. **Token generated**: Server creates a short-lived login token and emails it to the user.
//! 3. **Link clicked**: User clicks the link, passing the token back to the server.
//!    - **Login**: If the user is already registered, they get a new session cookie.
//!    - **Registration**: Otherwise, they're prompted to enter their first/last name.
//!      Upon submission, the user is registered and they get a new session cookie.

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use lettre::message::Mailbox;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `auth` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/login", get(login_page).post(login_form))
        .route("/register", get(register_page).post(register_form))
}

/// Display the login page.
async fn login_page(
    State(state): State<SharedAppState>,
    Query(login): Query<LoginQuery>,
) -> AppResult<Response> {
    let Some(user) = state.db.lookup_user_by_login_token(&login.token).await? else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    let session_token = state.db.create_session_token(user.id).await?;
    let headers = (
        // TODO: expiration date
        [(header::SET_COOKIE, format!("session={session_token}; Secure; Secure"))],
        Redirect::to(&state.config.app.url),
    );
    Ok(headers.into_response())
}
#[derive(serde::Deserialize)]
struct LoginQuery {
    token: String,
}

/// Process the login form.
async fn login_form(
    State(state): State<SharedAppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<impl IntoResponse> {
    let login_token = state.db.create_login_token(&form.email).await?;

    let url = &state.config.app.url;
    let url = match state.db.lookup_user_by_email(&form.email).await? {
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

/// Display the registration page.
async fn register_page(
    State(state): State<SharedAppState>,
    Query(register): Query<RegisterQuery>,
) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("token", &register.token);
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
    let Some(email) = state.db.lookup_email_by_login_token(&form.token).await? else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    let user_id = state.db.create_user(&form.first_name, &form.last_name, &email).await?;
    let session_token = state.db.create_session_token(user_id).await?;

    // TODO: Expiration date on the cookie
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

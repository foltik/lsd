use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::utils::{config::*, db::Db, email::Email};
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use serde::Deserialize;
use tera::{Tera, Value};

use anyhow::Result;
use axum::{
    extract::{MatchedPath, Path, Query, Request, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use axum_extra::extract::CookieJar;
use lettre::message::Mailbox;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::Span;

#[derive(Clone)]
#[allow(unused)]
struct AppState {
    config: Config,
    templates: Tera,
    db: Db,
    mail: Email,
}

fn format_datetime(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    // Extract the input string from the value
    let input = value.as_str().ok_or_else(|| tera::Error::msg("Value must be a string"))?;

    // Parse the input string as a NaiveDateTime
    let naive_datetime = NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M")
        .map_err(|_| tera::Error::msg("Failed to parse date"))?;

    // Convert NaiveDateTime to DateTime<Utc>
    let datetime: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_datetime, Utc);

    // Check for a custom format in arguments, or use a default
    let format = args.get("format").and_then(Value::as_str).unwrap_or("%m.%d.%Y");

    // Format the date-time
    let formatted = datetime.format(format).to_string();
    Ok(Value::String(formatted))
}

pub async fn build(config: Config) -> Result<Router> {
    let mut tera_templates = Tera::new("templates/*")?;
    // Register fiters to use while templating
    tera_templates.register_filter("format_datetime", format_datetime);

    let state = AppState {
        config: config.clone(),
        templates: tera_templates,
        db: Db::connect(&config.app.db).await?,
        mail: Email::connect(config.email).await?,
    };

    let router = Router::new()
        .route("/", get(home))
        .route("/login", post(login_form))
        .route("/login", get(login))
        .route("/register", get(register))
        .route("/register", post(register_form))
        .route("/event/create", get(event_create))
        .route("/event/create", post(create_event_form))
        .route("/events", get(event_list))
        .route("/event/:event_id", get(event_update))
        .route("/event/:event_id/update", post(update_event_form))
        .route("/event/:event_id/delete", post(deleting_event))
        .nest_service("/assets", ServeDir::new("assets"))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request<_>| {
                    let path = match req.extensions().get::<MatchedPath>() {
                        Some(path) => path.as_str(),
                        None => req.uri().path(),
                    };
                    tracing::info_span!("request", method = ?req.method(), path, status = tracing::field::Empty)
                })
                .on_request(|_req: &Request<_>, _span: &Span| {})
                .on_response(|res: &Response, latency: Duration, span: &Span| {
                    span.record("status", res.status().as_u16());
                    tracing::info!("handled in {latency:?}");
                }),
        )
        .with_state(Arc::new(state));
    Ok(router)
}

async fn home(State(state): State<Arc<AppState>>, cookies: CookieJar) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("message", "Hello, world!");

    if let Some(session_token) = cookies.get("session") {
        let Some(user) = state.db.lookup_user_from_session_token(session_token.value()).await? else {
            return Ok(StatusCode::FORBIDDEN.into_response());
        };
        ctx.insert("user", &user);
    }

    let html = state.templates.render("home.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

#[derive(serde::Deserialize)]
struct LoginForm {
    email: Mailbox,
}
async fn login_form(
    State(state): State<Arc<AppState>>,
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
struct LoginQuery {
    token: String,
}
async fn login(State(state): State<Arc<AppState>>, Query(login): Query<LoginQuery>) -> AppResult<Response> {
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
struct RegisterQuery {
    token: String,
}
async fn register(
    State(state): State<Arc<AppState>>,
    Query(register): Query<RegisterQuery>,
) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("token", &register.token);
    let html = state.templates.render("register.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

#[derive(serde::Deserialize)]
struct RegisterForm {
    token: String,
    first_name: String,
    last_name: String,
}
async fn register_form(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RegisterForm>,
) -> AppResult<Response> {
    let Some(email) = state.db.lookup_email_by_login_token(&form.token).await? else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };

    let user_id = state.db.create_user(&form.first_name, &form.last_name, &email).await?;
    let session_token = state.db.create_session_token(user_id).await?;

    // TODO: expiration date on the cookie
    let headers = (
        [(header::SET_COOKIE, format!("session={session_token}; Secure; Secure"))],
        Redirect::to(&state.config.app.url),
    );
    Ok(headers.into_response())
}

#[derive(Deserialize)]
struct EventsParam {
    #[serde(default = "default_to_false")]
    past: bool,
}
fn default_to_false() -> bool {
    false
}
async fn event_list(
    State(state): State<Arc<AppState>>,
    Query(param): Query<EventsParam>,
) -> AppResult<Response> {
    let mut ctx = tera::Context::new();

    let events = state
        .db
        .get_all_events(Local::now().format("fmt").to_string(), param.past)
        .await?;
    ctx.insert("events", &events);
    let html = state.templates.render("event-list.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

async fn event_create(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let ctx = tera::Context::new();
    let html = state.templates.render("event-create.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

#[derive(serde::Deserialize)]
struct EventCreateForm {
    title: String,
    artist: String,
    description: String,
    start_date: String,
}
async fn create_event_form(
    State(state): State<Arc<AppState>>,
    Form(form): Form<EventCreateForm>,
) -> AppResult<impl IntoResponse> {
    let _event_id = state
        .db
        .create_event(&form.title, &form.artist, &form.description, &form.start_date)
        .await?;
    Ok("Event created.")
}

async fn event_update(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    let Some(event) = state.db.lookup_event_by_event_id(&event_id.parse().unwrap()).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    ctx.insert("event", &event);

    let html = state.templates.render("event.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

#[derive(serde::Deserialize)]
struct EventUpdateForm {
    title: String,
    artist: String,
    description: String,
    start_date: String,
}
async fn update_event_form(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    Form(form): Form<EventUpdateForm>,
) -> AppResult<impl IntoResponse> {
    state
        .db
        .update_event(
            event_id.parse().unwrap(),
            &form.title,
            &form.artist,
            &form.description,
            &form.start_date,
        )
        .await?;
    Ok("Event updated.")
}

async fn deleting_event(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    state.db.delete_event(event_id.parse().unwrap()).await?;
    Ok("Event deleted.")
}

struct AppError(anyhow::Error);
type AppResult<T> = Result<T, AppError>;
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // TODO: add a `dev` mode to `config.app`, and:
        // * when enabled, respond with a stack trace
        // * when disabled, respond with a generic error message that doesn't leak any details
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", self.0)).into_response()
    }
}
impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

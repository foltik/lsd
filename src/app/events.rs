use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Form,
};
use chrono::Local;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `events` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/events", get(list_events_page))
        .route("/e/new", get(create_event_page).post(create_event_form))
        .route(
            "/e/:event_id",
            // TODO: Move to a separate `/e/:event_id/edit` route, and add a `/e/:event_id` to just view the event.
            get(update_event_page).post(update_event_form).delete(delete_event),
        )
}

/// Display a list of all events.
async fn list_events_page(
    State(state): State<SharedAppState>,
    Query(param): Query<ListEvents>,
) -> AppResult<Response> {
    let events = state.db.get_all_events(Local::now(), param.past.unwrap_or(false)).await?;

    let mut ctx = tera::Context::new();
    ctx.insert("events", &events);

    let html = state.templates.render("event-list.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}
#[derive(serde::Deserialize)]
struct ListEvents {
    past: Option<bool>,
}

/// Display the form to create a new event.
async fn create_event_page(State(state): State<SharedAppState>) -> AppResult<Response> {
    let ctx = tera::Context::new();
    let html = state.templates.render("event-create.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Process the form and create a new event.
async fn create_event_form(
    State(state): State<SharedAppState>,
    Form(form): Form<CreateEvent>,
) -> AppResult<impl IntoResponse> {
    let _event_id = state
        .db
        .create_event(&form.title, &form.artist, &form.description, &form.start_date)
        .await?;
    Ok("Event created.")
}
#[derive(serde::Deserialize)]
struct CreateEvent {
    title: String,
    artist: String,
    description: String,
    start_date: String,
}

/// Display the form to update an event.
async fn update_event_page(
    State(state): State<SharedAppState>,
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

/// Process the form and update an event.
async fn update_event_form(
    State(state): State<SharedAppState>,
    Path(event_id): Path<String>,
    Form(form): Form<UpdateEvent>,
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
#[derive(serde::Deserialize)]
struct UpdateEvent {
    title: String,
    artist: String,
    description: String,
    start_date: String,
}

/// Delete an event.
async fn delete_event(
    State(state): State<SharedAppState>,
    Path(event_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    state.db.delete_event(event_id.parse().unwrap()).await?;
    Ok("Event deleted.")
}

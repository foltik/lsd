use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use chrono::Local;

use crate::db::event::{Event, UpdateEvent};
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
    Query(query): Query<ListEventsQuery>,
) -> AppResult<Response> {
    let now = Local::now();
    let past = query.past.unwrap_or(false);

    let events = Event::list(&state.db)
        .await?
        .into_iter()
        .filter(|e| match past {
            true => e.start_date < now,
            // TODO: Don't filter out in-progress events until they're over.
            // Need to add an `end_date` field.
            false => e.start_date >= now,
        })
        .collect::<Vec<_>>();

    let mut ctx = tera::Context::new();
    ctx.insert("events", &events);

    let html = state.templates.render("event-list.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}
#[derive(serde::Deserialize)]
struct ListEventsQuery {
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
    Form(form): Form<UpdateEvent>,
) -> AppResult<impl IntoResponse> {
    let _ = Event::create(&state.db, &form).await?;
    // TODO: Redirect to event page.
    Ok("Event created.")
}

/// Display the form to update an event.
async fn update_event_page(State(state): State<SharedAppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut ctx = tera::Context::new();
    ctx.insert("event", &event);

    let html = state.templates.render("event.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Process the form and update an event.
async fn update_event_form(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateEvent>,
) -> AppResult<impl IntoResponse> {
    Event::update(&state.db, id, &form).await?;
    // TODO: Redirect to event page.
    Ok("Event updated.")
}

/// Delete an event.
async fn delete_event(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    Event::delete(&state.db, id).await?;
    Ok(Redirect::to("/events"))
}

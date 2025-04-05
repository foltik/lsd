use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use chrono::Utc;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};
use crate::{
    db::event::{Event, UpdateEvent},
    views,
};

/// Add all `events` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/events", get(list_events_page))
        .route("/event/new", get(create_event_page).post(create_event_form))
        .route(
            "/event/{id}",
            // TODO: Move to a separate `/e/{id}/edit` route, and add a `/e/{id}` to just view the event.
            get(update_event_page).post(update_event_form).delete(delete_event),
        )
}

/// Display a list of all events.
async fn list_events_page(
    State(state): State<SharedAppState>,
    Query(query): Query<ListEventsQuery>,
) -> AppResult<Response> {
    let now = Utc::now().naive_utc();
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

    let list_template = views::events::EventList { events };

    Ok(Html(list_template.render()?).into_response())
}
#[derive(serde::Deserialize)]
struct ListEventsQuery {
    past: Option<bool>,
}

/// Display the form to create a new event.
async fn create_event_page() -> AppResult<Response> {
    Ok(Html(views::events::EventCreate.render()?).into_response())
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

    let update_template = views::events::EventView { event: Some(event) };

    Ok(Html(update_template.render()?).into_response())
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

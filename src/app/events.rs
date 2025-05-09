use crate::db::event::{Event, UpdateEvent};
use crate::prelude::*;

/// Add all `events` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/events", get(list_events_page))
            .route("/events/new", get(create_event_page).post(create_event_form))
            .route(
                "/events/{id}",
                // TODO: Move to a separate `/e/{id}/edit` route, and add a `/e/{id}` to just view the event.
                get(update_event_page).post(update_event_form).delete(delete_event),
            )
    })
}

/// Display a list of all events.
async fn list_events_page(
    State(state): State<SharedAppState>,
    Query(query): Query<ListEventsQuery>,
) -> AppResult<impl IntoResponse> {
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

    #[derive(Template, WebTemplate)]
    #[template(path = "events/list.html")]
    pub struct Html {
        pub events: Vec<Event>,
    }
    Ok(Html { events })
}
#[derive(serde::Deserialize)]
struct ListEventsQuery {
    past: Option<bool>,
}

/// Display the form to create a new event.
async fn create_event_page() -> impl IntoResponse {
    #[derive(Template, WebTemplate)]
    #[template(path = "events/create.html")]
    pub struct Html;
    Html
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
async fn update_event_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/view.html")]
    pub struct Html {
        pub event: Event,
    }
    Ok(Html { event })
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

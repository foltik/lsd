use crate::db::event::{Event, UpdateEvent};
use crate::db::event_ticket::EventTicket;
use crate::db::ticket::Ticket;
use crate::prelude::*;

/// Add all `events` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/events", get(list_events_page))
            .route("/events/new", get(create_event_page).post(create_event_form))
            .route("/events/{id}", get(view_event_page))
            .route("/events/{id}/edit", get(edit_event_page).post(update_event_form))
            .route("/events/{id}/delete", post(delete_event))
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
            true => e.start < now,
            // TODO: Don't filter out in-progress events until they're over.
            // Need to add an `end_date` field.
            false => e.start >= now,
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

#[derive(serde::Deserialize, Debug)]
struct CreateEventWithTickets {
    #[serde(flatten)]
    pub event: UpdateEvent,
    pub tickets: Vec<EventTicketForm>,
    pub cover_image: Option<String>,
    pub cover_image_filename: Option<String>,
}

#[derive(serde::Deserialize)]
struct UpdateEventWithTickets {
    #[serde(flatten)]
    pub event: UpdateEvent,
    pub tickets: Vec<EventTicketForm>,
    pub cover_image: Option<String>,
    pub cover_image_filename: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct EventTicketForm {
    pub ticket_id: i64,
    pub price: i64,
    pub quantity: i64,
    pub sort: i64,
}

/// Display the form to create a new event with ticket type selection.
async fn create_event_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    let tickets = Ticket::list(&state.db).await?;
    #[derive(Template, WebTemplate)]
    #[template(path = "events/create.html")]
    pub struct Html {
        pub tickets: Vec<Ticket>,
    }
    Ok(Html { tickets })
}

/// Process the form and create a new event with tickets.
async fn create_event_form(
    State(state): State<SharedAppState>,
    Json(form): Json<CreateEventWithTickets>,
) -> AppResult<impl IntoResponse> {
    println!("Raw form data: {:?}", form);

    //Handle cover image processing
    let processed_flyer = if let Some(base64_image) = &form.cover_image {
        match process_image(base64_image, &form.cover_image_filename).await {
            Ok(path) => {
                println!("Successfully processed image, saved to: {}", path);
                Some(path)
            }
            Err(e) => {
                println!("Error processing image: {:?}", e);
                return Err(e);
            }
        }
    } else {
        println!("No cover image provided");
        None
    };

    //Create event with processed cover image path
    let mut event = form.event.clone();
    event.flyer = processed_flyer.clone();

    let event_id = Event::create(&state.db, &event).await?;

    sqlx::query!("DELETE FROM event_tickets WHERE event_id = ?", event_id)
        .execute(&state.db)
        .await?;

    for ticket_form in form.tickets {
        if ticket_form.ticket_id > 0
            && ticket_form.price > 0
            && ticket_form.quantity > 0
            && ticket_form.sort >= 0
        {
            EventTicket::create(
                &state.db,
                event_id,
                ticket_form.ticket_id,
                ticket_form.price,
                ticket_form.quantity,
                ticket_form.sort,
            )
            .await?;
        }
    }
    Ok(Redirect::to("/events"))
}

/// Display the event details (read-only).
async fn view_event_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;
    let event_tickets = EventTicket::list_for_event(&state.db, id).await?;
    let all_tickets = Ticket::list(&state.db).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/view.html")]
    pub struct Html {
        pub event: Event,
        pub event_tickets: Vec<EventTicket>,
        pub all_tickets: Vec<Ticket>,
    }
    Ok(Html { event, event_tickets, all_tickets })
}

/// Display the form to edit an existing event.
async fn edit_event_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;
    let event_tickets = EventTicket::list_for_event(&state.db, id).await?;
    let all_tickets = Ticket::list(&state.db).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    pub struct Html {
        pub event: Event,
        pub event_tickets: Vec<EventTicket>,
        pub all_tickets: Vec<Ticket>,
    }
    Ok(Html { event, event_tickets, all_tickets })
}

/// Process the form and update an event.
async fn update_event_form(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(form): Json<UpdateEventWithTickets>,
) -> AppResult<impl IntoResponse> {
    // Get existing event to preserve flyer if no new image
    let existing_event = Event::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    let processed_flyer = if let Some(base64_image) = &form.cover_image {
        println!("Processing new cover image");

        // Delete old image if it exists
        if let Some(old_flyer) = &existing_event.flyer {
            if let Err(e) = delete_image(old_flyer).await {
                println!("Warning: Failed to delete old image {}: {:?}", old_flyer, e);
            }
        }

        match process_image(base64_image, &form.cover_image_filename).await {
            Ok(path) => {
                println!("Successfully processed new image: {}", path);
                Some(path)
            }
            Err(e) => {
                println!("Error processing new image: {:?}", e);
                return Err(e);
            }
        }
    } else {
        println!("No new image provided, keeping existing: {:?}", existing_event.flyer);
        existing_event.flyer
    };

    let mut event = form.event;
    event.flyer = processed_flyer.clone();

    println!("Updating event with flyer: {:?}", event.flyer);

    Event::update(&state.db, id, &event).await?;

    // Delete existing event tickets first to prevent duplicates
    sqlx::query!("DELETE FROM event_tickets WHERE event_id = ?", id)
        .execute(&state.db)
        .await?;

    for ticket_form in form.tickets {
        if ticket_form.ticket_id > 0 && ticket_form.price > 0 && ticket_form.quantity > 0 {
            EventTicket::create(
                &state.db,
                id,
                ticket_form.ticket_id,
                ticket_form.price,
                ticket_form.quantity,
                ticket_form.sort,
            )
            .await?;
        }
    }
    // TODO: Redirect to event page.
    Ok(Redirect::to(&format!("/events/{}", id)))
}

/// Delete an event.
async fn delete_event(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    // Check if event exists first
    let existing_event = Event::lookup_by_id(&state.db, id).await?;
    if existing_event.is_none() {
        println!("DELETE EVENT: Event {} not found", id);
        return Err(AppError::NotFound);
    }

    let event = existing_event.unwrap();

    // Delete associated image file if it exists
    if let Some(flyer_path) = &event.flyer {
        if let Err(e) = delete_image(flyer_path).await {
            println!("Warning: Failed to delete image file {}: {:?}", flyer_path, e);
        } else {
            println!("Successfully deleted image file: {}", flyer_path);
        }
    }

    println!("DELETE EVENT: Event {} found, proceeding with deletion", id);
    Event::delete(&state.db, id).await?;
    println!("DELETE EVENT: Event {} deleted successfully, redirecting to /events", id);
    Ok(Redirect::to("/events"))
}

use crate::db::event::{Event, UpdateEvent};
use crate::db::event_ticket::EventTicket;
use crate::db::event_ticket::UpdateEventTicket;
use crate::db::ticket::Ticket;
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
}

#[derive(serde::Deserialize)]
struct UpdateEventWithTickets {
    #[serde(flatten)]
    pub event: UpdateEvent,
    pub tickets: Vec<EventTicketForm>,
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
    Form(form): Form<CreateEventWithTickets>,
) -> AppResult<impl IntoResponse> {
    println!("Raw form data: {:?}", form);
    let event_id = Event::create(&state.db, &form.event).await?;
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

/// Display the form to update an event.
async fn update_event_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;
    let event_tickets = EventTicket::list_for_event(&state.db, id).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/view.html")]
    pub struct Html {
        pub event: Event,
        pub event_tickets: Vec<EventTicket>,
    }
    Ok(Html { event, event_tickets })
}

/// Process the form and update an event.
async fn update_event_form(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateEventWithTickets>,
) -> AppResult<impl IntoResponse> {
    Event::update(&state.db, id, &form.event).await?;
    for ticket_form in form.tickets {
        if ticket_form.ticket_id > 0 {
            EventTicket::update(
                &state.db,
                id,
                ticket_form.ticket_id,
                &UpdateEventTicket {
                    price: ticket_form.price,
                    quantity: ticket_form.quantity,
                    sort: ticket_form.sort,
                },
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
    Event::delete(&state.db, id).await?;
    Ok(Redirect::to("/events"))
}

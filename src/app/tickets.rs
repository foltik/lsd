use crate::db::ticket::{Ticket, UpdateTicket};
use crate::prelude::*;

/// Add all `ticket` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/tickets", get(list_tickets_page))
          .route("/tickets/new", get(create_ticket_page).post(create_ticket_form))
          .route("/tickets/{id}", get(view_ticket_page).delete(delete_ticket))
          .route("/tickets/{id}/edit", get(update_ticket_page).post(update_ticket_form))
          .route("/tickets/{id}/delete", post(delete_ticket),
            )
        })
}

/// Display a list of all tickets.
async fn list_tickets_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    let tickets = Ticket::list(&state.db).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "tickets/list.html")]
    struct Html {
        pub tickets: Vec<Ticket>,
    }
    Ok(Html { tickets })
}

/// Display the form to create a new ticket.
async fn create_ticket_page() -> impl IntoResponse {
    #[derive(Template, WebTemplate)]
    #[template(path = "tickets/create.html")]
    struct Html;
    Html
}

/// Process the form and create a new ticket.
async fn create_ticket_form(
    State(state): State<SharedAppState>,
    Form(form): Form<UpdateTicket>,
) -> AppResult<impl IntoResponse> {
    let _ = Ticket::create(&state.db, &form).await?;
    Ok(Redirect::to("/tickets"))
}

async fn view_ticket_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let ticket = Ticket::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    #[derive(Template, WebTemplate)]
    #[template(path = "tickets/view.html")]
    pub struct Html {
        pub ticket: Ticket,
    }
    Ok(Html { ticket })
}

/// Display the form to update a ticket.
async fn update_ticket_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let ticket = Ticket::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    #[derive(Template, WebTemplate)]
    #[template(path = "tickets/view.html")]
    pub struct Html {
        pub ticket: Ticket,
    }
    Ok(Html { ticket })
}

/// Process the form and update a ticket.
async fn update_ticket_form(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateTicket>,
) -> AppResult<impl IntoResponse> {
    Ticket::update(&state.db, id, &form).await?;
    Ok("Ticket updated.")
}

/// Delete a ticket.
async fn delete_ticket(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    Ticket::delete(&state.db, id).await?;
    Ok(Redirect::to("/tickets"))
}

use crate::db::event::*;
use crate::db::list::*;
use crate::db::ticket::*;
use crate::prelude::*;

/// Add all `events` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/rsvp", get(read::rsvp_page))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/events", get(read::list_page))
                .route("/events/new", get(edit::new_page))
                .route("/events/{slug}/edit", get(edit::edit_page).post(edit::edit_form))
                .route("/events/{slug}/delete", post(edit::delete_form))
        })
}

// RETURN URL:
// /events/{}/confirm?session_id={{CHECKOUT_SESSION_ID}}\

// async fn checkout_session(
//     // user: Option<User>,
//     Path(id): Path<i64>,
//     State(state): State<SharedAppState>,
//     Json(CheckoutSessionData { price }): Json<CheckoutSessionData>,
// ) -> AppResult<String> {
//     let price_cents = price * 100;

//     // Gross but there doesn't seem to be any other supported way to build form data in the way
//     // that stripe expects in particular for lists of objects.
//     // The v2 APIs will allow sending JSON data but currently checkout API doesn't support v2
//     // as of 2025-06-10.
//     // See https://docs.stripe.com/api/checkout/sessions/create?api-version=2025-05-28.basil
//     let form_data = format!("ui_mode=custom&return_url={}/events/{}/confirm?session_id={{CHECKOUT_SESSION_ID}}&mode=payment&currency=usd&allow_promotion_codes=true&payment_method_types[]=card&payment_method_types[]=cashapp&line_items[0][quantity]=1&line_items[0][price_data][currency]=usd&line_items[0][price_data][unit_amount]={}&line_items[0][price_data][product_data][name]=Contribution&line_items[0][price_data][product_data][description]=Contribute to this event", state.config.app.url, id,price_cents);

//     // if let Some(user) = user {
//     //     form_data = format!("{}&customer_email={}", form_data, user.email);
//     // }
//     //

//     let res = state
//         .http
//         .post("https://api.stripe.com/v1/checkout/sessions")
//         .header(
//             header::AUTHORIZATION,
//             format!("Bearer {}", state..stripe_secret_key.expose_secret()),
//         )
//         .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
//         .body(form_data)
//         .send()
//         .await
//         .unwrap();

//     tracing::info!("{res:?}");

//     let json: serde_json::Value = res.json().await?;
//     let client_secret = json["client_secret"]
//         .as_str()
//         .context("Cannot parse client_secret as string")?
//         .to_string();

//     Ok(client_secret)
// }

// View and list events.
mod read {
    use super::*;

    /// View an event.
    pub async fn view_page(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        #[derive(Template, WebTemplate)]
        #[template(path = "events/view.html")]
        struct Html {
            event: Event,
        }
        Ok(Html {
            event: Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?,
        })
    }

    // List all events.
    pub async fn list_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        #[derive(Template, WebTemplate)]
        #[template(path = "events/list.html")]
        pub struct Html {
            pub events: Vec<Event>,
        }
        Ok(Html { events: Event::list(&state.db).await? })
    }

    // RSVP for an event.
    pub async fn rsvp_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        if user.is_none() && event.guest_list_id.is_some() {
            return Ok(Redirect::to(&format!("/login?redirect=/e/{slug}/rsvp")).into_response());
        }

        let tickets = Ticket::list_for_event_with_stats(&state.db, event.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp.html")]
        struct Html {
            user: Option<User>,
            event: Event,
            tickets: Vec<TicketWithStats>,
        }
        Ok(Html { user, event, tickets }.into_response())
    }
}

// Create and edit events.
mod edit {
    use super::*;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    pub struct EditHtml {
        pub event: Event,
        pub tickets: Vec<TicketWithStats>,
        pub lists: Vec<ListWithCount>,
    }

    /// Display the form to create a new event.
    pub async fn new_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        Ok(EditHtml {
            lists: List::list_with_counts(&state.db).await?,
            event: Event {
                id: 0,
                title: "".into(),
                slug: "".into(),
                description: "".into(),
                flyer: None,

                start: Utc::now().naive_utc(),
                end: None,

                unlisted: false,
                guest_list_id: None,
                target_revenue: None,

                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            tickets: vec![],
        })
    }

    /// Display the form to edit an event.
    pub async fn edit_page(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let tickets = Ticket::list_for_event_with_stats(&state.db, event.id).await?;
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml { event, tickets, lists })
    }

    // Handle edit submission.
    #[derive(Debug, serde::Deserialize)]
    pub struct EditForm {
        id: Option<i64>,
        #[serde(flatten)]
        event: UpdateEvent,
        tickets: Vec<UpdateTicket>,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>,
        Json(form): Json<EditForm>,
    ) -> AppResult<impl IntoResponse> {
        match form.id {
            Some(id) => {
                Event::update(&state.db, id, &form.event).await?;

                let mut to_add = vec![];
                let mut to_delete = Ticket::list_ids_for_event(&state.db, id).await?;

                for ticket in form.tickets {
                    match ticket.id {
                        Some(id) => {
                            Ticket::update(&state.db, id, &ticket).await?;
                            to_delete.retain(|&id_| id_ != id);
                        }
                        None => {
                            let id = Ticket::create(&state.db, &ticket).await?;
                            to_add.push(id);
                        }
                    }
                }

                Ticket::add_to_event(&state.db, id, to_add).await?;
                Ticket::remove_from_event(&state.db, id, to_delete).await?;
            }
            None => {
                let event_id = Event::create(&state.db, &form.event).await?;

                let mut ticket_ids = vec![];
                for ticket in form.tickets {
                    let id = Ticket::create(&state.db, &ticket).await?;
                    ticket_ids.push(id);
                }

                Ticket::add_to_event(&state.db, event_id, ticket_ids).await?;
            }
        }
        Ok(())
    }

    /// Handle delete submission.
    pub async fn delete_form(
        State(state): State<SharedAppState>,
        Path(id): Path<i64>,
    ) -> AppResult<impl IntoResponse> {
        Event::delete(&state.db, id).await?;
        Ok(Redirect::to("/events"))
    }
}

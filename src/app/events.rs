use crate::db::event::*;
use crate::db::spot::*;
use crate::prelude::*;
use crate::utils::stripe;

/// Add all `events` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/rsvp", get(rsvp::rsvp_page).post(rsvp::rsvp_form))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/events", get(read::list_page))
                .route("/events/new", get(edit::new_page))
                .route("/events/{slug}/edit", get(edit::edit_page).post(edit::edit_form))
                .route("/events/{slug}/delete", post(edit::delete_form))
        })
}

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
    #[derive(Template, WebTemplate)]
    #[template(path = "events/list.html")]
    pub struct ListHtml {
        pub events: Vec<Event>,
    }

    pub async fn list_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        Ok(ListHtml { events: Event::list(&state.db).await? })
    }
}

// Create and edit events.
mod edit {
    use super::*;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    pub struct EditHtml {
        pub event: Event,
        pub spots: Vec<Spot>,
    }

    /// Display the form to create a new event.
    pub async fn new_page() -> AppResult<impl IntoResponse> {
        Ok(EditHtml {
            event: Event {
                id: 0,
                title: "".into(),
                slug: "".into(),
                description: "".into(),

                start: Utc::now().naive_utc(),
                end: None,

                capacity: 0,
                unlisted: false,

                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            spots: vec![],
        })
    }

    /// Display the form to edit an event.
    pub async fn edit_page(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        Ok(EditHtml { event, spots })
    }

    // Handle edit submission.
    #[derive(Debug, serde::Deserialize)]
    pub struct EditForm {
        id: Option<i64>,
        #[serde(flatten)]
        event: UpdateEvent,
        spots: Vec<UpdateSpot>,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>,
        Json(form): Json<EditForm>,
    ) -> AppResult<impl IntoResponse> {
        match form.id {
            Some(id) => {
                Event::update(&state.db, id, &form.event).await?;

                let mut to_add = vec![];
                let mut to_delete = Spot::list_ids_for_event(&state.db, id).await?;

                for spot in form.spots {
                    match spot.id {
                        Some(id) => {
                            Spot::update(&state.db, id, &spot).await?;
                            to_delete.retain(|&id_| id_ != id);
                        }
                        None => {
                            let id = Spot::create(&state.db, &spot).await?;
                            to_add.push(id);
                        }
                    }
                }

                Spot::add_to_event(&state.db, id, to_add).await?;
                Spot::remove_from_event(&state.db, id, to_delete).await?;
            }
            None => {
                let event_id = Event::create(&state.db, &form.event).await?;

                let mut spot_ids = vec![];
                for spot in form.spots {
                    let id = Spot::create(&state.db, &spot).await?;
                    spot_ids.push(id);
                }

                Spot::add_to_event(&state.db, event_id, spot_ids).await?;
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

mod rsvp {
    use super::*;

    // RSVP to an event.
    pub async fn rsvp_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let stats = event.stats(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp.html")]
        struct RsvpHtml {
            user: Option<User>,
            event: Event,
            spots: Vec<Spot>,
            stats: EventStats,
        }
        Ok(RsvpHtml { user, event, spots, stats }.into_response())
    }

    // Handle RSVP submission.
    #[derive(Debug, serde::Deserialize)]
    pub struct RsvpForm {
        reservations: String,
    }
    #[derive(Debug, serde::Deserialize)]
    pub struct Reservation {
        id: i64,
        qty: i64,
        contribution: Option<i64>,
    }
    pub async fn rsvp_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Form(form): Form<RsvpForm>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let stats = event.stats(&state.db).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        let reservations: Vec<Reservation> =
            serde_json::from_str(&form.reservations).map_err(|_| AppError::BadRequest)?;

        if let Err(e) = validate_reservations(&stats, &spots, &reservations) {
            tracing::info!("{e}");
            return Err(AppError::BadRequest);
        }

        let line_items = make_line_items(&spots, &reservations);
        let stripe_checkout_client_secret = state
            .stripe
            .create_checkout_session(&user, line_items, format!("/e/{slug}/confirmation"))
            .await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/checkout.html")]
        struct CheckoutHtml {
            stripe_publishable_key: String,
            stripe_checkout_client_secret: String,
        }
        Ok(CheckoutHtml {
            stripe_checkout_client_secret,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        }
        .into_response())
    }

    #[derive(thiserror::Error, Debug)]
    pub enum ValidationError {
        #[error("unknown spot_id={id}")]
        UnknownSpot { id: i64 },

        #[error("number of reservations exceeds event capacity")]
        EventCapacity,
        #[error("number of reservations exceeds capacity for spot_id={id}")]
        SpotCapacity { id: i64 },

        #[error("contribution is outside of range for spot_id={id}")]
        SpotRange { id: i64 },
    }
    fn validate_reservations(
        stats: &EventStats,
        spots: &[Spot],
        reservations: &[Reservation],
    ) -> Result<(), ValidationError> {
        let mut reserved_qty = 0;
        for res in reservations {
            let id = res.id;
            let Some(spot) = spots.iter().find(|s| s.id == id) else {
                return Err(ValidationError::UnknownSpot { id });
            };

            if res.qty > *stats.remaining_spots.get(&res.id).unwrap_or(&0) {
                return Err(ValidationError::SpotCapacity { id });
            }
            if spot.kind == Spot::VARIABLE {
                let min = spot.min_contribution.unwrap();
                let max = spot.max_contribution.unwrap();
                if !res.contribution.is_some_and(|c| (min..=max).contains(&c)) {
                    return Err(ValidationError::SpotRange { id });
                }
            }
            reserved_qty += res.qty;
        }

        if reserved_qty > stats.remaining_capacity {
            return Err(ValidationError::EventCapacity);
        }

        Ok(())
    }

    fn make_line_items(spots: &[Spot], reservations: &[Reservation]) -> Vec<stripe::LineItem> {
        let mut items = vec![];
        for res in reservations {
            let spot = spots.iter().find(|s| s.id == res.id).unwrap(); // unwrap(): we've already validated
            items.push(stripe::LineItem {
                name: spot.name.clone(),
                quantity: res.qty,
                price: match spot.kind.as_str() {
                    Spot::FREE => 0,
                    Spot::FIXED => spot.required_contribution.unwrap(),
                    Spot::VARIABLE => res.contribution.unwrap(),
                    Spot::WORK => 0,
                    kind => panic!("unknown kind: {kind}"),
                },
            });
        }
        items
    }
}

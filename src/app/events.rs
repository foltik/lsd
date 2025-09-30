use crate::db::event::*;
use crate::db::spot::*;
use crate::prelude::*;
use crate::utils::stripe;

/// Add all `events` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/rsvp", post(rsvp::rsvp_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::rsvp_selection_page).post(rsvp::rsvp_selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::rsvp_attendees_page).post(rsvp::rsvp_attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::rsvp_contribution_page))
                .route("/e/{slug}/rsvp/confirmation", get(rsvp::rsvp_confirmation_page))
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
    use crate::db::rsvp::{CreateRsvp, Rsvp};
    use crate::db::rsvp_sessions::RsvpSession;

    /// Token passed between RSVP steps for idempotency.
    #[derive(Debug, serde::Deserialize)]
    pub struct RsvpSessionQuery {
        session: String,
    }
    /// Token plus stripe secret.
    #[derive(Debug, serde::Deserialize)]
    pub struct RsvpSessionStripeQuery {
        session: String,
        secret: String,
    }

    /// Create an RSVP session after a user clicks the RSVP button for an event.
    pub async fn rsvp_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let token = RsvpSession::create(&state.db, event.id, &user).await?;
        Ok(Redirect::to(&format!("/e/{slug}/rsvp/selection?session={token}")))
    }

    // Display the "Choose a contribution" page
    pub async fn rsvp_selection_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let stats = event.stats(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_selection.html")]
        struct SelectionHtml {
            slug: String,
            user: Option<User>,
            spots: Vec<Spot>,
            stats: EventStats,
        }
        Ok(SelectionHtml { slug, user, spots, stats }.into_response())
    }

    // Handle submission of the "Choose a contribution" page
    #[derive(Debug, serde::Deserialize)]
    pub struct RsvpSelectionForm {
        attendees: String,
    }
    pub async fn rsvp_selection_form(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<RsvpSessionQuery>,
        Form(form): Form<RsvpSelectionForm>,
    ) -> AppResult<impl IntoResponse> {
        let session = RsvpSession::lookup_by_token(&state.db, &query.session)
            .await?
            .ok_or(AppError::BadRequest)?;

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let stats = event.stats(&state.db).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        // Sending structured data through a form submission is pain, and axum doesn't even
        // support deserializing serde types from the several methods that exist... we just use a string.
        let rsvps =
            parse_rsvp_selection(&stats, &spots, &form.attendees).map_err(|_| AppError::BadRequest)?;

        // Delete any pending RSVPs and create new ones.
        // Needed in case the user presses back and changes their selection.
        Rsvp::delete_for_session(&state.db, session.id).await?;
        for rsvp in rsvps {
            Rsvp::create(
                &state.db,
                CreateRsvp {
                    event_id: event.id,
                    spot_id: rsvp.spot_id,
                    contribution: rsvp.contribution,
                    status: "pending".into(),
                    session_id: session.id,
                    user_id: None,
                },
            )
            .await?;
        }

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/attendees?session={}", session.token)))
    }

    // Display the "Who will be attending?" page after submitting spots
    pub async fn rsvp_attendees_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<RsvpSessionQuery>,
    ) -> AppResult<impl IntoResponse> {
        let session = RsvpSession::lookup_by_token(&state.db, &query.session)
            .await?
            .ok_or(AppError::BadRequest)?;

        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_attendees.html")]
        struct AttendeesHtml {
            slug: String,
            user: Option<User>,
            rsvps: Vec<Rsvp>,
        }
        Ok(AttendeesHtml { slug, user, rsvps })
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct RsvpAttendeesForm {
        attendees: String,
    }
    pub async fn rsvp_attendees_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<RsvpSessionQuery>,
    ) -> AppResult<impl IntoResponse> {
        let session = RsvpSession::lookup_by_token(&state.db, &query.session)
            .await?
            .ok_or(AppError::BadRequest)?;

        let line_items = session.line_items(&state.db).await?;
        let return_url = format!("/e/{slug}/rsvp/confirmation?session={}", session.id);
        let stripe_client_secret = state.stripe.create_session(&user, line_items, return_url).await?;

        Ok(Redirect::to(&format!(
            "/e/{slug}/rsvp/contribution?session={}&secret={}",
            session.token, stripe_client_secret
        )))
    }

    pub async fn rsvp_contribution_page(
        State(state): State<SharedAppState>,
        Query(query): Query<RsvpSessionStripeQuery>,
    ) -> AppResult<impl IntoResponse> {
        let session = RsvpSession::lookup_by_token(&state.db, &query.session)
            .await?
            .ok_or(AppError::BadRequest)?;

        let line_items = session.line_items(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_contribution.html")]
        struct ContributionHtml {
            line_items: Vec<stripe::LineItem>,
            stripe_publishable_key: String,
            stripe_client_secret: String,
        }
        Ok(ContributionHtml {
            line_items,
            stripe_client_secret: query.secret,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        })
    }

    pub async fn rsvp_confirmation_page() -> AppResult<impl IntoResponse> {
        Ok(Redirect::to("/"))
    }

    pub struct ParsedRsvp {
        spot_id: i64,
        contribution: i64,
    }
    #[derive(thiserror::Error, Debug)]
    pub enum ParseRsvpSelectionError {
        #[error("failed to parse request JSON")]
        Parse,

        #[error("unknown spot_id={spot_id}")]
        UnknownSpot { spot_id: i64 },

        #[error("number of rsvps exceeds event capacity")]
        EventCapacity,
        #[error("number of rsvps exceeds capacity for spot_id={spot_id}")]
        SpotCapacity { spot_id: i64 },

        #[error("contribution is outside of range for spot_id={spot_id}")]
        SpotRange { spot_id: i64 },
    }
    fn parse_rsvp_selection(
        stats: &EventStats,
        spots: &[Spot],
        rsvps: &str,
    ) -> Result<Vec<ParsedRsvp>, ParseRsvpSelectionError> {
        type Error = ParseRsvpSelectionError;

        #[derive(Debug, serde::Deserialize)]
        pub struct RsvpRequest {
            spot_id: i64,
            qty: i64,
            contribution: Option<i64>,
        }

        let requests: Vec<RsvpRequest> = serde_json::from_str(rsvps).map_err(|_| Error::Parse)?;
        let mut rsvps = vec![];

        let mut reserved_qty = 0;
        for res in requests {
            let spot_id = res.spot_id;
            let Some(spot) = spots.iter().find(|s| s.id == spot_id) else {
                return Err(Error::UnknownSpot { spot_id });
            };

            if res.qty > *stats.remaining_spots.get(&res.spot_id).unwrap_or(&0) {
                return Err(Error::SpotCapacity { spot_id });
            }

            let contribution = match spot.kind.as_str() {
                Spot::FIXED => spot.required_contribution.unwrap(),
                Spot::VARIABLE => res.contribution.unwrap(),
                Spot::FREE => 0,
                Spot::WORK => 0,
                kind => panic!("unknown kind: {kind}"),
            };
            if spot.kind == Spot::VARIABLE {
                let min = spot.min_contribution.unwrap();
                let max = spot.max_contribution.unwrap();
                if !(min..=max).contains(&contribution) {
                    return Err(Error::SpotRange { spot_id });
                }
            }
            reserved_qty += res.qty;

            for _ in 0..res.qty {
                rsvps.push(ParsedRsvp { spot_id, contribution })
            }
        }

        if reserved_qty > stats.remaining_capacity {
            return Err(Error::EventCapacity);
        }

        Ok(rsvps)
    }
}

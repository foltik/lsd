use crate::db::event::*;
use crate::db::spot::*;
use crate::prelude::*;

/// Add all `events` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/rsvp", post(rsvp::rsvp_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::selection_page).post(rsvp::selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::attendees_page).post(rsvp::attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::contribution_page))
                .route("/e/{slug}/rsvp/confirmation", get(rsvp::confirmation_page))
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
    use std::time::Instant;

    use super::*;
    use crate::db::rsvp::{CreateRsvp, Rsvp, UpdateRsvp};
    use crate::db::rsvp_session::RsvpSession;
    use crate::utils::stripe;

    /// Token passed between RSVP steps for idempotency.
    #[derive(Debug, serde::Deserialize)]
    pub struct SessionQuery {
        session: String,
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
    pub async fn selection_page(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == "paid" {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp/confirmation?session={}", query.session))
                .into_response());
        }

        let event = Event::lookup_by_id(&state.db, session.event_id)
            .await?
            .ok_or(AppError::NotFound)?;
        let stats = event.stats_for_session(&state.db, session.id).await?;

        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let mut spot_qtys = HashMap::default();
        let mut spot_contributions = HashMap::default();

        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        for rsvp in rsvps {
            *spot_qtys.entry(rsvp.spot_id).or_default() += 1;
            spot_contributions.insert(rsvp.spot_id, rsvp.contribution);
        }

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_selection.html")]
        struct SelectionHtml {
            slug: String,
            spots: Vec<Spot>,
            spot_qtys: HashMap<i64, usize>,
            spot_contributions: HashMap<i64, i64>,
            stats: EventStats,
        }
        Ok(SelectionHtml { slug, spots, spot_qtys, spot_contributions, stats }.into_response())
    }

    // Handle submission of the "Choose a contribution" form
    #[derive(Debug, serde::Deserialize)]
    pub struct SelectionForm {
        rsvps: String,
    }
    pub async fn selection_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
        Form(form): Form<SelectionForm>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == "paid" {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp/confirmation?session={}", query.session))
                .into_response());
        }

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        // Compute ticket stats. This includes other users' pending RSVPs, but excludes those from our own session.
        let stats = event.stats_for_session(&state.db, session.id).await?;

        // Sending structured data through a form submission is pain, and axum doesn't even
        // support deserializing serde types from the several methods that exist... we just use a string.
        let rsvps = parse_selection(&stats, &spots, &form.rsvps).map_err(|_| AppError::BadRequest)?;

        // Delete any pending RSVPs and create new ones.
        // Needed in case the user presses back and changes their selection.
        Rsvp::delete_for_session(&state.db, session.id).await?;

        if let Some(user) = user
            && rsvps.len() == 1
        {
            // In the case of a logged in user buying a single ticket, we can skip attendee selection.

            // unwrap(): we've just validated in `parse_selection()`.
            let rsvp = rsvps.get(0).unwrap();
            let spot = spots.iter().find(|s| s.id == rsvp.spot_id).unwrap();
            Rsvp::create(
                &state.db,
                CreateRsvp {
                    event_id: event.id,
                    spot_id: rsvp.spot_id,
                    contribution: rsvp.contribution,
                    status: "pending".into(),
                    session_id: session.id,
                    first_name: Some(user.first_name.clone()),
                    last_name: Some(user.last_name.clone()),
                    email: Some(user.email.clone()),
                    user_id: Some(user.id),
                },
            )
            .await?;

            let line_item =
                stripe::LineItem { name: spot.name.clone(), quantity: 1, price: rsvp.contribution };
            let return_url = format!("/e/{slug}/rsvp/confirmation?session={}", session.token);
            let stripe_client_secret = state
                .stripe
                .create_session(session.id, &user.email, vec![line_item], return_url)
                .await?;
            RsvpSession::set_stripe_client_secret(&state.db, session.id, &stripe_client_secret).await?;

            Ok(Redirect::to(&format!("/e/{slug}/rsvp/contribution?session={}", session.token))
                .into_response())
        } else {
            // Otherwise, proceed as normal to attendees selection

            for rsvp in rsvps {
                Rsvp::create(
                    &state.db,
                    CreateRsvp {
                        event_id: event.id,
                        spot_id: rsvp.spot_id,
                        contribution: rsvp.contribution,
                        status: "pending".into(),
                        session_id: session.id,
                        first_name: None,
                        last_name: None,
                        email: None,
                        user_id: None,
                    },
                )
                .await?;
            }

            Ok(Redirect::to(&format!("/e/{slug}/rsvp/attendees?session={}", session.token)).into_response())
        }
    }

    // Display the "Who will be attending?" page after submitting spots
    pub async fn attendees_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == "paid" {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp/confirmation?session={}", query.session))
                .into_response());
        }

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(serde::Serialize)]
        struct AttendeesRsvp {
            pub id: i64,
            pub spot_name: String,
            pub first_name: Option<String>,
            pub last_name: Option<String>,
            pub email: Option<String>,
        }
        // unwrap(): we've already validated `spot_id` must exist in `parse_contributions()`.
        let rsvps = rsvps
            .into_iter()
            .map(|r| AttendeesRsvp {
                id: r.id,
                spot_name: spots.iter().find(|s| s.id == r.spot_id).unwrap().name.clone(),
                first_name: r.first_name,
                last_name: r.last_name,
                email: r.email,
            })
            .collect::<Vec<_>>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_attendees.html")]
        struct AttendeesHtml {
            slug: String,
            user: Option<User>,
            session: RsvpSession,
            rsvps: Vec<AttendeesRsvp>,
        }
        Ok(AttendeesHtml { slug, user, session, rsvps }.into_response())
    }

    // Handle submission of the "Who will be attending?" form
    #[derive(Debug, serde::Deserialize)]
    pub struct AttendeesForm {
        attendees: String,
    }
    pub async fn attendees_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
        Form(form): Form<AttendeesForm>,
    ) -> AppResult<Response> {
        let Some(mut session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == "paid" {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp/confirmation?session={}", query.session))
                .into_response());
        }

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        let attendees = parse_attendees(&state.db, &rsvps, &form.attendees)
            .await
            .map_err(|_| AppError::BadRequest)?;

        for a in attendees {
            // Record the info of the "is_me" attendee in the session
            if a.is_me && user.is_none() {
                session
                    .set_contact(&state.db, a.first_name.clone(), a.last_name.clone(), a.email.clone())
                    .await?;
            }

            // unwrap(): parse_attendees verified attendees and rsvps correspond 1:1.
            let rsvp = rsvps.iter().find(|r| r.id == a.rsvp_id).unwrap();
            Rsvp::update(
                &state.db,
                rsvp.id,
                UpdateRsvp {
                    status: "pending".into(),
                    first_name: Some(a.first_name),
                    last_name: Some(a.last_name),
                    email: Some(a.email),
                    user_id: a.user_id,
                    checkin_at: None,
                },
            )
            .await?;
        }

        let line_items = session.line_items(&spots, &rsvps)?;
        let return_url = format!("/e/{slug}/rsvp/confirmation?session={}", session.token);
        // unwrap(): parse_attendees verified at least one attendee has `is_me: true`,
        // and we've written their email to `session.email` above.
        let stripe_client_secret = state
            .stripe
            .create_session(session.id, &session.email.unwrap(), line_items, return_url)
            .await?;

        RsvpSession::set_stripe_client_secret(&state.db, session.id, &stripe_client_secret).await?;

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/contribution?session={}", session.token)).into_response())
    }

    // Display the "Make your contribution" page after submitting attendees
    pub async fn contribution_page(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == "paid" {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp/confirmation?session={}", query.session))
                .into_response());
        }
        let Some(stripe_client_secret) = session.stripe_client_secret else {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/attendees?session={}", query.session)).into_response()
            );
        };

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(serde::Serialize)]
        struct ContributionRsvp {
            pub spot_name: String,
            pub first_name: String,
            pub last_name: String,
            pub email: String,
            pub contribution: i64,
        }
        // unwrap(): we've already validated these fields must exist in `attendees_form()`.
        let rsvps = rsvps
            .into_iter()
            .map(|r| ContributionRsvp {
                spot_name: spots.iter().find(|s| s.id == r.spot_id).unwrap().name.clone(),
                first_name: r.first_name.unwrap().clone(),
                last_name: r.last_name.unwrap().clone(),
                email: r.email.unwrap(),
                contribution: r.contribution,
            })
            .collect::<Vec<_>>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_contribution.html")]
        struct ContributionHtml {
            rsvps: Vec<ContributionRsvp>,
            stripe_publishable_key: String,
            stripe_client_secret: String,
        }
        Ok(ContributionHtml {
            rsvps,
            stripe_client_secret,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        }
        .into_response())
    }

    pub async fn confirmation_page(
        State(state): State<SharedAppState>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<impl IntoResponse> {
        let session = RsvpSession::lookup_by_token(&state.db, &query.session)
            .await?
            .ok_or(AppError::BadRequest)?;

        Stripe::wait_for_payment(&state.db, &state.webhooks, session.id).await?;

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(serde::Serialize)]
        struct ConfirmationRsvp {
            pub spot_name: String,
            pub first_name: String,
            pub last_name: String,
            pub email: String,
            pub contribution: i64,
        }
        // unwrap(): we've already validated these fields must exist in `attendees_form()`.
        let rsvps = rsvps
            .into_iter()
            .map(|r| ConfirmationRsvp {
                spot_name: spots.iter().find(|s| s.id == r.spot_id).unwrap().name.clone(),
                first_name: r.first_name.unwrap().clone(),
                last_name: r.last_name.unwrap().clone(),
                email: r.email.unwrap(),
                contribution: r.contribution,
            })
            .collect::<Vec<_>>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_confirmation.html")]
        struct ConfirmationHtml {
            rsvps: Vec<ConfirmationRsvp>,
        }
        Ok(ConfirmationHtml { rsvps }.into_response())
    }

    pub struct ParsedSelection {
        spot_id: i64,
        contribution: i64,
    }
    #[derive(thiserror::Error, Debug)]
    pub enum ParseSelectionError {
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
    fn parse_selection(
        stats: &EventStats,
        spots: &[Spot],
        selection: &str,
    ) -> Result<Vec<ParsedSelection>, ParseSelectionError> {
        type Error = ParseSelectionError;

        #[derive(Debug, serde::Deserialize)]
        pub struct RsvpForm {
            spot_id: i64,
            qty: i64,
            contribution: Option<i64>,
        }

        let rsvps: Vec<RsvpForm> = serde_json::from_str(selection).map_err(|_| Error::Parse)?;
        let mut parsed = vec![];

        let mut reserved_qty = 0;
        for rsvp in rsvps {
            let spot_id = rsvp.spot_id;
            let Some(spot) = spots.iter().find(|s| s.id == spot_id) else {
                return Err(Error::UnknownSpot { spot_id });
            };

            if rsvp.qty > *stats.remaining_spots.get(&rsvp.spot_id).unwrap_or(&0) {
                return Err(Error::SpotCapacity { spot_id });
            }

            let contribution = match spot.kind.as_str() {
                Spot::FIXED => spot.required_contribution.unwrap(),
                Spot::VARIABLE => rsvp.contribution.unwrap(),
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
            reserved_qty += rsvp.qty;

            for _ in 0..rsvp.qty {
                parsed.push(ParsedSelection { spot_id, contribution })
            }
        }

        if reserved_qty > stats.remaining_capacity {
            return Err(Error::EventCapacity);
        }

        Ok(parsed)
    }

    struct ParsedAttendee {
        rsvp_id: i64,

        first_name: String,
        last_name: String,
        email: String,
        user_id: Option<i64>,

        is_me: bool,
    }
    #[derive(thiserror::Error, Debug)]
    pub enum ParseAttendeesError {
        #[error("failed to parse request JSON")]
        Parse,

        #[error("unknown rsvp_id={rsvp_id}")]
        UnknownRsvp { rsvp_id: i64 },
        #[error("missing attendee for rsvp_id={rsvp_id}")]
        MissingAttendee { rsvp_id: i64 },

        #[error("{0}")]
        AppError(#[from] AppError),
    }
    async fn parse_attendees(
        db: &Db,
        rsvps: &[Rsvp],
        attendees: &str,
    ) -> Result<Vec<ParsedAttendee>, ParseAttendeesError> {
        type Error = ParseAttendeesError;

        #[derive(Debug, serde::Deserialize)]
        pub struct AttendeeForm {
            rsvp_id: i64,

            first_name: String,
            last_name: String,
            email: String,

            is_me: bool,
        }

        let attendees: Vec<AttendeeForm> = serde_json::from_str(attendees).map_err(|_| Error::Parse)?;
        let mut parsed = vec![];

        for rsvp in rsvps {
            // Ensure all RSVPs have an attendee specified
            if !attendees.iter().any(|a| a.rsvp_id == rsvp.id) {
                return Err(Error::MissingAttendee { rsvp_id: rsvp.id });
            }
        }

        for AttendeeForm { rsvp_id, first_name, last_name, email, is_me } in attendees {
            // Ensure all attendees correspond to a valid RSVP
            if !rsvps.iter().any(|r| r.id == rsvp_id) {
                return Err(Error::UnknownRsvp { rsvp_id });
            }

            let user_id = User::lookup_by_email(db, &email).await?.map(|u| u.id);
            parsed.push(ParsedAttendee { rsvp_id, first_name, last_name, email, user_id, is_me })
        }

        Ok(parsed)
    }
}

use image::DynamicImage;

use crate::db::event::*;
use crate::db::event_flyer::*;
use crate::db::spot::*;
use crate::prelude::*;

/// Add all `events` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/flyer", get(read::flyer))
                .route("/e/{slug}/rsvp", post(rsvp::rsvp_form))
                .route("/e/{slug}/guestlist", get(rsvp::guestlist_page).post(rsvp::guestlist_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::selection_page).post(rsvp::selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::attendees_page).post(rsvp::attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::contribution_page).post(rsvp::contribution_form))
                .route("/e/{slug}/rsvp/manage", get(rsvp::manage_page))
                .route("/e/{slug}/rsvp/status", get(rsvp::status))
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
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        #[derive(Template, WebTemplate)]
        #[template(path = "events/view.html")]
        struct Html {
            pub user: Option<User>,
            event: Event,
            has_flyer: bool,
        }
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let has_flyer = EventFlyer::exists_for_event(&state.db, event.id).await?;
        Ok(Html { user, event, has_flyer })
    }

    // List all events.
    #[derive(Template, WebTemplate)]
    #[template(path = "events/list.html")]
    struct ListHtml {
        user: Option<User>,
        events: Vec<Event>,
    }

    pub async fn list_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
    ) -> AppResult<impl IntoResponse> {
        Ok(ListHtml { user, events: Event::list(&state.db).await? })
    }

    /// Serve an event flyer.
    pub async fn flyer(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(params): Query<std::collections::HashMap<String, String>>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;

        let size = match params.get("size").map(|s| s.as_str()) {
            Some("sm") => EventFlyerSize::Small,
            Some("md") => EventFlyerSize::Medium,
            Some("lg") => EventFlyerSize::Large,
            Some(_) => return Err(AppError::BadRequest),
            None => EventFlyerSize::Full,
        };

        let bytes = EventFlyer::lookup_for_event(&state.db, event.id, size)
            .await?
            .ok_or(AppError::NotFound)?;

        Ok(([(header::CONTENT_TYPE, EventFlyer::CONTENT_TYPE)], bytes))
    }
}

// Create and edit events.
mod edit {
    use super::*;
    use crate::db::list::{List, ListWithCount};

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    struct EditHtml {
        user: Option<User>,
        event: Event,
        spots: Vec<Spot>,
        has_flyer: bool,
        lists: Vec<ListWithCount>,
    }

    /// Display the form to create a new event.
    pub async fn new_page(user: User, State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml {
            user: Some(user),
            event: Event {
                id: 0,
                title: "".into(),
                slug: "".into(),
                description: "".into(),

                start: Utc::now().naive_utc(),
                end: None,

                capacity: 0,
                unlisted: false,
                guest_list_id: None,

                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            spots: vec![],
            has_flyer: false,
            lists,
        })
    }

    /// Display the form to edit an event.
    pub async fn edit_page(
        user: User,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let has_flyer = EventFlyer::exists_for_event(&state.db, event.id).await?;
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml { user: Some(user), event, spots, has_flyer, lists })
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
        mut multipart: axum::extract::Multipart,
    ) -> AppResult<impl IntoResponse> {
        let mut form: Option<EditForm> = None;
        let mut flyer: Option<DynamicImage> = None;

        while let Some(field) = multipart.next_field().await? {
            match field.name().unwrap_or("") {
                "data" => {
                    let text = field.text().await?;
                    form = Some(serde_json::from_str(&text).map_err(|_| AppError::BadRequest)?);
                }
                "flyer" => {
                    let data = field.bytes().await?;
                    let img = crate::utils::image::decode(&data).await?;
                    flyer = Some(img);
                }
                _ => {}
            }
        }

        let form = form.ok_or(AppError::BadRequest)?;

        match form.id {
            Some(id) => {
                Event::update(&state.db, id, &form.event, &flyer).await?;

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
                let event_id = Event::create(&state.db, &form.event, &flyer).await?;

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
    use crate::db::list::List;
    use crate::db::rsvp::{CreateRsvp, Rsvp, UpdateRsvp};
    use crate::db::rsvp_session::RsvpSession;
    use crate::utils::stripe;

    /// Token passed between RSVP steps for idempotency.
    #[derive(Debug, serde::Deserialize)]
    pub struct SessionQuery {
        session: String,
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "error.html")]
    struct ErrorHtml {
        user: Option<User>,
        message: String,
    }

    /// Create an RSVP session after a user clicks the RSVP button for an event.
    pub async fn rsvp_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<Response> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;

        Ok(match event.guest_list_id {
            // If no guest list, create the session
            None => {
                let token = RsvpSession::create(&state.db, event.id, &user).await?;
                Redirect::to(&format!("/e/{slug}/rsvp/selection?session={token}")).into_response()
            }
            // When there's a guest list...
            Some(list_id) => match user {
                Some(user) => match List::has_user(&state.db, list_id, &user).await? {
                    // If logged in and on the list, direct them straight to ticket selection
                    true => {
                        let token = RsvpSession::create(&state.db, event.id, &Some(user)).await?;
                        Redirect::to(&format!("/e/{slug}/rsvp/selection?session={token}")).into_response()
                    }
                    // If logged in and not on the list, show an error page
                    false => ErrorHtml { user: Some(user), message: "Sorry, you're not on the list!".into() }
                        .into_response(),
                },
                // If not logged in, direct them to the guestlist gate
                None => Redirect::to(&format!("/e/{slug}/guestlist")).into_response(),
            },
        })
    }

    // Display the "Are you on the list?" page
    pub async fn guestlist_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> AppResult<Response> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let _guest_list_id = event.guest_list_id.ok_or(AppError::BadRequest)?;
        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_guestlist.html")]
        struct GuestlistHtml {
            user: Option<User>,
            slug: String,
        }
        Ok(GuestlistHtml { user, slug }.into_response())
    }

    // Handle submission of the "Are you on the  list?" form
    #[derive(Debug, serde::Deserialize)]
    pub struct GuestlistForm {
        email: String,
    }
    pub async fn guestlist_form(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Form(form): Form<GuestlistForm>,
    ) -> AppResult<Response> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
        let guest_list_id = event.guest_list_id.ok_or(AppError::BadRequest)?;

        Ok(match List::has_email(&state.db, guest_list_id, &form.email).await? {
            // If on the list, direct them to ticket selection
            true => {
                let token = RsvpSession::create(&state.db, event.id, &user).await?;
                Redirect::to(&format!("/e/{slug}/rsvp/selection?session={token}")).into_response()
            }
            // If not on the list, show an error page
            false => ErrorHtml { user, message: "Sorry, you're not on the list!".into() }.into_response(),
        })
    }

    // Display the "Choose a contribution" page
    pub async fn selection_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == RsvpSession::PAID {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)).into_response()
            );
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
            user: Option<User>,
            slug: String,
            session: RsvpSession,
            spots: Vec<Spot>,
            spot_qtys: HashMap<i64, usize>,
            spot_contributions: HashMap<i64, i64>,
            stats: EventStats,
        }
        Ok(
            SelectionHtml { user, slug, session, spots, spot_qtys, spot_contributions, stats }
                .into_response(),
        )
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
        if session.status == RsvpSession::PAID {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)).into_response()
            );
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
            let email = &user.email;

            // Handle RsvpSession conflicts. If another session exists for this email, delete it.
            // We expect this to happen when a user starts a session on one device and switches to another.
            let session_conflict =
                RsvpSession::lookup_conflicts(&state.db, session.event_id, session.id, email).await?;
            // If another pending session exists matching the `is_me` email, delete it.
            // We expect this to happen when a user starts a session on one device and switches to another.
            // Note: a PAID session will be caught when we check for Rsvp conflicts.
            if let Some(other_session) = session_conflict
                && other_session.status == RsvpSession::PENDING
            {
                other_session.delete(&state.db).await?;
            }
            if let Some(status) =
                Rsvp::lookup_conflicts(&state.db, session.event_id, session.id, email).await?
            {
                let message = match status.as_str() {
                    RsvpSession::PAID => format!("Someone has already RSVPed for {email}."),
                    RsvpSession::PENDING => format!("Someone is currently RSVPing for {email}."),
                    _ => unreachable!(),
                };
                return Ok(ErrorHtml { user: Some(user), message }.into_response());
            }

            // unwrap(): we've just validated in `parse_selection()`.
            let rsvp = rsvps.first().unwrap();
            let spot = spots.iter().find(|s| s.id == rsvp.spot_id).unwrap();
            Rsvp::create(
                &state.db,
                CreateRsvp {
                    event_id: event.id,
                    spot_id: rsvp.spot_id,
                    contribution: rsvp.contribution,
                    status: RsvpSession::PENDING.into(),
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
            let return_url = format!("/e/{slug}/rsvp/manage?session={}", session.token);
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
                        status: RsvpSession::PENDING.into(),
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
        if session.status == RsvpSession::PAID {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)).into_response()
            );
        }

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(serde::Serialize)]
        struct Attendee {
            pub rsvp_id: i64,
            pub spot_name: String,
            pub first_name: Option<String>,
            pub last_name: Option<String>,
            pub email: Option<String>,
            pub error: Option<String>,
        }
        let attendees = rsvps
            .into_iter()
            .map(|r| Attendee {
                rsvp_id: r.id,
                // unwrap(): we've already validated `spot_id` must exist in `parse_contributions()`.
                spot_name: spots.iter().find(|s| s.id == r.spot_id).unwrap().name.clone(),
                first_name: r.first_name,
                last_name: r.last_name,
                email: r.email,
                error: None,
            })
            .collect::<Vec<_>>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_attendees.html")]
        struct AttendeesHtml {
            slug: String,
            user: Option<User>,
            session: RsvpSession,
            attendees: Vec<Attendee>,
        }
        Ok(AttendeesHtml { slug, user, session, attendees }.into_response())
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
        if session.status == RsvpSession::PAID {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)).into_response()
            );
        }

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        // Parse form data
        let (is_me_attendee, attendees) = parse_attendees(&state.db, &rsvps, &form.attendees)
            .await
            .map_err(|_| AppError::BadRequest)?;

        // Handle RsvpSession conflicts.
        let session_conflict =
            RsvpSession::lookup_conflicts(&state.db, session.event_id, session.id, &is_me_attendee.email)
                .await?;
        // If another pending session exists matching the `is_me` email, delete it.
        // We expect this to happen when a user starts a session on one device and switches to another.
        // Note: a PAID session will be caught when we check for Rsvp conflicts.
        if let Some(other_session) = session_conflict
            && other_session.status == RsvpSession::PENDING
        {
            other_session.delete(&state.db).await?;
        }
        // Handle Rsvp conflicts.
        for ParsedAttendee { email, .. } in &attendees {
            let Some(status) = Rsvp::lookup_conflicts(&state.db, session.event_id, session.id, email).await?
            else {
                continue;
            };

            let message = match status.as_str() {
                RsvpSession::PAID => format!("Someone has already RSVPed for {email}."),
                RsvpSession::PENDING => format!("Someone is currently RSVPing for {email}."),
                _ => unreachable!(),
            };
            return Ok(ErrorHtml { user, message }.into_response());
        }

        // Store is_me contact info in the RsvpSession entry
        let ParsedAttendee { first_name, last_name, email, .. } = is_me_attendee.clone();
        session.set_contact(&state.db, first_name, last_name, email).await?;

        // Store all contact info (including is_me) in the individual Rsvp entries
        for attendee in attendees {
            let ParsedAttendee { rsvp, first_name, last_name, email, user_id, .. } = attendee;
            Rsvp::update(
                &state.db,
                rsvp.id,
                UpdateRsvp {
                    status: RsvpSession::PENDING.into(),
                    first_name: Some(first_name),
                    last_name: Some(last_name),
                    email: Some(email),
                    user_id,
                    checkin_at: None,
                },
            )
            .await?;
        }

        let line_items = session.line_items(&spots, &rsvps)?;
        let return_url = format!("/e/{slug}/rsvp/manage?session={}", session.token);
        let stripe_client_secret = state
            .stripe
            .create_session(session.id, &session.email.unwrap(), line_items, return_url)
            .await?;

        RsvpSession::set_stripe_client_secret(&state.db, session.id, &stripe_client_secret).await?;

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/contribution?session={}", session.token)).into_response())
    }

    // Display the "Make your contribution" page after submitting attendees
    pub async fn contribution_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Response> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };
        if session.status == RsvpSession::PAID {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)).into_response()
            );
        }

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

        let total = rsvps.iter().map(|r| r.contribution).sum();
        if total > 0 && session.stripe_client_secret.is_none() {
            return Ok(
                Redirect::to(&format!("/e/{slug}/rsvp/attendees?session={}", query.session)).into_response()
            );
        }

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_contribution.html")]
        struct ContributionHtml {
            user: Option<User>,
            slug: String,
            session: RsvpSession,
            rsvps: Vec<ContributionRsvp>,
            total: i64,
            stripe_publishable_key: String,
        }
        Ok(ContributionHtml {
            user,
            slug,
            session,
            rsvps,
            total,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        }
        .into_response())
    }

    // Handle submission of $0 RSVPs.
    pub async fn contribution_form(
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<Redirect> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")));
        };

        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        let total: i64 = rsvps.iter().map(|r| r.contribution).sum();
        if total > 0 {
            return Err(AppError::BadRequest);
        }

        session.set_paid(&state.db, None).await?;

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/manage?session={}", query.session)))
    }

    pub async fn manage_page(
        user: Option<User>,
        State(state): State<SharedAppState>,
        Path(slug): Path<String>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<impl IntoResponse> {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.session).await? else {
            return Ok(Redirect::to(&format!("/e/{slug}")).into_response());
        };

        match session.status.as_str() {
            RsvpSession::PENDING => Stripe::wait_for_payment(&state.db, &state.webhooks, session.id).await?,
            RsvpSession::PAID => {}
            _ => unreachable!(),
        };

        let spots = Spot::list_for_event(&state.db, session.event_id).await?;
        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        #[derive(serde::Serialize)]
        struct ManageRsvp {
            spot_name: String,
            first_name: String,
            last_name: String,
            email: String,
            contribution: i64,
        }
        // unwrap(): we've already validated these fields must exist in `attendees_form()`.
        let rsvps = rsvps
            .into_iter()
            .map(|r| ManageRsvp {
                spot_name: spots.iter().find(|s| s.id == r.spot_id).unwrap().name.clone(),
                first_name: r.first_name.unwrap().clone(),
                last_name: r.last_name.unwrap().clone(),
                email: r.email.unwrap(),
                contribution: r.contribution,
            })
            .collect::<Vec<_>>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_manage.html")]
        struct ManageHtml {
            user: Option<User>,
            session: RsvpSession,
            rsvps: Vec<ManageRsvp>,
        }
        Ok(ManageHtml { user, session, rsvps }.into_response())
    }

    /// Query existence of an RSVP session. Used by frontend to invalidate localStorage.
    pub async fn status(
        State(state): State<SharedAppState>,
        Query(query): Query<SessionQuery>,
    ) -> AppResult<StatusCode> {
        Ok(match RsvpSession::lookup_by_token(&state.db, &query.session).await? {
            Some(_) => StatusCode::OK,
            None => StatusCode::NOT_FOUND,
        })
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

    #[derive(Clone)]
    struct ParsedAttendee<'a> {
        rsvp: &'a Rsvp,

        first_name: String,
        last_name: String,
        email: String,
        user_id: Option<i64>,
    }
    #[derive(thiserror::Error, Debug)]
    pub enum ParseAttendeesError {
        #[error("failed to parse request JSON")]
        Parse,

        #[error("unknown rsvp_id={rsvp_id}")]
        UnknownRsvp { rsvp_id: i64 },
        #[error("missing attendee for rsvp_id={rsvp_id}")]
        MissingAttendee { rsvp_id: i64 },
        #[error("missing attendee with is_me=true")]
        MissingIsMe,
        #[error("multiple attendees with is_me=true")]
        MultipleIsMe,

        #[error("{0}")]
        AppError(#[from] AppError),
    }
    async fn parse_attendees<'a>(
        db: &Db,
        rsvps: &'a [Rsvp],
        attendees: &str,
    ) -> Result<(ParsedAttendee<'a>, Vec<ParsedAttendee<'a>>), ParseAttendeesError> {
        type Error = ParseAttendeesError;

        #[derive(Debug, serde::Deserialize)]
        pub struct AttendeeForm {
            rsvp_id: i64,

            first_name: String,
            last_name: String,
            email: String,

            is_me: bool,
        }

        let form: Vec<AttendeeForm> = serde_json::from_str(attendees).map_err(|_| Error::Parse)?;

        let mut attendees = vec![];
        let mut is_me_attendee = None;

        for rsvp in rsvps {
            // Ensure all RSVPs have an attendee specified
            if !form.iter().any(|a| a.rsvp_id == rsvp.id) {
                return Err(Error::MissingAttendee { rsvp_id: rsvp.id });
            }
        }

        for AttendeeForm { rsvp_id, first_name, last_name, email, is_me } in form {
            // Ensure all attendees correspond to a valid RSVP
            let Some(rsvp) = rsvps.iter().find(|r| r.id == rsvp_id) else {
                return Err(Error::UnknownRsvp { rsvp_id });
            };

            let user_id = User::lookup_by_email(db, &email).await?.map(|u| u.id);
            let attendee = ParsedAttendee { rsvp, first_name, last_name, email, user_id };

            if is_me {
                match is_me_attendee {
                    Some(_) => return Err(Error::MultipleIsMe),
                    None => is_me_attendee = Some(attendee.clone()),
                }
            }
            attendees.push(attendee);
        }

        let Some(is_me_attendee) = is_me_attendee else {
            return Err(Error::MissingIsMe);
        };

        Ok((is_me_attendee, attendees))
    }
}

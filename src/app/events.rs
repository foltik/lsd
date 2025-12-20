use image::DynamicImage;

use crate::db::event::*;
use crate::db::event_flyer::*;
use crate::db::rsvp_session::*;
use crate::db::spot::*;
use crate::prelude::*;

/// Add all `events` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/flyer", get(read::flyer))
                .route("/e/{slug}/rsvp", get(rsvp::rsvp_form))
                .route("/e/{slug}/rsvp/guestlist", get(rsvp::guestlist_page).post(rsvp::guestlist_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::selection_page).post(rsvp::selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::attendees_page).post(rsvp::attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::contribution_page).post(rsvp::contribution_form))
                .route("/e/{slug}/rsvp/manage", get(rsvp::manage_page).post(rsvp::temp_delete))
                .route("/e/{slug}/rsvp/edit", get(rsvp::edit_guests_page).post(rsvp::edit_guests_form))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/events", get(read::list_page))
                .route("/events/sessions", get(read::sessions_page))
                .route("/events/sessions/{id}", delete(read::delete_session))
                .route("/events/new", get(edit::new_page))
                .route("/events/{slug}/edit", get(edit::edit_page).post(edit::edit_form))
                .route("/events/{slug}/delete", post(edit::delete_form))
                .route("/events/{slug}/attendees", get(edit::attendees_page))
                .route("/events/{slug}/attendees/{rsvp_id}/checkin", post(edit::set_checkin).delete(edit::clear_checkin))
                .route("/events/{id}/invite/edit", get(edit::edit_invite_page).post(edit::edit_invite_form))
                .route("/events/{id}/invite/preview", get(edit::preview_invite_page))
                .route("/events/{id}/invite/send", get(edit::send_invite_page).post(edit::send_invite_form))
                .route("/events/{id}/confirmation/edit", get(edit::edit_confirmation_page).post(edit::edit_confirmation_form))
                .route("/events/{id}/confirmation/preview", get(edit::preview_confirmation_page))
                .route("/events/{id}/dayof/edit", get(edit::edit_dayof_page).post(edit::edit_dayof_form))
                .route("/events/{id}/dayof/preview", get(edit::preview_dayof_page))
                .route("/events/{id}/dayof/send", get(edit::send_dayof_page).post(edit::send_dayof_form))
                .route("/events/{id}/description/edit", get(edit::edit_description_page).post(edit::edit_description_form))
        })
}

// View and list events.
mod read {
    use super::*;
    use crate::db::rsvp_session;

    /// View an event.
    pub async fn view_page(
        session: Option<RsvpSession>, user: Option<User>, State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        #[derive(Template, WebTemplate)]
        #[template(path = "events/view.html")]
        struct Html {
            session: Option<RsvpSession>,
            pub user: Option<User>,
            event: Event,
            flyer: Option<EventFlyer>,
        }
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;
        Ok(Html { session, user, event, flyer }.into_response())
    }

    // List all events.
    #[derive(Template, WebTemplate)]
    #[template(path = "events/list.html")]
    struct ListHtml {
        user: Option<User>,
        events: Vec<Event>,
    }

    pub async fn list_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
        Ok(ListHtml { user, events: Event::list(&state.db).await? }.into_response())
    }

    /// Serve an event flyer.
    pub async fn flyer(
        State(state): State<SharedAppState>, Path(slug): Path<String>,
        Query(params): Query<std::collections::HashMap<String, String>>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        let size = match params.get("size").map(|s| s.as_str()) {
            Some("sm") => EventFlyerSize::Small,
            Some("md") => EventFlyerSize::Medium,
            Some("lg") => EventFlyerSize::Large,
            Some(_) => bail_invalid!(),
            None => EventFlyerSize::Full,
        };

        let bytes = EventFlyer::serve(&state.db, event.id, size).await?.ok_or_else(not_found)?;

        Ok((
            [
                (header::CONTENT_TYPE, EventFlyer::CONTENT_TYPE),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                (HeaderName::from_static("priority"), "u=1"), // urgency below main.css (u=0) and above default (u=3)
            ],
            bytes,
        )
            .into_response())
    }

    /// Debug view for RSVP sessions.
    #[derive(Template, WebTemplate)]
    #[template(path = "events/sessions.html")]
    struct SessionsHtml {
        user: Option<User>,
        sessions: Vec<rsvp_session::DebugSession>,
    }
    pub async fn sessions_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
        let sessions = RsvpSession::list_debug(&state.db).await?;
        Ok(SessionsHtml { user: Some(user), sessions }.into_response())
    }

    pub async fn delete_session(State(state): State<SharedAppState>, Path(id): Path<i64>) -> JsonResult<()> {
        let session = RsvpSession::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        if session.status == RsvpSession::PAYMENT_PENDING || session.status == RsvpSession::PAYMENT_CONFIRMED
        {
            bail_invalid!();
        }
        session.delete(&state.db).await?;
        Ok(Json(()))
    }
}

// Create and edit events.
mod edit {
    use axum::body::Body;

    use super::*;
    use crate::db::list::{List, ListWithCount};
    use crate::db::rsvp::{AdminAttendeesRsvp, Rsvp};
    use crate::utils::editor::{Editor, EditorContent};

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    struct EditHtml {
        user: Option<User>,
        event: Event,
        spots: Vec<Spot>,
        rsvp_counts: std::collections::HashMap<i64, i64>,
        has_flyer: bool,
        lists: Vec<ListWithCount>,
    }

    /// Display the form to create a new event.
    pub async fn new_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml {
            user: Some(user),
            event: Event {
                id: 0,
                title: "".into(),
                slug: "".into(),
                start: Utc::now().naive_utc(),
                end: None,
                capacity: 0,
                unlisted: false,
                closed: false,
                guest_list_id: None,
                spots_per_person: None,

                description_html: None,
                description_updated_at: None,

                invite_subject: None,
                invite_html: None,
                invite_updated_at: None,
                invite_sent_at: None,

                confirmation_subject: None,
                confirmation_html: None,
                confirmation_updated_at: None,

                dayof_subject: None,
                dayof_html: None,
                dayof_updated_at: None,
                dayof_sent_at: None,

                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            spots: vec![],
            rsvp_counts: Default::default(),
            has_flyer: false,
            lists,
        }
        .into_response())
    }

    /// Display the form to edit an event.
    pub async fn edit_page(
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let rsvp_counts = Spot::rsvp_counts_for_event(&state.db, event.id).await?;
        let has_flyer = EventFlyer::exists_for_event(&state.db, event.id).await?;
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml { user: Some(user), event, spots, rsvp_counts, has_flyer, lists }.into_response())
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
        State(state): State<SharedAppState>, mut multipart: axum::extract::Multipart,
    ) -> HtmlResult {
        let mut form: Option<EditForm> = None;
        let mut flyer: Option<DynamicImage> = None;

        while let Some(field) = multipart.next_field().await? {
            match field.name().unwrap_or("") {
                "data" => {
                    let text = field.text().await?;
                    form = Some(serde_json::from_str(&text).map_err(|_| invalid())?);
                }
                "flyer" => {
                    let data = field.bytes().await?;
                    let img = crate::utils::image::decode(&data).await?;
                    flyer = Some(img);
                }
                _ => {}
            }
        }

        let form = form.ok_or_else(invalid)?;

        match form.id {
            Some(id) => {
                Event::update(&state.db, id, &form.event, &flyer).await?;

                let rsvp_counts = Spot::rsvp_counts_for_event(&state.db, id).await?;
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

                // Only delete spots with no RSVPs
                to_delete.retain(|&spot_id| rsvp_counts.get(&spot_id).copied().unwrap_or(0) == 0);

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

        Ok(Redirect::to("/events").into_response())
    }

    // Edit invite page.
    pub async fn edit_invite_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/edit_invite.html")]
        struct EditInviteHtml {
            user: Option<User>,
            event: Event,
            editor: Editor,
        }
        Ok(EditInviteHtml {
            user: Some(user),
            event: event.clone(),
            editor: Editor {
                url: "/events/{id}/invite/edit",
                snapshot_prefix: "event/invite",
                entity_id: Some(event.id),
                content: match (event.invite_html, event.invite_updated_at) {
                    (Some(html), Some(updated_at)) => Some(EditorContent { html, updated_at }),
                    _ => None,
                },
            },
        }
        .into_response())
    }

    // Edit invite form.
    #[derive(serde::Deserialize)]
    pub struct EditInviteForm {
        id: i64,
        subject: String,
        content: String,
    }
    #[derive(serde::Serialize)]
    pub struct EditInviteResponse {
        id: Option<i64>,
        updated_at: Option<i64>,
        error: Option<String>,
    }
    pub async fn edit_invite_form(
        State(state): State<SharedAppState>, Form(form): Form<EditInviteForm>,
    ) -> JsonResult<EditInviteResponse> {
        let Some(event) = Event::lookup_by_id(&state.db, form.id).await? else {
            bail_not_found!();
        };

        let updated_at = Event::update_invite(&state.db, event.id, form.subject, form.content).await?;

        Ok(Json(EditInviteResponse {
            id: Some(event.id),
            updated_at: Some(updated_at.and_utc().timestamp_millis()),
            error: None,
        }))
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "emails/event_invite.html")]
    struct InviteEmailHtml {
        email_id: i64,
        email: String,
        event: Event,
    }
    // Preview invite page.
    pub async fn preview_invite_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        Ok(InviteEmailHtml { email_id: 0, email: user.email, event }.into_response())
    }

    /// Display the form to send a post.
    pub async fn send_invite_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };
        let Some(guest_list_id) = event.guest_list_id else {
            bail_invalid!()
        };

        #[derive(sqlx::FromRow)]
        struct ListCounts {
            name: String,
            count: i64,
            sent: i64,
        }
        let list = sqlx::query_as!(
            ListCounts,
            r#"
            SELECT
                l.name AS name,
                COUNT(lm.user_id) AS count,
                SUM(
                    CASE WHEN EXISTS (
                        SELECT 1
                        FROM emails e
                        WHERE kind = ?
                          AND e.user_id = u.id
                          AND e.event_id = ?
                          AND e.sent_at IS NOT NULL
                    )
                    THEN 1 ELSE 0 END
                ) AS sent
            FROM lists l
            LEFT JOIN list_members lm ON lm.list_id = l.id
            LEFT JOIN users u ON u.id = lm.user_id
            WHERE l.id = ?
            GROUP BY l.id;
            "#,
            Email::EVENT_INVITE,
            event.id,
            guest_list_id,
        )
        .fetch_one(&state.db)
        .await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/send_invites.html")]
        struct SendHtml {
            user: Option<User>,
            list: ListCounts,
            event: Event,
            ratelimit: usize,
        }
        let ratelimit = state.config.email.ratelimit;
        Ok(SendHtml { user: Some(user), event, list, ratelimit }.into_response())
    }

    pub async fn send_invite_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!();
        };
        let Some(guest_list_id) = event.guest_list_id else {
            bail_invalid!()
        };

        let emails = Email::create_send_invites(&state.db, event.id, guest_list_id).await?;

        let mut email_template = InviteEmailHtml { email_id: 0, email: "".into(), event: event.clone() };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.email_id = id;
            email_template.email = address.clone();

            let from = &state.config.email.from;
            let reply_to = config().email.contact_to.as_ref().unwrap_or(from);
            let message = state
                .mailer
                .builder()
                .to(address.parse().unwrap())
                .reply_to(reply_to.clone())
                .subject(event.invite_subject.as_deref().expect("missing invite_subject"))
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(email_template.render()?)
                .unwrap();

            messages.push(message);
            email_ids.push(id);
        }

        event.mark_sent_invites(&state.db).await?;

        let email_ids = futures::stream::iter(email_ids);
        let results = state.mailer.send_batch(Arc::clone(&state), messages).await;

        let body = Body::from_stream(async_stream::stream! {
            let mut stream = Box::pin(results.zip(email_ids));
            while let Some((progress, email_id)) = stream.next().await {
                let json = match progress {
                    Ok(p) => {
                        Email::mark_sent(&state.db, email_id).await?;
                        json!({"sent": p.sent, "remaining": p.remaining})
                    }
                    Err(e) => {
                        let e = e.message();
                        Email::mark_error(&state.db, email_id, e).await?;
                        json!({"error": e})
                    }
                }.to_string();
                yield Ok::<_, AnyError>(format!("{json}\n"));
            }
        });

        Ok(body.into_response())
    }

    // Edit confirmation page.
    pub async fn edit_confirmation_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/edit_confirmation.html")]
        struct EditConfirmationHtml {
            user: Option<User>,
            event: Event,
            editor: Editor,
        }
        Ok(EditConfirmationHtml {
            user: Some(user),
            event: event.clone(),
            editor: Editor {
                url: "/events/{id}/confirmation/edit",
                snapshot_prefix: "event/confirmation",
                entity_id: Some(event.id),
                content: match (event.confirmation_html, event.confirmation_updated_at) {
                    (Some(html), Some(updated_at)) => Some(EditorContent { html, updated_at }),
                    _ => None,
                },
            },
        }
        .into_response())
    }

    // Edit confirmation form.
    #[derive(serde::Deserialize)]
    pub struct EditConfirmationForm {
        id: i64,
        subject: String,
        content: String,
    }
    #[derive(serde::Serialize)]
    pub struct EditConfirmationResponse {
        id: Option<i64>,
        updated_at: Option<i64>,
        error: Option<String>,
    }
    pub async fn edit_confirmation_form(
        State(state): State<SharedAppState>, Form(form): Form<EditConfirmationForm>,
    ) -> JsonResult<EditConfirmationResponse> {
        let Some(event) = Event::lookup_by_id(&state.db, form.id).await? else {
            bail_not_found!();
        };

        let updated_at = Event::update_confirmation(&state.db, event.id, form.subject, form.content).await?;

        Ok(Json(EditConfirmationResponse {
            id: Some(event.id),
            updated_at: Some(updated_at.and_utc().timestamp_millis()),
            error: None,
        }))
    }

    // Preview confirmation page.
    pub async fn preview_confirmation_page(
        State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "emails/event_confirmation.html")]
        struct PreviewConfirmationHtml {
            email_id: i64,
            event: Event,
            token: String,
        }
        Ok(
            PreviewConfirmationHtml { email_id: 0, event: event.clone(), token: "xxxxxxxx".into() }
                .into_response(),
        )
    }

    // Edit dayof page.
    pub async fn edit_dayof_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/edit_dayof.html")]
        struct EditDayofHtml {
            user: Option<User>,
            event: Event,
            editor: Editor,
        }
        Ok(EditDayofHtml {
            user: Some(user),
            event: event.clone(),
            editor: Editor {
                url: "/events/{id}/dayof/edit",
                snapshot_prefix: "event/dayof",
                entity_id: Some(event.id),
                content: match (event.dayof_html, event.dayof_updated_at) {
                    (Some(html), Some(updated_at)) => Some(EditorContent { html, updated_at }),
                    _ => None,
                },
            },
        }
        .into_response())
    }

    // Edit dayof form.
    #[derive(serde::Deserialize)]
    pub struct EditDayofForm {
        id: i64,
        subject: String,
        content: String,
    }
    #[derive(serde::Serialize)]
    pub struct EditDayofResponse {
        id: Option<i64>,
        updated_at: Option<i64>,
        error: Option<String>,
    }
    pub async fn edit_dayof_form(
        State(state): State<SharedAppState>, Form(form): Form<EditDayofForm>,
    ) -> JsonResult<EditDayofResponse> {
        let Some(event) = Event::lookup_by_id(&state.db, form.id).await? else {
            bail_not_found!();
        };

        let updated_at = Event::update_dayof(&state.db, event.id, form.subject, form.content).await?;

        Ok(Json(EditDayofResponse {
            id: Some(event.id),
            updated_at: Some(updated_at.and_utc().timestamp_millis()),
            error: None,
        }))
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "emails/event_dayof.html")]
    struct DayofEmailHtml {
        email_id: i64,
        event: Event,
    }
    // Preview dayof page.
    pub async fn preview_dayof_page(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        Ok(DayofEmailHtml { email_id: 0, event: event.clone() }.into_response())
    }

    /// Display the form to send a post.
    pub async fn send_dayof_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(sqlx::FromRow)]
        struct Counts {
            count: i64,
            sent: i64,
        }
        let list = sqlx::query_as!(
            Counts,
            r#"
            SELECT
                COUNT(r.user_id) AS count,
                COALESCE(SUM(
                    CASE WHEN EXISTS (
                        SELECT 1
                        FROM emails e
                        WHERE e.kind = ?
                          AND e.user_id = r.user_id
                          AND e.event_id = rs.event_id
                          AND e.sent_at IS NOT NULL
                    )
                    THEN 1 ELSE 0 END
                ), 0) AS sent
            FROM rsvps r
            JOIN rsvp_sessions rs ON rs.id = r.session_id
            WHERE rs.event_id = ?
                AND (rs.status = ? OR rs.status = ?)
            "#,
            Email::EVENT_DAYOF,
            event.id,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_one(&state.db)
        .await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/send_dayof.html")]
        struct SendHtml {
            user: Option<User>,
            list: Counts,
            event: Event,
            ratelimit: usize,
        }
        let ratelimit = state.config.email.ratelimit;
        Ok(SendHtml { user: Some(user), event, list, ratelimit }.into_response())
    }

    pub async fn send_dayof_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!();
        };

        let emails = Email::create_send_dayof_batch(&state.db, event.id).await?;

        let mut email_template = DayofEmailHtml { email_id: 0, event: event.clone() };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.email_id = id;

            let from = &state.config.email.from;
            let reply_to = config().email.contact_to.as_ref().unwrap_or(from);
            let message = state
                .mailer
                .builder()
                .to(address.parse().unwrap())
                .reply_to(reply_to.clone())
                .subject(event.dayof_subject.as_deref().expect("missing dayof_subject"))
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(email_template.render()?)
                .unwrap();

            messages.push(message);
            email_ids.push(id);
        }

        event.mark_sent_dayof(&state.db).await?;

        let email_ids = futures::stream::iter(email_ids);
        let results = state.mailer.send_batch(Arc::clone(&state), messages).await;

        let body = Body::from_stream(async_stream::stream! {
            let mut stream = Box::pin(results.zip(email_ids));
            while let Some((progress, email_id)) = stream.next().await {
                let json = match progress {
                    Ok(p) => {
                        Email::mark_sent(&state.db, email_id).await?;
                        json!({"sent": p.sent, "remaining": p.remaining})
                    }
                    Err(e) => {
                        let e = e.message();
                        Email::mark_error(&state.db, email_id, e).await?;
                        json!({"error": e})
                    }
                }.to_string();
                yield Ok::<_, AnyError>(format!("{json}\n"));
            }
        });

        Ok(body.into_response())
    }

    // Edit description page.
    pub async fn edit_description_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/edit_description.html")]
        struct EditDescriptionHtml {
            user: Option<User>,
            event: Event,
            editor: Editor,
        }
        Ok(EditDescriptionHtml {
            user: Some(user),
            event: event.clone(),
            editor: Editor {
                url: "/events/{id}/description/edit",
                snapshot_prefix: "event/description",
                entity_id: Some(event.id),
                content: match (event.description_html, event.description_updated_at) {
                    (Some(html), Some(updated_at)) => Some(EditorContent { html, updated_at }),
                    _ => None,
                },
            },
        }
        .into_response())
    }

    // Edit description form.
    #[derive(serde::Deserialize)]
    pub struct EditDescriptionForm {
        id: i64,
        content: String,
    }
    #[derive(serde::Serialize)]
    pub struct EditDescriptionResponse {
        id: Option<i64>,
        updated_at: Option<i64>,
        error: Option<String>,
    }
    pub async fn edit_description_form(
        State(state): State<SharedAppState>, Form(form): Form<EditDescriptionForm>,
    ) -> JsonResult<EditDescriptionResponse> {
        let Some(event) = Event::lookup_by_id(&state.db, form.id).await? else {
            bail_not_found!();
        };

        let updated_at = Event::update_description(&state.db, event.id, form.content).await?;

        Ok(Json(EditDescriptionResponse {
            id: Some(event.id),
            updated_at: Some(updated_at.and_utc().timestamp_millis()),
            error: None,
        }))
    }

    /// View an event.
    pub async fn attendees_page(
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let rsvps = Rsvp::list_for_admin_attendees(&state.db, event.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/attendees.html")]
        struct Html {
            pub user: Option<User>,
            event: Event,
            rsvps: Vec<AdminAttendeesRsvp>,
        }

        Ok(Html { user: Some(user), event, rsvps }.into_response())
    }

    /// Handle delete submission.
    pub async fn delete_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        Event::delete(&state.db, id).await?;
        Ok(Redirect::to("/events").into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct CheckinPath {
        #[allow(unused)]
        slug: String,
        rsvp_id: i64,
    }

    pub async fn set_checkin(
        State(state): State<SharedAppState>, Path(path): Path<CheckinPath>,
    ) -> JsonResult<()> {
        Rsvp::set_checkin_at(&state.db, path.rsvp_id).await?;
        Ok(Json(()))
    }

    pub async fn clear_checkin(
        State(state): State<SharedAppState>, Path(path): Path<CheckinPath>,
    ) -> JsonResult<()> {
        Rsvp::clear_checkin_at(&state.db, path.rsvp_id).await?;
        Ok(Json(()))
    }
}

mod rsvp {
    use std::collections::HashSet;

    use super::*;
    use crate::app::events::rsvp::parse::ParsedAttendee;
    use crate::db::list::List;
    use crate::db::rsvp::{AttendeeRsvp, ContributionRsvp, CreateRsvp, EventRsvp, Rsvp};
    use crate::db::rsvp_session::RsvpSession;
    use crate::db::user::CreateUser;
    use crate::utils::sentry;

    #[derive(Template, WebTemplate)]
    #[template(path = "error_simple.html")]
    struct ErrorHtml {
        user: Option<User>,
        message: String,
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "message_simple.html")]
    struct MessageHtml {
        user: Option<User>,
        title: String,
        message: String,
    }

    #[derive(serde::Deserialize)]
    pub struct RsvpQuery {
        email: Option<String>,
    }
    /// Create an RSVP session after a user clicks the RSVP button for an event.
    pub async fn rsvp_form(
        session: Option<RsvpSession>, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Query(query): Query<RsvpQuery>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &None).await;
        }

        match event.guest_list_id {
            None => goto::selection_page(&state.db, &None, &session, &event).await,
            Some(guest_list_id) => match session {
                Some(session) => {
                    if let Some(user_id) = session.user_id
                        && List::has_user_id(&state.db, guest_list_id, user_id).await?
                    {
                        goto::selection_page(&state.db, &None, &Some(session), &event).await
                    } else {
                        goto::guestlist_page(&event)
                    }
                }
                _ => match query.email {
                    Some(email) => match List::has_email(&state.db, guest_list_id, &email).await? {
                        true => goto::selection_page(&state.db, &None, &session, &event).await,
                        false => goto::error_not_on_guestlist(),
                    },
                    _ => goto::guestlist_page(&event),
                },
            },
        }
    }

    // Display the "Are you on the list?" page
    pub async fn guestlist_page(State(state): State<SharedAppState>, Path(slug): Path<String>) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &None).await;
        }

        let _guest_list_id = event.guest_list_id.ok_or_else(invalid)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_guestlist.html")]
        struct GuestlistHtml {
            user: Option<User>,
            slug: String,
        }
        Ok(GuestlistHtml { user: None, slug }.into_response())
    }

    // Handle submission of the "Are you on the list?" form
    #[derive(Debug, serde::Deserialize)]
    pub struct GuestlistForm {
        email: String,
    }
    pub async fn guestlist_form(
        mut session: Option<RsvpSession>, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<GuestlistForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let guest_list_id = event.guest_list_id.ok_or_else(invalid)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &None).await;
        }

        // If they're on the list, there must be a corresponding user.
        let Some(user) = User::lookup_by_email(&state.db, &form.email).await? else {
            return goto::error_not_on_guestlist();
        };
        let primary_user = CreateUser {
            email: user.email.clone(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            phone: user.phone.clone(),
        };

        match List::has_user_id(&state.db, guest_list_id, user.id).await? {
            true => {
                // Check for conflicts
                let other_users =
                    Rsvp::list_reserved_users_for_event(&state.db, &event, session.as_ref()).await?;
                use validate::Conflict;
                if let Some(Conflict::Guest { email, status } | Conflict::Primary { email, status }) =
                    validate::no_conflicts(&other_users, &primary_user, &[])
                {
                    return goto::error_conflict(&email, &status);
                }

                // Set user on session if already exists
                if let Some(session) = session.as_mut() {
                    session.set_user(&state.db, &user).await?;
                }

                goto::selection_page(&state.db, &Some(user), &session, &event).await
            }
            false => goto::error_not_on_guestlist(),
        }
    }

    // Display the "Choose a contribution" page
    pub async fn selection_page(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(session)).await;
        }

        let user = session.user(&state.db).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        let our_rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        let mut our_qtys = HashMap::default();
        let mut our_contributions = HashMap::default();
        for rsvp in our_rsvps {
            *our_qtys.entry(rsvp.spot_id).or_default() += 1;
            our_contributions.insert(rsvp.spot_id, rsvp.contribution);
        }

        let other_rsvps = Rsvp::list_reserved_for_event(&state.db, &event, &session).await?;
        let limits = event.compute_limits(&user, &spots, &other_rsvps);
        if limits.total_limit == 0 {
            return goto::error_registration_closed(&state.db, &None).await;
        }
        let stats = Spot::stats(&spots, &other_rsvps);

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_selection.html")]
        struct SelectionHtml {
            event: Event,
            spots: Vec<Spot>,
            our_qtys: HashMap<i64, usize>,
            our_contributions: HashMap<i64, i64>,
            limits: EventLimits,
            stats: SpotStats,
        }
        Ok(SelectionHtml { event, spots, our_qtys, our_contributions, limits, stats }.into_response())
    }

    // Handle submission of the "Choose a contribution" form
    #[derive(Debug, serde::Deserialize)]
    pub struct SelectionForm {
        rsvps: String,
    }
    pub async fn selection_form(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<SelectionForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(session)).await;
        }

        let user = session.user(&state.db).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        // Parse and validate form
        let our_rsvps = parse::selection_form(&spots, &form.rsvps).map_err(|e| {
            sentry::report(format!("selection_form(): session={} form={form:?}: {e}", session.token));
            invalid()
        })?;

        // Verify limits
        let other_rsvps = Rsvp::list_reserved_for_event(&state.db, &event, &session).await?;
        let limits = event.compute_limits(&user, &spots, &other_rsvps);
        if limits.total_limit == 0 {
            return goto::error_registration_closed(&state.db, &None).await;
        }
        if !validate::within_limits(&limits, &our_rsvps) {
            return goto::error_spot_taken(&state.db, &session).await;
        }

        // Delete any old and create new RSVPs
        Rsvp::delete_for_session(&state.db, session.id).await?;
        for rsvp in our_rsvps {
            Rsvp::create(
                &state.db,
                CreateRsvp {
                    session_id: session.id,
                    spot_id: rsvp.spot_id,
                    contribution: rsvp.contribution,
                    user_id: None,
                    user_version: None,
                },
            )
            .await?;
        }
        session.clear_stripe_client_secret(&state.db).await?;
        session.set_status(&state.db, RsvpSession::ATTENDEES).await?;

        // TODO: skip to /contribution if only one spot and RsvpSession already has an associated user
        goto::attendees_page(&event)
    }

    #[derive(PartialEq)]
    enum AttendeesMode {
        Create,
        Edit,
    }
    #[derive(Template, WebTemplate)]
    #[template(path = "events/rsvp_attendees.html")]
    struct AttendeesHtml {
        mode: AttendeesMode,
        event: Event,
        user: Option<User>,
        session: RsvpSession,
        attendees: Vec<AttendeeRsvp>,
        price: i64,
    }
    // Display the "Who will be attending?" page after submitting spots
    pub async fn attendees_page(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let user = session.user(&state.db).await?;
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(session)).await;
        }

        let attendees = Rsvp::list_for_attendees(&state.db, session.id).await?;
        let price = attendees.iter().map(|r| r.contribution).sum::<i64>();
        let mode = AttendeesMode::Create;
        Ok(AttendeesHtml { mode, event, user, session, attendees, price }.into_response())
    }

    // Handle submission of the "Who will be attending?" form
    #[derive(Debug, serde::Deserialize)]
    pub struct AttendeesForm {
        attendees: String,
    }
    pub async fn attendees_form(
        mut our_session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<AttendeesForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(our_session)).await;
        }

        let user = our_session.user(&state.db).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let our_rsvps = Rsvp::list_for_session(&state.db, our_session.id).await?;

        // Parse and validate form
        let (primary_attendee, guest_attendees) =
            parse::attendees_form(&user, &our_rsvps, &form.attendees).await.map_err(|e| {
                sentry::report(format!("attendees_form(): session={} form={form:?}: {e}", our_session.token));
                invalid()
            })?;
        // Collect all users
        let primary_user = primary_attendee.user.clone();
        let guest_users = guest_attendees.iter().map(|a| a.user.clone()).collect::<Vec<_>>();
        let mut all_users = guest_users.clone();
        all_users.push(primary_user.clone());

        // Check for conflicts
        let other_users = Rsvp::list_reserved_users_for_event(&state.db, &event, Some(&our_session)).await?;
        if let Some(conflict) = validate::no_conflicts(&other_users, &primary_user, &guest_users) {
            use validate::Conflict;
            match conflict {
                // For guest conflicts, always show an error.
                Conflict::Guest { email, status } => return goto::error_conflict(&email, &status),
                // For primary conflicts...
                Conflict::Primary { email, status } => match status.as_str() {
                    // If in a draft status, "take it over" by deleting the conflicting session
                    RsvpSession::ATTENDEES | RsvpSession::SELECTION => {
                        our_session.takeover_for_event(&state.db, &event, &email).await?;
                    }
                    // If reserved, show an error.
                    RsvpSession::CONTRIBUTION
                    | RsvpSession::PAYMENT_PENDING
                    | RsvpSession::PAYMENT_CONFIRMED => {
                        return goto::error_conflict(&email, &status);
                    }
                    _ => unreachable!(),
                },
            }
        }

        // Verify limits in case of preemption since `selection_form()` submission.
        // Once we transition to CONTRIBUTION, our rsvps spots are held.
        let other_rsvps = Rsvp::list_reserved_for_event(&state.db, &event, &our_session).await?;
        let limits = event.compute_limits(&user, &spots, &other_rsvps);
        if limits.total_limit == 0 {
            return goto::error_registration_closed(&state.db, &None).await;
        }
        if !validate::within_limits(&limits, &our_rsvps) {
            return goto::error_spot_taken(&state.db, &our_session).await;
        }

        // Create and store primary user on RsvpSession and Rsvp
        let primary_user = User::update_or_create(&state.db, &primary_user).await?;
        our_session.set_user(&state.db, &primary_user).await?;
        Rsvp::set_user(&state.db, primary_attendee.rsvp_id, &primary_user).await?;

        // Create and store users on guest Rsvps
        for ParsedAttendee { rsvp_id, user } in guest_attendees {
            let user = User::update_or_create(&state.db, &user).await?;
            Rsvp::set_user(&state.db, rsvp_id, &user).await?;
        }

        our_session.set_status(&state.db, RsvpSession::CONTRIBUTION).await?;
        goto::contribution_page(&event)
    }

    // Display the "Make your contribution" page after submitting attendees
    pub async fn contribution_page(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(session)).await;
        }

        // A user is guaranteed to exist, since either:
        // * There already was one in rsvp_form() and we redirected straight here (TODO, we don't redirect yet)
        // * We've collected their info and just linked one in attendees_form()
        let user = User::lookup_by_id(&state.db, session.user_id.unwrap()).await?.unwrap();
        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;

        let price = rsvps.iter().map(|r| r.contribution).sum();
        if price > 0 {
            let line_items = session.line_items(&rsvps)?;
            let return_url = format!("/e/{slug}/rsvp/manage?reservation={}", session.token);

            // Clear expired stripe sessions (older than 14 minutes)
            if session.stripe_client_secret.is_some() && session.is_stripe_expired() {
                session.clear_stripe_client_secret(&state.db).await?;
            }

            if session.stripe_client_secret.is_none() {
                let stripe_client_secret = state
                    .stripe
                    .create_session(session.id, &user.email, line_items, return_url)
                    .await?;

                session.set_stripe_client_secret(&state.db, &stripe_client_secret).await?;
            }
        }

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_contribution.html")]
        struct ContributionHtml {
            event: Event,
            session: RsvpSession,
            rsvps: Vec<ContributionRsvp>,
            price: i64,
            stripe_publishable_key: String,
        }
        Ok(ContributionHtml {
            event,
            session,
            rsvps,
            price,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        }
        .into_response())
    }

    // Handle submission of $0 RSVPs.
    pub async fn contribution_form(
        State(state): State<SharedAppState>, session: RsvpSession, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if !validate::registration_open(&event) {
            return goto::error_registration_closed(&state.db, &Some(session)).await;
        }

        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;
        let price: i64 = rsvps.iter().map(|r| r.contribution).sum();
        match price {
            0 => session.set_status(&state.db, RsvpSession::PAYMENT_CONFIRMED).await?,
            _ => bail_invalid!(),
        }

        // Send confirmation email
        let user_id = session.user_id.ok_or_else(invalid)?;
        let user = User::lookup_by_id(&state.db, user_id).await?.ok_or_else(invalid)?;

        if !Email::have_sent_confirmation(&state.db, event.id, user_id).await? {
            let email = Email::create_confirmation(&state.db, event.id, user_id).await?;

            #[derive(Template, WebTemplate)]
            #[template(path = "emails/event_confirmation.html")]
            struct ConfirmationEmailHtml {
                email_id: i64,
                event: Event,
                token: String,
            }

            let from = &state.config.email.from;
            let reply_to = state.config.email.contact_to.as_ref().unwrap_or(from);
            let subject = event
                .confirmation_subject
                .clone()
                .unwrap_or_else(|| format!("Confirmation for {}", event.title));
            let message = state
                .mailer
                .builder()
                .to(user.email.parse().unwrap())
                .reply_to(reply_to.clone())
                .subject(subject)
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(
                    ConfirmationEmailHtml { email_id: email.id, event, token: session.token.clone() }
                        .render()?,
                )
                .unwrap();

            state.mailer.send(&message).await?;
        }

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/manage?reservation={}", &session.token)).into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct SessionQuery {
        reservation: String,
    }
    // Show the "Manage your RSVP" page.
    pub async fn manage_page(
        user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<SessionQuery>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.reservation).await? else {
            let error = ErrorHtml { user: user.clone(), message: "Reservation not found.".into() };
            return Ok(error.into_response());
        };
        let Some(user_id) = session.user_id else {
            bail_invalid!()
        };
        let Some(session_user) = User::lookup_by_id(&state.db, user_id).await? else {
            bail_invalid!()
        };

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        match session.status.as_str() {
            RsvpSession::SELECTION => {
                return goto::selection_page(&state.db, &None, &Some(session), &event).await;
            }
            RsvpSession::ATTENDEES => return goto::attendees_page(&event),
            // If you get here, we hold your spot and assume payment is coming later via webhook.
            // This is technically exploitable, but we could check for still unpaid rsvps at event start.
            RsvpSession::CONTRIBUTION => session.set_status(&state.db, RsvpSession::PAYMENT_PENDING).await?,
            // If pending or confirmed, you're good.
            RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED => {}
            _ => unreachable!(),
        }

        if !Email::have_sent_confirmation(&state.db, session.event_id, user_id).await? {
            let email = Email::create_confirmation(&state.db, session.event_id, user_id).await?;

            #[derive(Template, WebTemplate)]
            #[template(path = "emails/event_confirmation.html")]
            struct ConfirmationEmailHtml {
                email_id: i64,
                event: Event,
                token: String,
            }

            let from = &state.config.email.from;
            let reply_to = state.config.email.contact_to.as_ref().unwrap_or(from);
            let message = state
                .mailer
                .builder()
                .to(session_user.email.parse().unwrap())
                .reply_to(reply_to.clone())
                .subject(
                    event
                        .confirmation_subject
                        .clone()
                        .unwrap_or_else(|| format!("Confirmation for {}", event.title)),
                )
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(
                    ConfirmationEmailHtml {
                        email_id: email.id,
                        event: event.clone(),
                        token: session.token.clone(),
                    }
                    .render()?,
                )
                .unwrap();

            state.mailer.send(&message).await?;

            // If dayof email has been sent out, also send it to this new RSVP
            if event.dayof_sent_at.is_some() {
                let dayof_email = Email::create_send_dayof_single(&state.db, event.id, user_id).await?;

                #[derive(Template, WebTemplate)]
                #[template(path = "emails/event_dayof.html")]
                struct DayofEmailHtml {
                    email_id: i64,
                    event: Event,
                }

                let dayof_message = state
                    .mailer
                    .builder()
                    .to(session_user.email.parse().unwrap())
                    .reply_to(reply_to.clone())
                    .subject(event.dayof_subject.as_deref().expect("missing dayof_subject"))
                    .header(lettre::message::header::ContentType::TEXT_HTML)
                    .body(DayofEmailHtml { email_id: dayof_email.id, event: event.clone() }.render()?)
                    .unwrap();

                state.mailer.send(&dayof_message).await?;
            }
        }

        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;
        let price = rsvps.iter().map(|r| r.contribution).sum::<i64>();

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_manage.html")]
        struct ManageHtml {
            user: Option<User>,
            session: RsvpSession,
            event: Event,
            flyer: Option<EventFlyer>,
            rsvps: Vec<ContributionRsvp>,
            price: i64,
        }
        Ok(ManageHtml { user, session, event, flyer, rsvps, price }.into_response())
    }
    // Show the "Manage your RSVP" page.
    pub async fn temp_delete(
        State(state): State<SharedAppState>, Query(query): Query<SessionQuery>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let session = RsvpSession::lookup_by_token(&state.db, &query.reservation)
            .await?
            .ok_or_else(not_found)?;
        session.delete(&state.db).await?;
        Ok(Redirect::to(&format!("/e/{slug}")).into_response())
    }

    // Show the editor for "Who will be attending?" page.
    pub async fn edit_guests_page(
        user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<SessionQuery>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.reservation).await? else {
            // A nonexistant session should never reach /edit, and a confirmed session should never be deleted.
            bail_not_found!();
        };

        let rsvps = Rsvp::list_for_attendees(&state.db, session.id).await?;
        let mode = AttendeesMode::Edit;
        Ok(AttendeesHtml { mode, event, user, session, attendees: rsvps, price: 0 }.into_response())
    }
    pub async fn edit_guests_form(
        State(state): State<SharedAppState>, Query(query): Query<SessionQuery>, Path(slug): Path<String>,
        Form(form): Form<AttendeesForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let session = RsvpSession::lookup_by_token(&state.db, &query.reservation)
            .await?
            .ok_or_else(invalid)?;
        let user = session.user(&state.db).await?;
        let our_rsvps = Rsvp::list_for_session(&state.db, session.id).await?;

        // Parse and validate form. NOTE that we only allow editing guest info.
        // The primary_attendee form is disabled on the frontend, and changes are ignored here.
        let (primary_attendee, guest_attendees) =
            parse::attendees_form(&user, &our_rsvps, &form.attendees).await.map_err(|e| {
                sentry::report(format!("edit_form(): session={} form={form:?}: {e}", session.token));
                tracing::error!("{}", format!("edit_form(): session={} form={form:?}: {e}", session.token));
                invalid()
            })?;

        // Check for conflicts
        let primary_user = primary_attendee.user.clone();
        let guest_users = guest_attendees.iter().map(|a| a.user.clone()).collect::<Vec<_>>();
        let other_users = Rsvp::list_reserved_users_for_event(&state.db, &event, Some(&session)).await?;
        use validate::Conflict;
        if let Some(Conflict::Guest { email, status } | Conflict::Primary { email, status }) =
            validate::no_conflicts(&other_users, &primary_user, &guest_users)
        {
            return goto::error_conflict(&email, &status);
        }

        // Update guests
        for ParsedAttendee { rsvp_id, user } in guest_attendees {
            let user = User::update_or_create(&state.db, &user).await?;
            Rsvp::set_user(&state.db, rsvp_id, &user).await?;
        }

        goto::manage_page(&session, &event)
    }

    /// Helpers for changing RSVP session state and redirecting.
    #[rustfmt::skip]
    pub mod goto {
        use super::*;

        pub fn guestlist_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/guestlist", &event.slug)).into_response())
        }
        pub async fn selection_page(db: &Db, user: &Option<User>, session: &Option<RsvpSession>, event: &Event) -> HtmlResult {
            let headers = RsvpSession::get_or_create(db, user, session, event.id).await?;
            Ok((headers, Redirect::to(&format!("/e/{}/rsvp/selection", &event.slug))).into_response())
        }
        pub fn attendees_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/attendees", &event.slug)).into_response())
        }
        pub fn contribution_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/contribution", &event.slug)).into_response())
        }
        pub fn manage_page(session: &RsvpSession, event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/manage?reservation={}", &event.slug, &session.token)).into_response())
        }

        pub fn error_not_on_guestlist() -> HtmlResult {
            let error = ErrorHtml { user: None, message: "Sorry, you're not on the list.".into() };
            Ok(error.into_response())
        }
        pub async fn error_registration_closed(db: &Db, session: &Option<RsvpSession>) -> HtmlResult {
            if let Some(session) = session {
                session.delete(db).await?;
            }
            Ok(MessageHtml {
                user: None,
                title: "Sorry".into(),
                message: "Registration for this event is closed.".into(),
            }
            .into_response())
        }
        pub async fn error_spot_taken(db: &Db, session: &RsvpSession) -> HtmlResult {
            session.delete(db).await?;
            Ok(ErrorHtml {
                user: None,
                message: "Sorry, a spot you selected was taken. Please try again.".to_string(),
            }
            .into_response())
        }
        pub fn error_conflict(email: &str, status: &str) -> HtmlResult {
            let wording = match status {
                RsvpSession::SELECTION | RsvpSession::ATTENDEES | RsvpSession::CONTRIBUTION => "is currently in the process of RSVPing",
                RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED => "has already RSVPed",
                _ => unreachable!()
            };
            Ok(ErrorHtml {
                message: format!("Someone {wording} for {email}."),
                user: None,
            }.into_response())
        }
    }

    mod validate {
        use super::*;
        use crate::db::rsvp::UserRsvp;

        pub fn registration_open(event: &Event) -> bool {
            !event.closed
        }

        /// Returns true if rsvps satisfy total and per-spot limits.
        pub fn within_limits(limits: &EventLimits, rsvps: &[EventRsvp]) -> bool {
            let total_qty = rsvps.len() as i64;
            let mut spot_qtys: HashMap<i64, i64> = HashMap::default();
            for rsvp in rsvps {
                *spot_qtys.entry(rsvp.spot_id).or_default() += 1;
            }

            if total_qty > limits.total_limit {
                return false;
            }
            for (spot_id, spot_qty) in spot_qtys {
                if spot_qty > *limits.spot_limits.get(&spot_id).unwrap_or(&0) {
                    return false;
                }
            }

            true
        }

        pub enum Conflict {
            Primary { email: String, status: String },
            Guest { email: String, status: String },
        }
        #[rustfmt::skip]
        pub fn no_conflicts(
            other_users: &[UserRsvp], primary: &CreateUser, guests: &[CreateUser],
        ) -> Option<Conflict> {
            for other_user in other_users {
                if primary.email == other_user.email {
                    return Some(Conflict::Primary { email: other_user.email.clone(), status: other_user.status.clone() })
                }

                for guest in guests {
                    if guest.email == other_user.email {
                        return Some(Conflict::Guest { email: other_user.email.clone(), status: other_user.status.clone() });
                    }
                }
            }
            None
        }
    }

    mod parse {
        use super::*;

        #[derive(thiserror::Error, Debug)]
        pub enum ParseSelectionError {
            #[error("failed to parse request JSON")]
            Parse,
            #[error("unknown spot_id={spot_id}")]
            UnknownSpot { spot_id: i64 },

            #[error("contribution is outside of range for spot_id={spot_id}")]
            SpotRange { spot_id: i64 },
        }
        pub fn selection_form(
            spots: &[Spot], selection: &str,
        ) -> Result<Vec<EventRsvp>, ParseSelectionError> {
            type Error = ParseSelectionError;

            #[derive(Debug, serde::Deserialize)]
            pub struct RsvpForm {
                spot_id: i64,
                qty: i64,
                contribution: Option<i64>,
            }

            let rsvps: Vec<RsvpForm> = serde_json::from_str(selection).map_err(|_| Error::Parse)?;
            let mut parsed = vec![];

            for rsvp in rsvps {
                let spot_id = rsvp.spot_id;
                let Some(spot) = spots.iter().find(|s| s.id == spot_id) else {
                    return Err(Error::UnknownSpot { spot_id });
                };

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

                for _ in 0..rsvp.qty {
                    parsed.push(EventRsvp { rsvp_id: 0, spot_id, contribution })
                }
            }

            Ok(parsed)
        }

        #[derive(Clone)]
        pub struct ParsedAttendee {
            pub rsvp_id: i64,
            pub user: CreateUser,
        }
        #[derive(thiserror::Error, Debug)]
        pub enum ParseAttendeesError {
            #[error("failed to parse request JSON")]
            Parse,

            #[error("unknown or duplicate rsvp_id={rsvp_id}")]
            UnknownOrDuplicateRsvp { rsvp_id: i64 },
            #[error("missing attendee for rsvp_ids={rsvp_ids:?}")]
            MissingAttendee { rsvp_ids: Vec<i64> },
            #[error("missing attendee with is_me=true")]
            MissingPrimary,
            #[error("multiple attendees with is_me=true")]
            MultiplePrimary,
            #[error("modified attendee with is_me=true")]
            PrimaryChanged,
            #[error("invalid name: first={first_name:?} last={last_name:?}")]
            InvalidName { first_name: String, last_name: String },
            #[error("invalid phone number: {phone}")]
            InvalidPhone { phone: String },
            #[error("duplicate email: {email}")]
            DuplicateEmail { email: String },
            #[error("duplicate phone: {phone}")]
            DuplicatePhone { phone: String },
        }
        pub async fn attendees_form(
            session_user: &Option<User>, rsvps: &[EventRsvp], attendees: &str,
        ) -> Result<(ParsedAttendee, Vec<ParsedAttendee>), ParseAttendeesError> {
            type Error = ParseAttendeesError;

            #[derive(Debug, serde::Deserialize)]
            pub struct AttendeeForm {
                rsvp_id: i64,

                first_name: String,
                last_name: String,
                email: String,
                phone: Option<String>,

                is_me: bool,
            }
            let attendees: Vec<AttendeeForm> = serde_json::from_str(attendees).map_err(|_| Error::Parse)?;

            // Track available rsvp_ids, seen email/phones for duplicate detection
            let mut remaining_rsvps: HashSet<i64> = HashSet::from_iter(rsvps.iter().map(|r| r.rsvp_id));
            let mut seen_emails: HashSet<String> = HashSet::default();
            let mut seen_phones: HashSet<String> = HashSet::default();

            // Extract primary/guest attendees from form, and map to rsvps.
            let mut primary_attendee = None;
            let mut guest_attendees = vec![];
            for AttendeeForm { rsvp_id, first_name, last_name, email, phone, is_me } in attendees {
                // Validate rsvp_id
                if !remaining_rsvps.remove(&rsvp_id) {
                    return Err(Error::UnknownOrDuplicateRsvp { rsvp_id });
                }

                // Validate name
                if first_name.is_empty() || last_name.is_empty() {
                    return Err(Error::InvalidName { first_name, last_name });
                }

                // Validate email/phone and check for duplicates
                if !seen_emails.insert(email.clone()) {
                    return Err(Error::DuplicateEmail { email });
                }
                let phone = parse_phone(phone)?;
                if let Some(phone) = phone.clone()
                    && !seen_phones.insert(phone.clone())
                {
                    return Err(Error::DuplicatePhone { phone });
                }

                let user =
                    CreateUser { first_name: Some(first_name), last_name: Some(last_name), email, phone };
                let attendee = ParsedAttendee { rsvp_id, user };
                if is_me {
                    // Disallow changing is_me email when it's already set on the session (it's disabled on the frontend)
                    if session_user.as_ref().is_some_and(|u| attendee.user.email != u.email) {
                        return Err(Error::PrimaryChanged);
                    }

                    match primary_attendee {
                        Some(_) => return Err(Error::MultiplePrimary),
                        None => primary_attendee = Some(attendee),
                    }
                } else {
                    guest_attendees.push(attendee);
                }
            }
            // Ensure exactle one primary attendee.
            let Some(primary_attendee) = primary_attendee else {
                return Err(Error::MissingPrimary);
            };

            // Ensure no remaining rsvps without an attendee specified
            if !remaining_rsvps.is_empty() {
                let rsvp_ids = remaining_rsvps.into_iter().collect();
                return Err(Error::MissingAttendee { rsvp_ids });
            }

            Ok((primary_attendee, guest_attendees))
        }
        /// Normalize phone to E.164 format.
        /// Empty string is ok (returns None). 10 digits assumes +1. 11-15 digits assumes leading +.
        fn parse_phone(phone: Option<String>) -> Result<Option<String>, ParseAttendeesError> {
            let Some(phone) = phone else { return Ok(None) };
            if phone.trim().is_empty() {
                return Ok(None);
            };

            let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
            match digits.len() {
                10 => Ok(Some(format!("+1{digits}"))),
                11..=15 => Ok(Some(format!("+{digits}"))),
                _ => Err(ParseAttendeesError::InvalidPhone { phone }),
            }
        }
    }
}

pub fn add_middleware(router: AxumRouter, state: SharedAppState) -> AxumRouter {
    /// Middleware layer to lookup add an `RsvpSession` to the request if an rsvp_session token is present.
    /// Also blocks RSVP pages (except manage/edit) when registration is closed.
    pub async fn rsvp_session_middleware(
        State(state): State<SharedAppState>, cookies: CookieJar, mut request: Request, next: Next,
    ) -> HtmlResult {
        let is_rsvp_path = request.uri().path().contains("/rsvp");

        if let Some(token) = cookies.get("rsvp_session")
            && let Some(session) = RsvpSession::lookup_by_token(&state.db, token.value()).await?
        {
            // Don't remove stale cookies if session is not found (e.g. it expired).
            // They will be overwritten when a new session is created.
            request.extensions_mut().insert(session);
        }

        let mut res = next.run(request).await;

        if is_rsvp_path {
            // Prevent browser from storing these stateful pages in the back-forward cache
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }

        Ok(res)
    }
    router.layer(axum::middleware::from_fn_with_state(state, rsvp_session_middleware))
}

/// Enable extracting an `Option<RsvpSession>` in an events handler matching /e/{slug}.
impl axum::extract::OptionalFromRequestParts<SharedAppState> for RsvpSession {
    type Rejection = Infallible;
    async fn from_request_parts(
        parts: &mut Parts, _state: &SharedAppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(parts.extensions.get::<RsvpSession>().cloned())
    }
}
/// Enable extracting an `RsvpSession` in an events handler matching /e/{slug}.
/// * Redirects to /e/{slug} if no session is present.
/// * Redirects to /e/{slug}/rsvp/manage if rsvp is already completed.
impl axum::extract::FromRequestParts<SharedAppState> for RsvpSession {
    type Rejection = Redirect;
    async fn from_request_parts(parts: &mut Parts, _state: &SharedAppState) -> Result<Self, Self::Rejection> {
        fn parse_slug(url: &str) -> Option<&str> {
            let url = url.trim_start_matches('/');
            let (e, rest) = url.split_once('/')?;
            match e {
                "e" => {
                    let (slug, _rest) = rest.split_once('/')?;
                    Some(slug)
                }
                _ => None,
            }
        }
        let Some(slug) = parse_slug(parts.uri.path()) else {
            panic!(
                "RsvpSession extractor used at path={:?} not matching /e/{{slug}}",
                parts.uri.path()
            );
        };

        match parts.extensions.get::<RsvpSession>().cloned() {
            Some(session) => match session.status.as_str() {
                RsvpSession::SELECTION | RsvpSession::ATTENDEES | RsvpSession::CONTRIBUTION => Ok(session),
                RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED => {
                    match parts.uri.path().contains("manage") {
                        true => Ok(session), // avoid redirect loop
                        false => Err(Redirect::to(&format!(
                            "{}/e/{slug}/rsvp/manage?reservation={}",
                            config().app.url,
                            session.token
                        ))),
                    }
                }
                _ => unreachable!(),
            },
            None => Err(Redirect::to(&format!("/e/{slug}"))),
        }
    }
}

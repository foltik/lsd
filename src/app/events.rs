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
                .route("/e/{slug}/guestlist", get(rsvp::guestlist_page).post(rsvp::guestlist_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::selection_page).post(rsvp::selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::attendees_page).post(rsvp::attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::contribution_page).post(rsvp::contribution_form))
                .route("/e/{slug}/rsvp/manage", get(rsvp::manage_page).post(rsvp::temp_delete))
                .route("/e/{slug}/rsvp/edit", get(rsvp::edit_page).post(rsvp::edit_form))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/events", get(read::list_page))
                .route("/events/new", get(edit::new_page))
                .route("/events/{slug}/edit", get(edit::edit_page).post(edit::edit_form))
                .route("/events/{slug}/delete", post(edit::delete_form))
                .route("/events/{slug}/attendees", get(edit::attendees_page))
                .route("/events/{id}/invite/edit", get(edit::edit_invite_page).post(edit::edit_invite_form))
                .route("/events/{id}/invite/preview", get(edit::preview_invite_page))
                .route("/events/{id}/invite/send", get(edit::send_invite_page).post(edit::send_invite_form))
                .route("/events/{id}/confirmation/edit", get(edit::edit_confirmation_page).post(edit::edit_confirmation_form))
                .route("/events/{id}/confirmation/preview", get(edit::preview_confirmation_page))
                .route("/events/{id}/dayof/edit", get(edit::edit_dayof_page).post(edit::edit_dayof_form))
                .route("/events/{id}/dayof/preview", get(edit::preview_dayof_page))
                .route("/events/{id}/dayof/send", get(edit::send_dayof_page).post(edit::send_dayof_form))
        })
}

// View and list events.
mod read {
    use super::*;

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
                description: "".into(),
                start: Utc::now().naive_utc(),
                end: None,
                capacity: 0,
                unlisted: false,
                guest_list_id: None,

                invite_html: None,
                invite_updated_at: None,
                invite_sent_at: None,

                confirmation_html: None,
                confirmation_updated_at: None,

                dayof_html: None,
                dayof_updated_at: None,
                dayof_sent_at: None,

                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            spots: vec![],
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
        let has_flyer = EventFlyer::exists_for_event(&state.db, event.id).await?;
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml { user: Some(user), event, spots, has_flyer, lists }.into_response())
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
                // TODO: Disallow??
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
        content: String, // TODO: rename to html in editor
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

        let updated_at = Event::update_invite(&state.db, event.id, form.content).await?;

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
        struct Counts {
            count: i64,
            sent: i64,
        }
        let list = sqlx::query_as!(
            Counts,
            r#"
            SELECT
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
            list: Counts,
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
                .subject(format!("Invitation to {}", event.title))
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
        content: String, // TODO: rename to html in editor
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

        let updated_at = Event::update_confirmation(&state.db, event.id, form.content).await?;

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
        content: String, // TODO: rename to html in editor
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

        let updated_at = Event::update_dayof(&state.db, event.id, form.content).await?;

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
            RsvpSession::AWAITING_PAYMENT,
            RsvpSession::CONFIRMED,
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

        let emails = Email::create_send_dayof(&state.db, event.id).await?;

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
                .subject(format!("What to know for {}", event.title))
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
}

mod rsvp {
    use super::*;
    use crate::db::list::List;
    use crate::db::rsvp::{AttendeeRsvp, ContributionRsvp, CreateRsvp, Rsvp};
    use crate::db::rsvp_session::RsvpSession;
    use crate::db::user::{CreateUser, UpdateUser};

    #[derive(Template, WebTemplate)]
    #[template(path = "error_simple.html")]
    struct ErrorHtml {
        user: Option<User>,
        message: String,
    }

    fn goto_guestlist_page(event: &Event) -> HtmlResult {
        Ok(Redirect::to(&format!("/e/{}/guestlist", &event.slug)).into_response())
    }
    fn goto_guestlist_error(user: &Option<User>) -> HtmlResult {
        let error = ErrorHtml { user: user.clone(), message: "Sorry, you're not on the list.".into() };
        Ok(error.into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct RsvpQuery {
        email: Option<String>,
    }
    /// Create an RSVP session after a user clicks the RSVP button for an event.
    pub async fn rsvp_form(
        user: Option<User>, session: Option<RsvpSession>, State(state): State<SharedAppState>,
        Path(slug): Path<String>, Query(query): Query<RsvpQuery>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        match event.guest_list_id {
            None => goto_selection_page(&state.db, &user, &session, &event).await,
            Some(guest_list_id) => match session {
                Some(session) => {
                    if let Some(user_id) = session.user_id
                        && List::has_user_id(&state.db, guest_list_id, user_id).await?
                    {
                        goto_selection_page(&state.db, &user, &Some(session), &event).await
                    } else {
                        goto_guestlist_page(&event)
                    }
                }
                _ => match query.email {
                    Some(email) => match List::has_email(&state.db, guest_list_id, &email).await? {
                        true => goto_selection_page(&state.db, &user, &session, &event).await,
                        false => goto_guestlist_error(&user),
                    },
                    _ => goto_guestlist_page(&event),
                },
            },
        }
    }

    // Display the "Are you on the list?" page
    pub async fn guestlist_page(
        user: Option<User>, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let _guest_list_id = event.guest_list_id.ok_or_else(invalid)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_guestlist.html")]
        struct GuestlistHtml {
            user: Option<User>,
            slug: String,
        }
        Ok(GuestlistHtml { user, slug }.into_response())
    }

    // Handle submission of the "Are you on the list?" form
    #[derive(Debug, serde::Deserialize)]
    pub struct GuestlistForm {
        email: String,
    }
    pub async fn guestlist_form(
        user: Option<User>, session: Option<RsvpSession>, State(state): State<SharedAppState>,
        Path(slug): Path<String>, Form(form): Form<GuestlistForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let guest_list_id = event.guest_list_id.ok_or_else(invalid)?;

        match List::has_email(&state.db, guest_list_id, &form.email).await? {
            true => goto_selection_page(&state.db, &user, &session, &event).await,
            false => goto_guestlist_error(&user),
        }
    }

    // *Ensure a session exists*, and then goto the selection page.
    async fn goto_selection_page(
        db: &Db, user: &Option<User>, session: &Option<RsvpSession>, event: &Event,
    ) -> HtmlResult {
        let headers = RsvpSession::get_or_create(db, user, session, event.id).await?;
        Ok((headers, Redirect::to(&format!("/e/{}/rsvp/selection", &event.slug))).into_response())
    }
    // Display the "Choose a contribution" page
    pub async fn selection_page(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let stats = event.stats_for_session(&state.db, session.id).await?;

        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let mut spot_qtys = HashMap::default();
        let mut spot_contributions = HashMap::default();

        let rsvps = Rsvp::list_for_selection(&state.db, session.id).await?;
        for rsvp in rsvps {
            *spot_qtys.entry(rsvp.spot_id).or_default() += 1;
            spot_contributions.insert(rsvp.spot_id, rsvp.contribution);
        }

        let user_spots = Some(4);

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_selection.html")]
        struct SelectionHtml {
            event: Event,
            user_max_spots: Option<u64>,
            spots: Vec<Spot>,
            spot_qtys: HashMap<i64, usize>,
            spot_contributions: HashMap<i64, i64>,
            stats: EventStats,
        }
        Ok(SelectionHtml {
            event,
            user_max_spots: user_spots,
            spots,
            spot_qtys,
            spot_contributions,
            stats,
        }
        .into_response())
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
        let spots = Spot::list_for_event(&state.db, event.id).await?;

        // Compute ticket stats. This includes other users' pending RSVPs, but excludes those from our own session.
        let stats = event.stats_for_session(&state.db, session.id).await?;
        // Parse and validate the selection (checking capacity, etc.)
        let rsvps = parse_selection(&stats, &spots, &form.rsvps).map_err(|_| invalid())?;

        // Delete any pending RSVPs (in case the user goes back) and create new ones.
        Rsvp::delete_for_session(&state.db, session.id).await?;
        for rsvp in rsvps {
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

        // TODO: skip to /contribution if only one spot and RsvpSession already has an associated user
        goto_attendees_page(&event)
    }

    // Goto the attendees page.
    fn goto_attendees_page(event: &Event) -> HtmlResult {
        Ok(Redirect::to(&format!("/e/{}/rsvp/attendees", &event.slug)).into_response())
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
        user: Option<User>, session: RsvpSession, State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
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
        let rsvps = Rsvp::list_for_attendees(&state.db, our_session.id).await?;

        // Parse attendees and lookup or create associated users
        let (primary_attendee, guest_attendees) =
            parse_attendees(&rsvps, &form.attendees).await.map_err(|_| invalid())?;

        // Get or create the User for primary attendee
        let user = User::get_or_create(&state.db, &primary_attendee.info).await?;

        // Handle conflicts for the primary attendee.
        // If our session has no user, this is the first time they're RSVPing with this browser/device.
        if our_session.user_id.is_none() {
            if let Some(other_session) =
                RsvpSession::lookup_for_user_and_event(&state.db, &user, &event).await?
            {
                // If this user has an existing session for this event...
                match other_session.status.as_str() {
                    // If in DRAFT status, delete the draft
                    RsvpSession::DRAFT => {
                        other_session.delete(&state.db).await?;
                    }
                    // If they've already confirmed for this event, display an error.
                    // IMPORTANT: We can't just assume the existing session's cookie and goto /manage
                    // Otherwise, someone could type in any random email and be able to modify their RSVP.
                    RsvpSession::AWAITING_PAYMENT | RsvpSession::CONFIRMED => {
                        our_session.delete(&state.db).await?;
                        return Ok(ErrorHtml {
                            message: format!(
                                "You've already RSVPed for this event! \
                                 Manage your RSVP via the confirmation email that was sent to {}.",
                                &user.email
                            ),
                            user: Some(user),
                        }
                        .into_response());
                    }
                    _ => unreachable!(),
                }

                // ....replace it with ours.
                our_session.set_user(&state.db, &user).await?;
            } else {
                // If no existing session, populate user_id on our session.
                our_session.set_user(&state.db, &user).await?;
            }
        }

        // Handle conflicts for guest attendees.
        for ParsedAttendee { info, .. } in &guest_attendees {
            let email = &info.email;
            if let Some(status) = Rsvp::lookup_conflicts(&state.db, &our_session, &event, email).await? {
                let message = match status.as_str() {
                    RsvpSession::DRAFT => format!("Someone is in the process of RSVPing for {email}."),
                    _ => format!("Someone has already RSVPed for {email}."),
                };
                return Ok(ErrorHtml { user: Some(user), message }.into_response());
            }
        }

        // Get or create Users for guest attendees, now that conflicts are resolved.
        for ParsedAttendee { rsvp, info } in &guest_attendees {
            let guest_user = User::get_or_create(&state.db, info).await?;
            Rsvp::set_user(&state.db, rsvp.rsvp_id, &guest_user).await?;
        }

        goto_contribution_page(&event)
    }

    // Goto the contribution page.
    fn goto_contribution_page(event: &Event) -> HtmlResult {
        Ok(Redirect::to(&format!("/e/{}/rsvp/contribution", &event.slug)).into_response())
    }
    // Display the "Make your contribution" page after submitting attendees
    pub async fn contribution_page(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        // A user is guaranteed to exist, since either:
        // * There already was one in rsvp_form() and we redirected straight here (TODO, we don't redirect yet)
        // * We've collected their info and just linked one in attendees_form()
        let user = User::lookup_by_id(&state.db, session.user_id.unwrap()).await?.unwrap();
        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;

        let price = rsvps.iter().map(|r| r.contribution).sum();
        if price > 0 {
            let line_items = session.line_items(&rsvps)?;
            let return_url = format!("/e/{slug}/rsvp/manage?reservation={}", session.token);

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
        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;
        let price: i64 = rsvps.iter().map(|r| r.contribution).sum();
        match price {
            0 => session.set_confirmed(&state.db, None).await?,
            _ => bail_invalid!(),
        }
        Ok(Redirect::to(&format!("/e/{slug}/rsvp/manage?reservation={}", &session.token)).into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct SessionQuery {
        reservation: String,
    }
    // Goto the manage page.
    fn goto_manage_page(session: &RsvpSession, event: &Event) -> HtmlResult {
        Ok(
            Redirect::to(&format!("/e/{}/rsvp/manage?reservation={}", &event.slug, &session.token))
                .into_response(),
        )
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
            RsvpSession::CONFIRMED | RsvpSession::AWAITING_PAYMENT => {}
            // If you get here, we hold your spot and assume payment is coming later via webhook.
            // This is technically exploitable, but we could check for still unpaid rsvps at event start.
            RsvpSession::DRAFT => session.set_awaiting_payment(&state.db).await?,
            _ => bail_invalid!(),
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
                .subject(format!("Invitation to {}", event.title))
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
    pub async fn edit_page(
        user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<SessionQuery>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.reservation).await? else {
            // A nonexistant session should never reach /edit, and a confirmed session should never be deleted.
            bail_invalid!();
        };

        // Collect all guest rsvps
        let rsvps = Rsvp::list_for_attendees(&state.db, session.id)
            .await?
            .into_iter()
            .filter(|r| r.user_id != session.user_id)
            .collect::<Vec<_>>();

        let mode = AttendeesMode::Edit;
        Ok(AttendeesHtml { mode, event, user, session, attendees: rsvps, price: 0 }.into_response())
    }
    pub async fn edit_form(
        user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<SessionQuery>,
        Path(slug): Path<String>, Form(form): Form<AttendeesForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let session = RsvpSession::lookup_by_token(&state.db, &query.reservation)
            .await?
            .ok_or_else(invalid)?;

        // Collect all guest rsvps
        let rsvps = Rsvp::list_for_attendees(&state.db, session.id)
            .await?
            .into_iter()
            .filter(|r| r.user_id != session.user_id)
            .collect::<Vec<_>>();

        #[derive(Debug, serde::Deserialize)]
        pub struct AttendeeForm {
            rsvp_id: i64,

            first_name: String,
            last_name: String,
            email: String,
            phone: Option<String>,
        }

        let form: Vec<AttendeeForm> = serde_json::from_str(&form.attendees).map_err(|_| invalid())?;

        for new in form {
            // Can't edit random rsvps
            let old = rsvps.iter().find(|old| old.rsvp_id == new.rsvp_id).ok_or_else(invalid)?;

            // Must exist for a valid RSVP
            let old_email = old.email.as_ref().unwrap();
            let new_email = &new.email;

            // If changing user, confirm they haven't already RSVPed
            if new_email != old_email
                && let Some(status) = Rsvp::lookup_conflicts(&state.db, &session, &event, new_email).await?
            {
                let message = match status.as_str() {
                    RsvpSession::DRAFT => {
                        format!("Someone is in the process of RSVPing for {new_email}.")
                    }
                    _ => format!("Someone has already RSVPed for {new_email}."),
                };
                return Ok(ErrorHtml { user, message }.into_response());
            }

            // Get or create the new user. This could be:
            // * A newly created user with the new email
            // * An existing user with the new email
            // * The old user with the same email
            let user = User::get_or_create(
                &state.db,
                &CreateUser {
                    email: new.email.clone(),
                    first_name: Some(new.first_name.clone()),
                    last_name: Some(new.last_name.clone()),
                    phone: new.phone.clone(),
                },
            )
            .await?;

            // In either case, if any of the info has changed, update this user
            if user.first_name != Some(new.first_name.clone())
                || user.last_name != Some(new.last_name.clone())
                || user.phone != new.phone.clone()
            {
                user.update(
                    &state.db,
                    &UpdateUser {
                        email: Some(new.email.clone()),
                        first_name: Some(new.first_name),
                        last_name: Some(new.last_name),
                        phone: new.phone,
                    },
                )
                .await?;
            }

            // Update the rsvp user and version
            Rsvp::set_user(&state.db, old.rsvp_id, &user).await?;
        }

        goto_manage_page(&session, &event)
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
        stats: &EventStats, spots: &[Spot], selection: &str,
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
        rsvp: &'a AttendeeRsvp,
        info: CreateUser,
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
    }
    async fn parse_attendees<'a>(
        rsvps: &'a [AttendeeRsvp], attendees: &str,
    ) -> Result<(ParsedAttendee<'a>, Vec<ParsedAttendee<'a>>), ParseAttendeesError> {
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

        let form: Vec<AttendeeForm> = serde_json::from_str(attendees).map_err(|_| Error::Parse)?;

        let mut attendees = vec![];
        let mut is_me_attendee = None;

        for rsvp in rsvps {
            // Ensure all RSVPs have an attendee specified
            if !form.iter().any(|a| a.rsvp_id == rsvp.rsvp_id) {
                return Err(Error::MissingAttendee { rsvp_id: rsvp.rsvp_id });
            }
        }

        for AttendeeForm { rsvp_id, first_name, last_name, email, phone, is_me } in form {
            // Ensure all attendees correspond to a valid RSVP
            let Some(rsvp) = rsvps.iter().find(|r| r.rsvp_id == rsvp_id) else {
                return Err(Error::UnknownRsvp { rsvp_id });
            };

            let attendee = ParsedAttendee {
                rsvp,
                info: CreateUser { first_name: Some(first_name), last_name: Some(last_name), email, phone },
            };

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

pub fn add_middleware(router: AxumRouter, state: SharedAppState) -> AxumRouter {
    /// Middleware layer to lookup add an `RsvpSession` to the request if an rsvp_session token is present.
    pub async fn rsvp_session_middleware(
        State(state): State<SharedAppState>, mut cookies: CookieJar, mut request: Request, next: Next,
    ) -> HtmlResult {
        if let Some(token) = cookies.get("rsvp_session") {
            match RsvpSession::lookup_by_token(&state.db, token.value()).await? {
                Some(session) => {
                    request.extensions_mut().insert(session);
                }
                None => cookies = cookies.remove("rsvp_session"),
            }
        }
        let response = next.run(request).await;
        Ok((cookies, response).into_response())
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
                RsvpSession::DRAFT => Ok(session),
                RsvpSession::AWAITING_PAYMENT | RsvpSession::CONFIRMED => {
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

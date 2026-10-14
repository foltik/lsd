use image::DynamicImage;

use crate::db::event::{Event, EventLimits, EventWithStats, UpdateEvent};
use crate::db::event_flyer::*;
use crate::db::rsvp_session::*;
use crate::db::spot::*;
use crate::prelude::*;
use crate::utils::calendar;

/// Add all `events` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/e/{slug}", get(read::view_page))
                .route("/e/{slug}/flyer", get(read::flyer_by_slug))
                .route("/e/{slug}/event.ics", get(read::calendar_ics))
                .route("/e/{slug}/stats", get(read::stats_page))
                .route("/e/{slug}/rsvp", get(rsvp::rsvp_form))
                .route("/e/{slug}/rsvp/guestlist", get(rsvp::guestlist_page).post(rsvp::guestlist_form))
                .route("/e/{slug}/rsvp/selection", get(rsvp::selection_page).post(rsvp::selection_form))
                .route("/e/{slug}/rsvp/attendees", get(rsvp::attendees_page).post(rsvp::attendees_form))
                .route("/e/{slug}/rsvp/contribution", get(rsvp::contribution_page).post(rsvp::contribution_form))
                .route("/e/{slug}/rsvp/code", post(rsvp::coupon_apply_form))
                .route("/e/{slug}/rsvp/code/check", post(rsvp::coupon_check_api))
                .route("/e/{slug}/rsvp/code/remove", post(rsvp::coupon_remove_form))
                .route("/e/{slug}/rsvp/manage", get(rsvp::manage_page)) // REMOVEME .post(rsvp::temp_delete))
                .route("/e/{slug}/rsvp/add-guests", post(rsvp::add_guests_form))
                .route("/e/{slug}/rsvp/edit", get(rsvp::edit_guests_page).post(rsvp::edit_guests_form))
                .route("/e/{slug}/{link_slug}", get(rsvp::link_page).post(rsvp::link_form))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/events", get(read::list_page))
                .route("/events/all", get(read::list_all_page))
                .route("/events/sessions", get(read::sessions_page))
                .route("/events/sessions/{id}", delete(read::delete_session))
                .route("/events/new", get(edit::new_page))
                .route("/events/{id}/edit", get(edit::edit_page).post(edit::edit_form))
                .route("/events/{id}/delete", post(edit::delete_form))
                .route("/events/{id}/duplicate", post(edit::duplicate_form))
                .route("/events/{id}/flyer", get(read::flyer_by_id))
                .route("/events/{id}/attendees", get(edit::attendees_page))
                .route("/events/{id}/attendees/add", get(edit::add_attendee_page).post(edit::add_attendee_form))
                .route("/events/{id}/attendees/{rsvp_id}", delete(edit::soft_delete_attendee))
                .route("/events/{id}/attendees/{rsvp_id}/refund", post(edit::refund_attendee))
                .route("/events/{id}/attendees/{rsvp_id}/checkin", post(edit::set_checkin).delete(edit::clear_checkin))
                .route("/events/{id}/attendees/{rsvp_id}/edit", get(edit::edit_attendee_page).post(edit::edit_attendee_form))
                .route("/events/{id}/attendees/manual/{manual_id}", delete(edit::delete_manual_attendee))
                .route("/events/{id}/attendees/manual/{manual_id}/checkin", post(edit::set_manual_checkin).delete(edit::clear_manual_checkin))
                .route("/events/{id}/attendees/manual/{manual_id}/edit", get(edit::edit_manual_attendee_page).post(edit::edit_manual_attendee_form))
                .route("/events/{id}/invite/edit", get(edit::edit_invite_page).post(edit::edit_invite_form))
                .route("/events/{id}/invite/preview", get(edit::preview_invite_page))
                .route("/events/{id}/invite/send", get(edit::send_invite_page).post(edit::send_invite_form))
                .route("/events/{id}/confirmation/edit", get(edit::edit_confirmation_page).post(edit::edit_confirmation_form))
                .route("/events/{id}/confirmation/preview", get(edit::preview_confirmation_page))
                .route("/events/{id}/dayof/edit", get(edit::edit_dayof_page).post(edit::edit_dayof_form))
                .route("/events/{id}/dayof/preview", get(edit::preview_dayof_page))
                .route("/events/{id}/dayof/send", get(edit::send_dayof_page).post(edit::send_dayof_form))
                .route("/events/{id}/description/edit", get(edit::edit_description_page).post(edit::edit_description_form))
                .route("/events/{id}/links", get(links::list_page))
                .route("/events/{id}/links/new", get(links::new_page))
                .route("/events/{id}/links/{link_id}", get(links::edit_page).post(links::edit_form))
                .route("/events/{id}/links/{link_id}/delete", post(links::delete_form))
        })
}

// View and list events.
mod read {
    use super::*;
    use crate::db::manual_rsvp::ManualRsvp;
    use crate::db::rsvp::Rsvp;
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
            full: bool,
            open: bool,
        }

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        // Private events redirect straight to the RSVP process
        if event.guest_list_id.is_some() {
            return Ok(Redirect::to(&format!("/e/{slug}/rsvp")).into_response());
        }

        let flyer = EventFlyer::lookup(&state.db, event.id).await?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[]).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        // No need to count user limits, they'd see the Manage page link anyways
        let limits = event.compute_limits(&spots, &held_rsvps, None, manual_count, None);
        let full = limits.total_limit == 0;
        let open = event.registration_open(None);
        Ok(Html { session, user, event, flyer, full, open }.into_response())
    }

    // List all events.
    #[derive(Template, WebTemplate)]
    #[template(path = "events/list.html")]
    struct ListHtml {
        user: Option<User>,
        events: Vec<EventWithStats>,
        all: bool,
    }

    pub async fn list_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
        let events = Event::list_with_stats(&state.db, true).await?;
        Ok(ListHtml { user, events, all: false }.into_response())
    }

    pub async fn list_all_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
        let events = Event::list_with_stats(&state.db, false).await?;
        Ok(ListHtml { user, events, all: true }.into_response())
    }

    /// Serve an event flyer.
    pub async fn flyer_by_id(
        State(state): State<SharedAppState>, Path(id): Path<i64>,
        Query(params): Query<std::collections::HashMap<String, String>>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!();
        };
        EventFlyer::serve(&state.db, &event.slug, params.get("size")).await
    }
    pub async fn flyer_by_slug(
        State(state): State<SharedAppState>, Path(slug): Path<String>,
        Query(params): Query<std::collections::HashMap<String, String>>,
    ) -> HtmlResult {
        EventFlyer::serve(&state.db, &slug, params.get("size")).await
    }

    /// Serve an event as an iCalendar file, for Apple Calendar, Outlook desktop, and anything else
    /// without a deep link scheme.
    pub async fn calendar_ics(State(state): State<SharedAppState>, Path(slug): Path<String>) -> HtmlResult {
        let Some(event) = Event::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
        };

        let ics = calendar::CalendarEvent::from_event(&event).ics();
        Ok((
            [
                (header::CONTENT_TYPE, "text/calendar; charset=utf-8".to_string()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{slug}.ics\"")),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            ics,
        )
            .into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct StatsQuery {
        secret: String,
    }

    /// Semi-public stats page for event organizers, gated by the event token.
    pub async fn stats_page(
        user: Option<User>, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Query(query): Query<StatsQuery>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if query.secret != event.token {
            bail_not_found!();
        }

        let mut spots = Spot::stats_for_event(&state.db, event.id).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;

        // Display only: a spot can't offer more than the overall event capacity
        for spot in &mut spots {
            spot.qty_total = spot.qty_total.min(event.capacity);
        }

        let rsvp_count: i64 = spots.iter().map(|s| s.rsvp_count).sum();
        let attendees = rsvp_count + manual_count;
        let remaining = (event.capacity - attendees).max(0);
        let capacity_pct = match event.capacity {
            0 => 0,
            capacity => (attendees * 100 / capacity).min(100),
        };

        let total_contributions: i64 = spots.iter().map(|s| s.contributions).sum();
        let artist_total = event.artist_share(total_contributions);
        let studio_total = total_contributions - artist_total;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/stats.html")]
        struct StatsHtml {
            user: Option<User>,
            event: Event,
            spots: Vec<SpotStats>,
            manual_count: i64,
            attendees: i64,
            remaining: i64,
            capacity_pct: i64,
            total_contributions: i64,
            artist_total: i64,
            studio_total: i64,
        }
        Ok(StatsHtml {
            user,
            event,
            spots,
            manual_count,
            attendees,
            remaining,
            capacity_pct,
            total_contributions,
            artist_total,
            studio_total,
        }
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
        if matches!(
            session.status.as_str(),
            RsvpSession::PAYMENT_PENDING
                | RsvpSession::PAYMENT_CONFIRMED
                | RsvpSession::REFUND_PENDING
                | RsvpSession::REFUND_CONFIRMED
        ) {
            bail_invalid!();
        }
        session.delete(&state.db, &state.stripe).await?;
        Ok(Json(()))
    }
}

// Create and edit events.
mod edit {
    use axum::body::Body;

    use super::*;
    use crate::db::list::{List, ListWithCount};
    use crate::db::manual_rsvp::ManualRsvp;
    use crate::db::rsvp::{AdminAttendeesRsvp, Rsvp};
    use crate::db::user::{CreateUser, UpdateUser};
    use crate::utils::editor::{Editor, EditorContent};

    #[derive(Template, WebTemplate)]
    #[template(path = "events/edit.html")]
    struct EditHtml {
        user: Option<User>,
        event: Event,
        spots: Vec<Spot>,
        rsvp_counts: std::collections::HashMap<i64, SpotCounts>,
        flyer_version: Option<i64>,
        lists: Vec<ListWithCount>,
    }

    /// Display the form to create a new event.
    pub async fn new_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml {
            user: Some(user),
            event: Event {
                id: 0,
                token: "".into(),
                kind: Event::INTERNAL.into(),
                title: "".into(),
                slug: "".into(),
                url: None,
                start: Utc::now().naive_utc(),
                end: None,
                capacity: 0,
                unlisted: false,
                closed: false,
                guest_list_id: None,
                spots_per_person: None,
                artist_share: 40,

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
            flyer_version: None,
            lists,
        }
        .into_response())
    }

    /// Display the form to edit an event.
    pub async fn edit_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let rsvp_counts = Spot::rsvp_counts_for_event(&state.db, event.id).await?;
        let flyer_version = EventFlyer::version_for_event(&state.db, event.id).await?;
        let lists = List::list_with_counts(&state.db).await?;
        Ok(EditHtml { user: Some(user), event, spots, rsvp_counts, flyer_version, lists }.into_response())
    }

    // Handle edit submission.
    #[derive(Debug, serde::Deserialize)]
    pub struct EditForm {
        id: i64,
        #[serde(flatten)]
        event: UpdateEvent,
        spots: Vec<UpdateSpot>,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>, mut multipart: axum::extract::Multipart,
    ) -> JsonResult<()> {
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
                    let img = crate::utils::image::decode(&data)
                        .await
                        .map_err(|_| bad_request("Unsupported or invalid image format."))?;
                    flyer = Some(img);
                }
                _ => {}
            }
        }

        let mut form = form.ok_or_else(invalid)?;

        // Kind is chosen at creation and immutable afterwards; ignore the submitted kind on update.
        if form.id != 0 {
            let event = Event::lookup_by_id(&state.db, form.id).await?.ok_or_else(not_found)?;
            form.event.kind = event.kind;
        }

        // Validate slug: must be non-empty and only contain alphanumeric characters and dashes
        if form.event.slug.is_empty()
            || !form.event.slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            bail_bad_request!("Slug can only contain letters, numbers, and dashes.");
        }

        match form.event.kind.as_str() {
            Event::INTERNAL => {
                // Internal events have no url.
                form.event.url = None;
            }
            Event::EXTERNAL => {
                // External events are links elsewhere: a url and no RSVP machinery.
                if url::Url::parse(form.event.url.as_deref().unwrap_or("")).is_err() {
                    bail_bad_request!("Invalid url.");
                }
                form.event.capacity = 0;
                form.event.unlisted = false;
                form.event.closed = false;
                form.event.guest_list_id = None;
                form.event.spots_per_person = None;
                form.event.artist_share = 0;
                form.spots = vec![];
            }
            _ => bail_invalid!(),
        }

        match form.id {
            0 => {
                tracing::info!("create: {:?}", &form.event);
                let event_id = Event::create(&state.db, &form.event, &flyer).await?;

                let mut spot_ids = vec![];
                for spot in form.spots {
                    let id = Spot::create(&state.db, &spot).await?;
                    spot_ids.push(id);
                }

                Spot::add_to_event(&state.db, event_id, spot_ids).await?;
            }
            id => {
                tracing::info!("edit: {:?}", &form.event);
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

                // Only delete spots with no confirmed RSVPs (cart items don't block deletion)
                to_delete
                    .retain(|&spot_id| rsvp_counts.get(&spot_id).map(|c| c.rsvp_count).unwrap_or(0) == 0);

                Spot::add_to_event(&state.db, id, to_add).await?;
                Spot::remove_from_event(&state.db, id, to_delete).await?;
            }
        }

        Ok(Json(()))
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
        email_token: String,
        email: String,
        event: Event,
        flyer: Option<EventFlyer>,
    }
    // Preview invite page.
    pub async fn preview_invite_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        Ok(InviteEmailHtml { email_token: String::new(), email: user.email, event, flyer }.into_response())
    }

    /// Display the form to send invites.
    #[derive(serde::Deserialize)]
    pub struct SendInviteQuery {
        list_id: Option<i64>,
    }
    pub async fn send_invite_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
        Query(query): Query<SendInviteQuery>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };
        let list_id = event.guest_list_id.or(query.list_id);

        #[derive(sqlx::FromRow)]
        struct ListCounts {
            name: String,
            count: i64,
            sent: i64,
        }
        let list = match list_id {
            None => None,
            Some(list_id) => Some(
                sqlx::query_as!(
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
                                  AND e.error IS NULL
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
                    list_id,
                )
                .fetch_one(&state.db)
                .await?,
            ),
        };
        let lists = match event.guest_list_id {
            None => List::list_with_counts(&state.db).await?,
            Some(_) => vec![],
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/send_invites.html")]
        struct SendHtml {
            user: Option<User>,
            list_id: Option<i64>,
            list: Option<ListCounts>,
            lists: Vec<ListWithCount>,
            event: Event,
            ratelimit: usize,
        }
        let ratelimit = state.config.email.ratelimit;
        Ok(SendHtml { user: Some(user), event, list_id, list, lists, ratelimit }.into_response())
    }

    pub async fn send_invite_form(
        State(state): State<SharedAppState>, Path(id): Path<i64>, Query(query): Query<SendInviteQuery>,
    ) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!();
        };
        let Some(list_id) = event.guest_list_id.or(query.list_id) else {
            bail_invalid!()
        };

        let emails = Email::create_send_invites(&state.db, event.id, list_id).await?;
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        let mut email_template =
            InviteEmailHtml { email_token: String::new(), email: "".into(), event: event.clone(), flyer };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, token, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.email_token = token;
            email_template.email = address.clone();

            let from = &state.config.email.from;
            let reply_to = config().email.contact_to.as_ref().unwrap_or(from);
            let message = state
                .mailer
                .builder(&email_template.email_token)
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
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "emails/event_confirmation.html")]
        struct PreviewConfirmationHtml {
            email_token: String,
            event: Event,
            token: String,
            flyer: Option<EventFlyer>,
            is_guest: bool,
        }
        Ok(PreviewConfirmationHtml {
            email_token: String::new(),
            event: event.clone(),
            token: "xxxxxxxx".into(),
            flyer,
            is_guest: false,
        }
        .into_response())
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
        email_token: String,
        event: Event,
        flyer: Option<EventFlyer>,
    }
    // Preview dayof page.
    pub async fn preview_dayof_page(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let Some(event) = Event::lookup_by_id(&state.db, id).await? else {
            bail_not_found!()
        };
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        Ok(DayofEmailHtml { email_token: String::new(), event: event.clone(), flyer }.into_response())
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
                COUNT(*) AS count,
                COALESCE(SUM(
                    CASE WHEN EXISTS (
                        SELECT 1
                        FROM emails e
                        WHERE e.kind = ?
                          AND e.user_id = a.user_id
                          AND e.event_id = ?
                          AND e.sent_at IS NOT NULL
                    )
                    THEN 1 ELSE 0 END
                ), 0) AS sent
            FROM (
                SELECT r.user_id
                FROM rsvps r
                JOIN rsvp_sessions rs ON rs.id = r.session_id
                WHERE rs.event_id = ?
                  AND rs.status IN ('payment_pending', 'payment_confirmed', 'refund_pending', 'refund_confirmed')
                UNION
                SELECT m.user_id
                FROM manual_rsvps m
                WHERE m.event_id = ? AND m.user_id IS NOT NULL
            ) a
            "#,
            Email::EVENT_DAYOF,
            event.id,
            event.id,
            event.id,
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
        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, token, .. } in emails {
            if sent_at.is_some() {
                continue;
            }
            messages.push(email::format_dayof_email(&state, &event, &flyer, &address, &token)?);
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

    #[derive(serde::Deserialize)]
    pub struct AttendeesQuery {
        link: Option<String>,
    }

    /// View an event.
    pub async fn attendees_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
        Query(query): Query<AttendeesQuery>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        let mut rsvps = Rsvp::list_for_admin_attendees(&state.db, event.id).await?;
        if let Some(link) = &query.link {
            rsvps.retain(|r| r.link_slug.as_ref() == Some(link));
        }

        #[derive(Template, WebTemplate)]
        #[template(path = "events/attendees.html")]
        struct Html {
            pub user: Option<User>,
            event: Event,
            rsvp_count: usize,
            refund_count: usize,
            total_contributions: i64,
            show_notes: bool,
            show_links: bool,
            link: Option<String>,
            rsvps: Vec<AdminAttendeesRsvp>,
        }

        let rsvp_count = rsvps.iter().filter(|r| !r.is_deleted).count();
        let refund_count = rsvps.iter().filter(|r| r.is_refunded()).count();
        let total_contributions = rsvps.iter().filter(|r| !r.is_refunded()).map(|r| r.contribution).sum();
        let show_notes = rsvps.iter().any(|r| r.note.is_some());
        let show_links = rsvps.iter().any(|r| r.link_slug.is_some());
        Ok(Html {
            user: Some(user),
            event,
            rsvp_count,
            refund_count,
            total_contributions,
            show_notes,
            show_links,
            link: query.link,
            rsvps,
        }
        .into_response())
    }

    /// Handle delete submission.
    pub async fn delete_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        Event::delete(&state.db, event.id, &event.slug).await?;
        Ok(Redirect::to("/events").into_response())
    }

    /// Handle duplicate submission.
    pub async fn duplicate_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        let new_id = Event::duplicate(&state.db, event.id).await?;
        Ok(Redirect::to(&format!("/events/{new_id}/edit")).into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct AttendeePath {
        id: i64,
        rsvp_id: i64,
    }

    pub async fn set_checkin(
        State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
    ) -> JsonResult<()> {
        Rsvp::set_checkin_at(&state.db, path.rsvp_id).await?;
        Ok(Json(()))
    }

    pub async fn clear_checkin(
        State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
    ) -> JsonResult<()> {
        Rsvp::clear_checkin_at(&state.db, path.rsvp_id).await?;
        Ok(Json(()))
    }

    pub async fn soft_delete_attendee(
        State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
    ) -> JsonResult<()> {
        Rsvp::soft_delete(&state.db, path.rsvp_id).await?;
        Ok(Json(()))
    }

    #[derive(serde::Deserialize)]
    pub struct ManualAttendeePath {
        id: i64,
        manual_id: i64,
    }

    pub async fn set_manual_checkin(
        State(state): State<SharedAppState>, Path(path): Path<ManualAttendeePath>,
    ) -> JsonResult<()> {
        ManualRsvp::set_checkin_at(&state.db, path.manual_id).await?;
        Ok(Json(()))
    }

    pub async fn clear_manual_checkin(
        State(state): State<SharedAppState>, Path(path): Path<ManualAttendeePath>,
    ) -> JsonResult<()> {
        ManualRsvp::clear_checkin_at(&state.db, path.manual_id).await?;
        Ok(Json(()))
    }

    pub async fn delete_manual_attendee(
        State(state): State<SharedAppState>, Path(path): Path<ManualAttendeePath>,
    ) -> JsonResult<()> {
        ManualRsvp::delete(&state.db, path.manual_id).await?;
        Ok(Json(()))
    }

    pub async fn refund_attendee(
        State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
    ) -> JsonResult<()> {
        let rsvp = Rsvp::lookup_by_id(&state.db, path.rsvp_id).await?.ok_or_else(not_found)?;
        let session = RsvpSession::lookup_by_id(&state.db, rsvp.session_id).await?.unwrap();

        // Refund all the owner's RSVP sessions, including from "Add Guests" flows.
        let all_sessions =
            RsvpSession::list_for_user(&state.db, session.user_id.unwrap(), session.event_id).await?;
        let paid_sessions = all_sessions
            .iter()
            .filter(|s| s.status == RsvpSession::PAYMENT_CONFIRMED && s.stripe_payment_intent_id.is_some())
            .collect::<Vec<_>>();
        if paid_sessions.is_empty() {
            bail_invalid!();
        }

        for session in paid_sessions {
            let refund_id = state.stripe.refund(session.stripe_payment_intent_id.as_ref().unwrap()).await?;
            session.set_refund_id(&state.db, &refund_id).await?;
            session.set_status(&state.db, RsvpSession::REFUND_PENDING).await?;
        }
        Ok(Json(()))
    }

    struct AttendeeForm {
        id: i64,
        first_name: String,
        last_name: String,
        email: Option<String>,
        note: String,
        is_manual: bool,
    }

    /// Display the form to add a manual attendee.
    pub async fn add_attendee_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/attendees_add.html")]
        struct Html {
            user: Option<User>,
            event: Event,
            attendee: Option<AttendeeForm>,
        }
        Ok(Html { user: Some(user), event, attendee: None }.into_response())
    }

    /// Display the form to edit an existing attendee.
    pub async fn edit_attendee_page(
        user: User, State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, path.id).await?.ok_or_else(not_found)?;
        let data = Rsvp::lookup_for_edit(&state.db, path.rsvp_id).await?.ok_or_else(not_found)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/attendees_add.html")]
        struct Html {
            user: Option<User>,
            event: Event,
            attendee: Option<AttendeeForm>,
        }
        Ok(Html {
            user: Some(user),
            event,
            attendee: Some(AttendeeForm {
                id: path.rsvp_id,
                first_name: data.first_name.unwrap_or_default(),
                last_name: data.last_name.unwrap_or_default(),
                email: data.email,
                note: data.note.unwrap_or_default(),
                is_manual: false,
            }),
        }
        .into_response())
    }

    /// Display the form to edit an existing manual attendee.
    pub async fn edit_manual_attendee_page(
        user: User, State(state): State<SharedAppState>, Path(path): Path<ManualAttendeePath>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, path.id).await?.ok_or_else(not_found)?;
        let data = ManualRsvp::lookup_for_edit(&state.db, path.manual_id)
            .await?
            .ok_or_else(not_found)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/attendees_add.html")]
        struct Html {
            user: Option<User>,
            event: Event,
            attendee: Option<AttendeeForm>,
        }
        Ok(Html {
            user: Some(user),
            event,
            attendee: Some(AttendeeForm {
                id: path.manual_id,
                first_name: data.first_name.unwrap_or_default(),
                last_name: data.last_name.unwrap_or_default(),
                email: data.email,
                note: data.note.unwrap_or_default(),
                is_manual: true,
            }),
        }
        .into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct AddAttendeeForm {
        first_name: String,
        last_name: String,
        email: String,
        note: Option<String>,
    }

    /// Handle add attendee form submission.
    pub async fn add_attendee_form(
        admin: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
        Form(form): Form<AddAttendeeForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        let note = form.note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());

        if form.email.is_empty() {
            ManualRsvp::create(
                &state.db,
                event.id,
                None,
                Some(&form.first_name),
                Some(&form.last_name),
                admin.id,
                note.as_deref(),
            )
            .await?;
            return Ok(Redirect::to(&format!("/events/{id}/attendees")).into_response());
        }

        // Check if already has an RSVP (manual or regular) for this event
        if let Some(existing) = User::lookup_by_email(&state.db, &form.email).await?
            && (ManualRsvp::user_committed_for_event(&state.db, event.id, existing.id).await?
                || Rsvp::user_committed_for_event(&state.db, event.id, existing.id).await?)
        {
            return Ok(Redirect::to(&format!("/events/{id}/attendees")).into_response());
        }

        let user = User::update_or_create(
            &state.db,
            &CreateUser {
                first_name: Some(form.first_name),
                last_name: Some(form.last_name),
                email: form.email,
                phone: None,
            },
        )
        .await?;

        // Create manual RSVP
        ManualRsvp::create(&state.db, event.id, Some(user.id), None, None, admin.id, note.as_deref()).await?;

        Ok(Redirect::to(&format!("/events/{id}/attendees")).into_response())
    }

    #[derive(serde::Deserialize)]
    pub struct EditAttendeeForm {
        first_name: String,
        last_name: String,
        note: Option<String>,
    }

    /// Handle edit attendee form submission.
    pub async fn edit_attendee_form(
        _admin: User, State(state): State<SharedAppState>, Path(path): Path<AttendeePath>,
        Form(form): Form<EditAttendeeForm>,
    ) -> HtmlResult {
        let rsvp = Rsvp::lookup_by_id(&state.db, path.rsvp_id).await?.ok_or_else(not_found)?;
        let attendee = User::lookup_by_id(&state.db, rsvp.user_id.unwrap())
            .await?
            .ok_or_else(not_found)?;

        // Email is read-only, so reuse the existing email and phone.
        attendee
            .update(
                &state.db,
                &UpdateUser {
                    email: attendee.email.clone(),
                    first_name: Some(form.first_name),
                    last_name: Some(form.last_name),
                    phone: attendee.phone.clone(),
                },
            )
            .await?;

        let note = form.note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        Rsvp::update_note(&state.db, rsvp.id, note.as_deref()).await?;

        Ok(Redirect::to(&format!("/events/{}/attendees", path.id)).into_response())
    }

    /// Handle edit manual attendee form submission.
    pub async fn edit_manual_attendee_form(
        _admin: User, State(state): State<SharedAppState>, Path(path): Path<ManualAttendeePath>,
        Form(form): Form<EditAttendeeForm>,
    ) -> HtmlResult {
        let manual = ManualRsvp::lookup_by_id(&state.db, path.manual_id)
            .await?
            .ok_or_else(not_found)?;
        let note = form.note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        ManualRsvp::update_note(&state.db, manual.id, note.as_deref()).await?;

        let Some(user_id) = manual.user_id else {
            ManualRsvp::update_name(&state.db, manual.id, &form.first_name, &form.last_name).await?;
            return Ok(Redirect::to(&format!("/events/{}/attendees", path.id)).into_response());
        };

        let attendee = User::lookup_by_id(&state.db, user_id).await?.ok_or_else(not_found)?;

        // Email is read-only, so reuse the existing email and phone.
        attendee
            .update(
                &state.db,
                &UpdateUser {
                    email: attendee.email.clone(),
                    first_name: Some(form.first_name),
                    last_name: Some(form.last_name),
                    phone: attendee.phone.clone(),
                },
            )
            .await?;

        Ok(Redirect::to(&format!("/events/{}/attendees", path.id)).into_response())
    }
}

// Manage event links.
mod links {
    use super::*;
    use crate::db::event_link::{EventLink, EventLinkWithUses, UpdateEventLink};

    #[derive(serde::Deserialize)]
    pub struct LinkPath {
        id: i64,
        link_id: i64,
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "events/links_edit.html")]
    struct EditHtml {
        user: Option<User>,
        event: Event,
        link: EventLink,
    }

    // Display an event's links.
    pub async fn list_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        let links = EventLink::list_for_event(&state.db, event.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/links.html")]
        struct Html {
            user: Option<User>,
            event: Event,
            links: Vec<EventLinkWithUses>,
        }
        Ok(Html { user: Some(user), event, links }.into_response())
    }

    // Display the form to create a new link.
    pub async fn new_page(
        user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
        Ok(EditHtml {
            user: Some(user),
            link: EventLink {
                id: 0,
                event_id: event.id,
                slug: "".into(),
                password: None,
                capacity: 0,
                bypass_capacity: false,
                bypass_closed: false,
                spots_per_person: None,
                closed: false,
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            event,
        }
        .into_response())
    }

    // Display the form to edit a link.
    pub async fn edit_page(
        user: User, State(state): State<SharedAppState>, Path(path): Path<LinkPath>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, path.id).await?.ok_or_else(not_found)?;
        let link = EventLink::lookup_by_id(&state.db, event.id, path.link_id)
            .await?
            .ok_or_else(not_found)?;
        Ok(EditHtml { user: Some(user), event, link }.into_response())
    }

    // Handle create/edit submission.
    pub async fn edit_form(
        State(state): State<SharedAppState>, Path(path): Path<LinkPath>,
        Form(mut form): Form<UpdateEventLink>,
    ) -> HtmlResult {
        let event = Event::lookup_by_id(&state.db, path.id).await?.ok_or_else(not_found)?;

        // Slug must be non-empty and alphanumeric with dashes
        if form.slug.is_empty() || !form.slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            bail_bad_request!("Slug can only contain letters, numbers, and dashes.");
        }
        let slug = &form.slug;

        // Slug must be unique
        if let Some(existing) = EventLink::lookup_by_slug(&state.db, event.id, slug).await?
            && existing.id != path.link_id
        {
            bail_bad_request!("Link '{slug}' already exists.");
        }

        // Trim password
        form.password = form.password.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());

        match path.link_id {
            0 => {
                tracing::info!("create event_id={} link={form:?}", event.id);
                EventLink::create(&state.db, event.id, &form).await?;
            }
            link_id => {
                tracing::info!("edit event_id={} link_id={link_id} link={form:?}", event.id);
                EventLink::lookup_by_id(&state.db, event.id, link_id)
                    .await?
                    .ok_or_else(not_found)?;
                EventLink::update(&state.db, link_id, &form).await?;
            }
        }

        Ok(Redirect::to(&format!("/events/{}/links", event.id)).into_response())
    }

    // Delete a link.
    pub async fn delete_form(State(state): State<SharedAppState>, Path(path): Path<LinkPath>) -> HtmlResult {
        EventLink::lookup_by_id(&state.db, path.id, path.link_id)
            .await?
            .ok_or_else(not_found)?;
        EventLink::delete(&state.db, path.link_id).await?;
        Ok(Redirect::to(&format!("/events/{}/links", path.id)).into_response())
    }
}

mod rsvp {
    use std::cmp::Reverse;
    use std::collections::HashSet;

    use super::*;
    use crate::app::events::rsvp::parse::ParsedAttendee;
    use crate::db::coupon::{Coupon, CouponCheck};
    use crate::db::event_link::EventLink;
    use crate::db::list::List;
    use crate::db::manual_rsvp::ManualRsvp;
    use crate::db::rsvp::{AttendeeRsvp, ContributionRsvp, CreateRsvp, EventRsvp, Rsvp};
    use crate::db::rsvp_session::RsvpSession;
    use crate::db::user::CreateUser;
    use crate::utils::stripe;

    #[derive(Template, WebTemplate)]
    #[template(path = "error_simple.html")]
    struct ErrorHtml {
        user: Option<User>,
        message: String,
    }

    /// Renders message as raw HTML, so never interpolate user input into it.
    #[derive(Template, WebTemplate)]
    #[template(path = "error_simple_safe.html")]
    struct ErrorHtmlSafe {
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

    // Check if clicking the RSVP button (or a custom link) should resume an existing session instead of replacing it.
    fn should_resume_session(link: Option<&EventLink>, session: &RsvpSession) -> bool {
        // Always resume Add Guests sessions, since the parent session must already be confirmed.
        if session.is_adding_guests() {
            return true;
        }
        // Resume any other reserved (in CONTRIBUTION) or confirmed sessions.
        if !session.is_draft() {
            return true;
        }
        // Otherwise, resume any session already started from this link or the RSVP button.
        session.link_id == link.map(|link| link.id)
    }

    /// Create an RSVP session after a user clicks the RSVP button for an event.
    pub async fn rsvp_form(
        session: Option<RsvpSession>, user: Option<User>, State(state): State<SharedAppState>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        // If already in a session, check if we should resume it instead of starting a new one.
        if let Some(session) = session.clone()
            && should_resume_session(None, &session)
        {
            return goto::by_status(&state.db, &event, session).await;
        }

        if !event.registration_open(None) {
            return goto::error_registration_closed(&state.db, &state.stripe, &None).await;
        }

        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[]).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        // No need to count user limits, they'd be redirected anyways
        let limits = event.compute_limits(&spots, &held_rsvps, None, manual_count, None);
        if limits.total_limit == 0 {
            return goto::error_at_capacity(&state.db, &state.stripe, &None).await;
        }

        // Delete any existing session.
        if let Some(session) = session {
            session.delete(&state.db, &state.stripe).await?;
        }

        match event.guest_list_id {
            None => goto::selection_page(&state.db, &user, &None, &event).await,
            Some(_) => goto::guestlist_page(&event),
        }
    }

    // Display the guestlist confirmation page
    pub async fn guestlist_page(
        session: Option<RsvpSession>, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        // If a session exists, the user must have navigated here directly, send them back.
        if let Some(session) = session {
            return goto::by_status(&state.db, &event, session).await;
        }

        let _guest_list_id = event.guest_list_id.ok_or_else(invalid)?;
        if !event.registration_open(None) {
            return goto::error_registration_closed(&state.db, &state.stripe, &None).await;
        }

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_guestlist.html")]
        struct GuestlistHtml {
            user: Option<User>,
            event: Event,
        }
        Ok(GuestlistHtml { user: None, event }.into_response())
    }

    // Handle submission of the guestlist confirmation form
    #[derive(Debug, serde::Deserialize)]
    pub struct GuestlistForm {
        email: String,
    }
    pub async fn guestlist_form(
        session: Option<RsvpSession>, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<GuestlistForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;

        // If a session exists, the user must have navigated here directly, send them back.
        if let Some(session) = session {
            return goto::by_status(&state.db, &event, session).await;
        }

        let guest_list_id = event.guest_list_id.ok_or_else(invalid)?;
        if !event.registration_open(None) {
            return goto::error_registration_closed(&state.db, &state.stripe, &None).await;
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
                tracing::info!(
                    "Guestlist check passed with event_id={} user_id={} user_email={:?}",
                    event.id,
                    user.id,
                    user.email
                );

                // Check for conflicts, possibly taking over another session.
                let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[]).await?;
                if let Some(conflict) = validate::no_conflicts(&held_rsvps, Some(&primary_user), &[])
                    && let Some(resp) = resolve_conflict(&state, &event, None, conflict).await?
                {
                    return Ok(resp);
                }

                goto::selection_page(&state.db, &Some(user), &None, &event).await
            }
            false => goto::error_not_on_guestlist(),
        }
    }

    // Handle clicking an event link.
    pub async fn link_page(
        session: Option<RsvpSession>, user: Option<User>, State(state): State<SharedAppState>,
        Path((slug, link_slug)): Path<(String, String)>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = EventLink::lookup_by_slug(&state.db, event.id, &link_slug)
            .await?
            .ok_or_else(not_found)?;

        // If already in a session, check if we should resume it instead of starting a new one.
        if let Some(session) = session.clone()
            && should_resume_session(Some(&link), &session)
        {
            return goto::by_status(&state.db, &event, session).await;
        }

        if link.password.is_some() {
            #[derive(Template, WebTemplate)]
            #[template(path = "events/rsvp_link_password.html")]
            struct Html {
                user: Option<User>,
                event: Event,
                link: EventLink,
            }
            Ok(Html { user, event, link }.into_response())
        } else {
            link_create_session(&state, &event, &link, session, &user).await
        }
    }

    // Handle submission of the link password form.
    #[derive(serde::Deserialize)]
    pub struct LinkPasswordForm {
        password: String,
    }
    pub async fn link_form(
        session: Option<RsvpSession>, user: Option<User>, State(state): State<SharedAppState>,
        Path((slug, link_slug)): Path<(String, String)>, Form(form): Form<LinkPasswordForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = EventLink::lookup_by_slug(&state.db, event.id, &link_slug)
            .await?
            .ok_or_else(not_found)?;

        // If already in a session, check if we should resume it instead of starting a new one.
        if let Some(session) = session.clone()
            && should_resume_session(Some(&link), &session)
        {
            return goto::by_status(&state.db, &event, session).await;
        }

        if link.password.as_deref() == Some(form.password.trim()) {
            link_create_session(&state, &event, &link, session, &user).await
        } else {
            goto::error_wrong_password()
        }
    }

    // Create a new session associated with the given link.
    async fn link_create_session(
        state: &SharedAppState, event: &Event, link: &EventLink, session: Option<RsvpSession>,
        user: &Option<User>,
    ) -> HtmlResult {
        // Check event/link registration closure.
        if !event.registration_open(Some(link)) {
            return goto::error_registration_closed(&state.db, &state.stripe, &None).await;
        }

        // Check event/link capacity.
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let held_rsvps = Rsvp::list_held_for_event(&state.db, event, &[]).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        // No need to count user limits, they'd be redirected anyways
        let limits = event.compute_limits(&spots, &held_rsvps, None, manual_count, Some(link));
        if limits.total_limit == 0 {
            return goto::error_at_capacity(&state.db, &state.stripe, &None).await;
        }

        // Replace existing session with a new one.
        if let Some(session) = session {
            session.delete(&state.db, &state.stripe).await?;
        }
        let session = RsvpSession::create(&state.db, event.id, user, Some(link.id)).await?;

        let cookie = session.cookie(&format!("/e/{}", event.slug), Some(link));
        let header = (header::SET_COOKIE, cookie);
        Ok(([header], Redirect::to(&format!("/e/{}/rsvp/selection", event.slug))).into_response())
    }

    // Display the "Choose a contribution" page
    pub async fn selection_page(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;

        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }

        let spots = Spot::list_for_event(&state.db, event.id).await?;

        let our_rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        let mut our_qtys = HashMap::default();
        let mut our_contributions = HashMap::default();
        for rsvp in our_rsvps {
            *our_qtys.entry(rsvp.spot_id).or_default() += 1;
            our_contributions.insert(rsvp.spot_id, rsvp.contribution);
        }

        let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[session.id]).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        let limits = event.compute_limits(&spots, &held_rsvps, session.user_id, manual_count, link.as_ref());
        if limits.total_limit == 0 {
            return goto::error_at_capacity(&state.db, &state.stripe, &Some(session)).await;
        }
        let stats = Spot::stats(&spots, &held_rsvps);

        let parent_token = match session.parent_session_id {
            Some(parent_id) => {
                let parent = RsvpSession::lookup_by_id(&state.db, parent_id).await?.unwrap();
                Some(parent.token)
            }
            None => None,
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_selection.html")]
        struct SelectionHtml {
            event: Event,
            link: Option<EventLink>,
            spots: Vec<Spot>,
            our_qtys: HashMap<i64, usize>,
            our_contributions: HashMap<i64, i64>,
            limits: EventLimits,
            stats: VariableSpotStats,
            parent_token: Option<String>,
        }
        Ok(SelectionHtml {
            event,
            link,
            spots,
            our_qtys,
            our_contributions,
            limits,
            stats,
            parent_token,
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
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }

        let spots = Spot::list_for_event(&state.db, event.id).await?;

        // Parse and validate form
        let our_rsvps = parse::selection_form(&spots, &form.rsvps).map_err(|e| {
            alert!("selection_form(): session={} form={form:?}: {e}", session.token);
            invalid()
        })?;

        // Verify limits
        let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[session.id]).await?;
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        let limits = event.compute_limits(&spots, &held_rsvps, session.user_id, manual_count, link.as_ref());
        if limits.total_limit == 0 {
            return goto::error_at_capacity(&state.db, &state.stripe, &Some(session)).await;
        }
        if !validate::within_limits(&limits, &our_rsvps) {
            return goto::error_spot_taken(&state.db, &state.stripe, &session).await;
        }

        // Delete any old and create new RSVPs.
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
        // Also clear any coupon, which no longer applies since RSVPs have changed.
        if session.coupon_id.is_some() {
            session.set_coupon(&state.db, None).await?;
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
        is_adding_guests: bool,
    }
    // Display the "Who will be attending?" page after submitting spots
    pub async fn attendees_page(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        // CONTRIBUTION may arrive by navigating backwards
        if session.status != RsvpSession::ATTENDEES && session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let user = session.user(&state.db).await?;
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }

        // Preemptively check limits so user isn't rejected after filling out the whole form.
        {
            let spots = Spot::list_for_event(&state.db, event.id).await?;
            let our_rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
            let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[session.id]).await?;
            let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
            let limits =
                event.compute_limits(&spots, &held_rsvps, session.user_id, manual_count, link.as_ref());
            if limits.total_limit == 0 {
                return goto::error_at_capacity(&state.db, &state.stripe, &Some(session)).await;
            }
            if !validate::within_limits(&limits, &our_rsvps) {
                return goto::error_spot_taken(&state.db, &state.stripe, &session).await;
            }
        }

        let is_adding_guests = session.is_adding_guests();
        let attendees = Rsvp::list_for_attendees(&state.db, session.id).await?;
        let price = attendees.iter().map(|r| r.contribution).sum::<i64>();
        let mode = AttendeesMode::Create;
        Ok(AttendeesHtml { mode, event, user, session, attendees, price, is_adding_guests }.into_response())
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
        // CONTRIBUTION may arrive by navigating backwards
        if our_session.status != RsvpSession::ATTENDEES && our_session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let spots = Spot::list_for_event(&state.db, event.id).await?;
        let link = our_session.link(&state.db).await?;

        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(our_session)).await;
        }

        // Parse and validate attendees form
        let session_user = our_session.user(&state.db).await?;
        let our_rsvps = Rsvp::list_for_session(&state.db, our_session.id).await?;
        let is_adding_guests = our_session.is_adding_guests();
        let lock_primary = event.guest_list_id.is_some();
        let (primary_attendee, guest_attendees) =
            parse::attendees_form(&session_user, &our_rsvps, &form.attendees, is_adding_guests, lock_primary)
                .await
                .map_err(|e| {
                    alert!("attendees_form(): session={} form={form:?}: {e}", our_session.token);
                    invalid()
                })?;

        // Check for conflicts, possibly taking over another session.
        let mut held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[our_session.id]).await?;
        let primary_user = primary_attendee.as_ref().map(|a| &a.user);
        if let Some(conflict) = validate::no_conflicts(&held_rsvps, primary_user, &guest_attendees) {
            if let Some(resp) = resolve_conflict(&state, &event, Some(our_session.id), conflict).await? {
                return Ok(resp);
            }
            // Reload held RSVPs since resolving a conflict may have deleted a stale CONTRIBUTION session holding spots.
            held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[our_session.id]).await?;
        }

        // Verify limits in case of preemption since `selection_form()` submission.
        let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
        let limits =
            event.compute_limits(&spots, &held_rsvps, our_session.user_id, manual_count, link.as_ref());
        if limits.total_limit == 0 {
            return goto::error_at_capacity(&state.db, &state.stripe, &Some(our_session)).await;
        }
        if !validate::within_limits(&limits, &our_rsvps) {
            return goto::error_spot_taken(&state.db, &state.stripe, &our_session).await;
        }

        // Create and store users on the primary Rsvp
        if let Some(primary_attendee) = &primary_attendee {
            let primary_user =
                User::upsert_for_rsvp(&state.db, &primary_attendee.user, our_session.created_at).await?;
            Rsvp::set_user(&state.db, primary_attendee.rsvp_id, &primary_user).await?;

            // Store the primary attendee on the RsvpSession.
            // If you're logged in and RSVP for someone else, oh well, they now control the reservation.
            our_session.set_user(&state.db, &primary_user).await?;
        }
        // Create and store users on guest Rsvps
        for ParsedAttendee { rsvp_id, user } in guest_attendees {
            let user = User::upsert_for_rsvp(&state.db, &user, our_session.created_at).await?;
            Rsvp::set_user(&state.db, rsvp_id, &user).await?;
        }

        our_session.set_status(&state.db, RsvpSession::CONTRIBUTION).await?;
        goto::contribution_page(&event)
    }

    // Discount line item for the checkout summary.
    struct DiscountLine {
        label: String,
        amount: i64,
    }
    impl DiscountLine {
        fn build(coupon: &Coupon, rsvps: &[ContributionRsvp]) -> Option<DiscountLine> {
            let amount: i64 = rsvps.iter().map(|r| r.discount).sum();
            if amount == 0 {
                return None;
            }
            let label = match coupon.kind.as_str() {
                Coupon::SPOT => {
                    format!("Free RSVP x{}", rsvps.iter().filter(|r| r.discount > 0).count())
                }
                Coupon::FIXED => format!("${} off", coupon.dollars_off.unwrap()),
                Coupon::PERCENT => format!("{}% off", coupon.percent_off.unwrap()),
                kind => panic!("unknown coupon kind: {kind}"),
            };
            Some(DiscountLine { label, amount })
        }
    }

    // Display the "Make your contribution" page after submitting attendees
    pub async fn contribution_page(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        if session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }

        // A user is guaranteed to exist: attendees_form() links one before setting CONTRIBUTION status
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
                let stripe::CheckoutSession { id, client_secret } = state
                    .stripe
                    .create_session(session.id, &user.email, line_items, return_url)
                    .await?;

                session.set_stripe_checkout_session(&state.db, &id, &client_secret).await?;
            }
        }

        let (coupon, discount) = match session.coupon_id {
            Some(coupon_id) => {
                let coupon = Coupon::lookup_by_id(&state.db, coupon_id).await?.expect("invalid coupon_id");
                let discount = DiscountLine::build(&coupon, &rsvps);
                (Some(coupon), discount)
            }
            None => (None, None),
        };
        let coupon_qty = rsvps.iter().filter(|r| r.discount > 0).count();
        let show_coupon =
            price > 0 && coupon.is_none() && Coupon::any_usable(&state.db, event.id, user.id).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_contribution.html")]
        struct ContributionHtml {
            event: Event,
            session: RsvpSession,
            rsvps: Vec<ContributionRsvp>,
            price: i64,
            discount: Option<DiscountLine>,
            coupon: Option<Coupon>,
            coupon_qty: usize,
            show_coupon: bool,
            stripe_publishable_key: String,
        }
        Ok(ContributionHtml {
            event,
            session,
            rsvps,
            price,
            discount,
            coupon,
            coupon_qty,
            show_coupon,
            stripe_publishable_key: state.config.stripe.publishable_key.clone(),
        }
        .into_response())
    }

    // Handle submission of $0 RSVPs.
    pub async fn contribution_form(
        State(state): State<SharedAppState>, session: RsvpSession, Path(slug): Path<String>,
    ) -> HtmlResult {
        if session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }

        let rsvps = Rsvp::list_for_contributions(&state.db, session.id).await?;
        let price: i64 = rsvps.iter().map(|r| r.contribution).sum();
        match price {
            0 => session.set_status(&state.db, RsvpSession::PAYMENT_CONFIRMED).await?,
            _ => bail_invalid!(),
        }

        // Send confirmation emails to any attendees who haven't gotten one yet.
        email::ensure_sent_confirmation_emails(&state, &event, &session, "contribution").await?;

        Ok(Redirect::to(&format!("/e/{slug}/rsvp/manage?reservation={}", session.token)).into_response())
    }

    // Check if a coupon is valid for the current session.
    #[derive(Debug, serde::Deserialize)]
    pub struct CheckCouponForm {
        code: String,
    }
    #[derive(serde::Serialize)]
    pub struct CheckCouponResponse {
        ok: bool,
        kind: Option<String>,
        available: i64,
        percent_off: Option<i64>,
        dollars_off: Option<i64>,
        message: Option<&'static str>,
    }
    pub async fn coupon_check_api(
        session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<CheckCouponForm>,
    ) -> JsonResult<CheckCouponResponse> {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        if session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        let check = Coupon::check(&state.db, &form.code, event.id, &session, &rsvps).await?;

        Ok(Json(match check {
            CouponCheck::Usable { coupon, available } => CheckCouponResponse {
                ok: true,
                kind: Some(coupon.kind),
                available,
                percent_off: coupon.percent_off,
                dollars_off: coupon.dollars_off,
                message: None,
            },
            unusable => CheckCouponResponse {
                ok: false,
                kind: None,
                available: 0,
                percent_off: None,
                dollars_off: None,
                message: unusable.error_message(),
            },
        }))
    }

    // Apply a discount code: reprice the rsvps and restart the Stripe checkout.
    #[derive(Debug, serde::Deserialize)]
    pub struct ApplyCouponForm {
        code: String,
        qty: Option<i64>,
    }
    pub async fn coupon_apply_form(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
        Form(form): Form<ApplyCouponForm>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }
        if session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        let mut rsvps = Rsvp::list_for_session(&state.db, session.id).await?;
        let check = Coupon::check(&state.db, &form.code, event.id, &session, &rsvps).await?;

        let (coupon, available) = match check {
            CouponCheck::Usable { coupon, available } => (coupon, available),
            check => {
                let message = check.error_message().unwrap().into();
                return Ok(ErrorHtml { user: None, message }.into_response());
            }
        };

        tracing::info!(
            "Applying coupon_id={} code={:?} kind={:?} to session_id={} event_id={} qty={:?}",
            coupon.id,
            coupon.code,
            coupon.kind,
            session.id,
            event.id,
            form.qty,
        );

        // Clear any previously applied code
        Rsvp::clear_discounts(&state.db, session.id).await?;

        // Apply discounts to spots in order from most to least expensive
        rsvps.sort_by_key(|r| Reverse(r.contribution));
        match coupon.kind.as_str() {
            Coupon::SPOT => {
                let qty = form.qty.unwrap_or(available).clamp(1, available);

                rsvps.sort_by_key(|r| Reverse(r.contribution));
                for rsvp in rsvps.iter().filter(|r| r.price() > 0).take(qty as usize) {
                    Rsvp::set_discount(&state.db, rsvp.rsvp_id, 0, rsvp.contribution).await?;
                }
            }
            Coupon::FIXED => {
                let mut remaining = coupon.dollars_off.unwrap();
                for rsvp in &rsvps {
                    let discount = remaining.min(rsvp.contribution);
                    if discount == 0 {
                        break;
                    }

                    Rsvp::set_discount(&state.db, rsvp.rsvp_id, rsvp.contribution - discount, discount)
                        .await?;
                    remaining -= discount;
                }
            }
            Coupon::PERCENT => {
                let percent = coupon.percent_off.unwrap();
                for rsvp in &rsvps {
                    let discount = (rsvp.contribution * percent) / 100;
                    if discount > 0 {
                        Rsvp::set_discount(&state.db, rsvp.rsvp_id, rsvp.contribution - discount, discount)
                            .await?;
                    }
                }
            }
            kind => panic!("unknown coupon kind: {kind}"),
        }
        session.set_coupon(&state.db, Some(coupon.id)).await?;

        // Expire the old Stripe checkout session; contribution_page will create another one at the new price.
        if let Some(checkout_session_id) = session.stripe_checkout_session_id.clone() {
            tracing::info!("Expiring stripe_checkout_session_id={checkout_session_id} after applying coupon");
            state.stripe.expire_session(&checkout_session_id).await?;
        }
        session.clear_stripe_client_secret(&state.db).await?;

        goto::contribution_page(&event)
    }

    // Remove the applied code and restore full prices.
    pub async fn coupon_remove_form(
        mut session: RsvpSession, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let link = session.link(&state.db).await?;
        if !event.registration_open(link.as_ref()) {
            return goto::error_registration_closed(&state.db, &state.stripe, &Some(session)).await;
        }
        if session.status != RsvpSession::CONTRIBUTION {
            bail_invalid!();
        }

        tracing::info!("Removing coupon_id={:?} from session_id={}", session.coupon_id, session.id);
        Rsvp::clear_discounts(&state.db, session.id).await?;
        session.set_coupon(&state.db, None).await?;

        // Expire the old Stripe checkout session; contribution_page will create another one at the new price.
        if let Some(checkout_session_id) = session.stripe_checkout_session_id.clone() {
            tracing::info!("Expiring stripe_checkout_session_id={checkout_session_id} for coupon");
            state.stripe.expire_session(&checkout_session_id).await?;
        }
        session.clear_stripe_client_secret(&state.db).await?;

        goto::contribution_page(&event)
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
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.reservation).await? else {
            return goto::error_rsvp_not_found();
        };

        // Child session tokens resolve to parent (Stripe return_url may use child token)
        let session = match session.parent_session_id {
            Some(parent_id) => {
                // First transition child state
                match session.status.as_str() {
                    RsvpSession::SELECTION => {
                        return goto::selection_page(&state.db, &None, &Some(session), &event).await;
                    }
                    RsvpSession::ATTENDEES => return goto::attendees_page(&event),
                    // If you get here, we hold your spot and assume payment is coming later via webhook.
                    // This is technically exploitable, but we could check for still unpaid rsvps at event start.
                    RsvpSession::CONTRIBUTION => {
                        session
                            .swap_status(&state.db, RsvpSession::CONTRIBUTION, RsvpSession::PAYMENT_PENDING)
                            .await?
                    }
                    RsvpSession::PAYMENT_PENDING
                    | RsvpSession::PAYMENT_CONFIRMED
                    | RsvpSession::REFUND_PENDING
                    | RsvpSession::REFUND_CONFIRMED => {}
                    _ => unreachable!(),
                }
                // Then resolve to parent session, and proceed as normal
                RsvpSession::lookup_by_id(&state.db, parent_id)
                    .await?
                    .expect("invalid parent_session_id")
            }
            None => session,
        };

        let Some(user_id) = session.user_id else {
            bail_invalid!()
        };

        // Transition state
        match session.status.as_str() {
            RsvpSession::SELECTION => {
                return goto::selection_page(&state.db, &None, &Some(session), &event).await;
            }
            RsvpSession::ATTENDEES => return goto::attendees_page(&event),
            // If you get here, we hold your spot and assume payment is coming later via webhook.
            // This is technically exploitable, but we could check for still unpaid rsvps at event start.
            RsvpSession::CONTRIBUTION => {
                session
                    .swap_status(&state.db, RsvpSession::CONTRIBUTION, RsvpSession::PAYMENT_PENDING)
                    .await?
            }
            RsvpSession::PAYMENT_PENDING
            | RsvpSession::PAYMENT_CONFIRMED
            | RsvpSession::REFUND_PENDING
            | RsvpSession::REFUND_CONFIRMED => {}
            _ => unreachable!(),
        }

        // If user's RSVP is deleted, they lose access to the manage page.
        // Checked *after* transition, which may have just committed the session.
        if !Rsvp::user_committed_for_event(&state.db, event.id, user_id).await? {
            return goto::error_rsvp_deleted(&session);
        }

        let flyer = EventFlyer::lookup(&state.db, event.id).await?;

        email::ensure_sent_confirmation_emails(&state, &event, &session, "manage").await?;
        if event.dayof_sent_at.is_some() {
            for user in Rsvp::list_family_users(&state.db, &event, user_id).await? {
                email::ensure_sent_dayof_email(&state, &event, &user, "manage").await?;
            }
        }

        // Aggregate RSVPs from parent + all confirmed children
        let rsvps = Rsvp::list_family_contributions(&state.db, &event, user_id).await?;
        let price = rsvps.iter().map(|r| r.contribution).sum::<i64>();
        let discount = match session.coupon_id {
            Some(coupon_id) => {
                let coupon = Coupon::lookup_by_id(&state.db, coupon_id).await?.expect("invalid coupon_id");
                DiscountLine::build(&coupon, &rsvps)
            }
            None => None,
        };

        // Check if user can add more guests
        let link = session.link(&state.db).await?;
        let can_add_guests = if event.registration_open(link.as_ref()) {
            let spots = Spot::list_for_event(&state.db, event.id).await?;
            let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[]).await?;
            let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
            let limits =
                event.compute_limits(&spots, &held_rsvps, session.user_id, manual_count, link.as_ref());
            limits.total_limit > 0
        } else {
            false
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "events/rsvp_manage.html")]
        struct ManageHtml {
            user: Option<User>,
            session: RsvpSession,
            event: Event,
            flyer: Option<EventFlyer>,
            rsvps: Vec<ContributionRsvp>,
            price: i64,
            discount: Option<DiscountLine>,
            can_add_guests: bool,
        }
        Ok(
            ManageHtml { user, session, event, flyer, rsvps, price, discount, can_add_guests }
                .into_response(),
        )
    }

    /// Handle "Add guests" form submission from the manage page.
    /// Creates or resumes a child session and redirects into the checkout flow.
    pub async fn add_guests_form(
        State(state): State<SharedAppState>, Query(query): Query<SessionQuery>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let parent_session = RsvpSession::lookup_by_token(&state.db, &query.reservation)
            .await?
            .ok_or_else(not_found)?;
        let link = parent_session.link(&state.db).await?;

        // Lookup parent session and validate parent session is confirmed
        match parent_session.status.as_str() {
            RsvpSession::PAYMENT_PENDING
            | RsvpSession::PAYMENT_CONFIRMED
            | RsvpSession::REFUND_PENDING
            | RsvpSession::REFUND_CONFIRMED => {}
            _ => bail_invalid!(),
        }

        let child_session = match RsvpSession::lookup_unconfirmed_child(&state.db, parent_session.id).await? {
            // Resume any existing unconfirmed "Add Guests" session.
            Some(child_session) => child_session,
            // Otherwise start a new one, subject to limits.
            None => {
                if !event.registration_open(link.as_ref()) {
                    return goto::error_registration_closed(&state.db, &state.stripe, &None).await;
                }

                let spots = Spot::list_for_event(&state.db, event.id).await?;
                let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &[]).await?;
                let manual_count = ManualRsvp::count_for_event(&state.db, event.id).await?;
                let limits = event.compute_limits(
                    &spots,
                    &held_rsvps,
                    parent_session.user_id,
                    manual_count,
                    link.as_ref(),
                );
                if limits.total_limit == 0 {
                    return goto::error_at_capacity(&state.db, &state.stripe, &None).await;
                }

                RsvpSession::create_child(&state.db, &parent_session).await?
            }
        };

        let cookie = parent_session.cookie(&format!("/e/{slug}"), link.as_ref());
        let redirect = goto::by_status(&state.db, &event, child_session).await?;
        Ok(([(header::SET_COOKIE, cookie)], redirect).into_response())
    }

    // Show the "Manage your RSVP" page.
    #[allow(unused)]
    pub async fn temp_delete(
        State(state): State<SharedAppState>, Query(query): Query<SessionQuery>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let session = RsvpSession::lookup_by_token(&state.db, &query.reservation)
            .await?
            .ok_or_else(not_found)?;
        session.delete(&state.db, &state.stripe).await?;
        Ok(Redirect::to(&format!("/e/{slug}")).into_response())
    }

    // Show the editor for "Who will be attending?" page.
    pub async fn edit_guests_page(
        user: Option<User>, State(state): State<SharedAppState>, Query(query): Query<SessionQuery>,
        Path(slug): Path<String>,
    ) -> HtmlResult {
        let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or_else(not_found)?;
        let Some(session) = RsvpSession::lookup_by_token(&state.db, &query.reservation).await? else {
            bail_not_found!();
        };

        // Load RSVPs from parent + all confirmed children
        let user_id = session.user_id.unwrap();
        let rsvps = Rsvp::list_family_attendees(&state.db, &event, user_id).await?;
        let mode = AttendeesMode::Edit;
        Ok(AttendeesHtml {
            mode,
            event,
            user,
            session,
            attendees: rsvps,
            price: 0,
            is_adding_guests: false,
        }
        .into_response())
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

        // Collect rsvps from parent + all confirmed children for validation
        let user_id = session.user_id.unwrap();
        let our_rsvps = Rsvp::list_family_rsvps(&state.db, &event, user_id).await?;

        // Parse and validate form. NOTE that we only allow editing guest info.
        // The primary_attendee form is disabled on the frontend, and changes are ignored here.
        let (primary_attendee, guest_attendees) =
            parse::attendees_form(&user, &our_rsvps, &form.attendees, false, true)
                .await
                .map_err(|e| {
                    alert!("edit_form(): session={} form={form:?}: {e}", session.token);
                    invalid()
                })?;

        // Check for conflicts, possibly taking over another session.
        let primary_user = &primary_attendee.as_ref().unwrap().user;
        let family_sessions = RsvpSession::list_for_user(&state.db, user_id, event.id).await?;
        let family_ids: Vec<i64> = family_sessions.iter().map(|s| s.id).collect();
        let held_rsvps = Rsvp::list_held_for_event(&state.db, &event, &family_ids).await?;
        if let Some(conflict) = validate::no_conflicts(&held_rsvps, Some(primary_user), &guest_attendees)
            && let Some(resp) = resolve_conflict(&state, &event, None, conflict).await?
        {
            return Ok(resp);
        }

        // Update guests
        for ParsedAttendee { rsvp_id, user } in guest_attendees {
            let user = User::upsert_for_rsvp(&state.db, &user, session.created_at).await?;
            Rsvp::set_user(&state.db, rsvp_id, &user).await?;
        }

        goto::manage_page(&session, &event)
    }

    // Resolve an RSVP conflict. Returns:
    // * Ok(None) to proceed: the primary's own pre-payment draft was taken over.
    // * Ok(Some(resp)) for the caller to return: a guest collision, the primary's own committed
    //   RSVP, or someone else's reservation listing them.
    // exclude_session_id is the caller's current session, which is spared from takeover.
    async fn resolve_conflict(
        state: &SharedAppState, event: &Event, exclude_session_id: Option<i64>, conflict: validate::Conflict,
    ) -> std::result::Result<Option<Response>, HtmlError> {
        use validate::Conflict;
        let Conflict::Primary { is_own_session, ref status, ref email, .. } = conflict else {
            // A guest collision is never takeoverable.
            return Ok(Some(goto::error_conflict(&conflict)?));
        };
        tracing::info!(
            "RSVP primary conflict event_id={} email={email:?} status={status:?} is_own_session={is_own_session}",
            event.id,
        );
        let pre_payment = status == RsvpSession::CONTRIBUTION;
        if is_own_session && pre_payment {
            RsvpSession::delete_drafts_for_email(&state.db, &state.stripe, event, email, exclude_session_id)
                .await?;
            Ok(None)
        } else {
            Ok(Some(goto::error_conflict(&conflict)?))
        }
    }

    /// Helpers for changing RSVP session state and redirecting.
    #[rustfmt::skip]
    pub mod goto {
        use super::*;
        use crate::utils::stripe::Stripe;

        /// Redirect to the correct page for the session's status.
        pub async fn by_status(db: &Db, event: &Event, session: RsvpSession) -> HtmlResult {
            match session.status.as_str() {
                RsvpSession::SELECTION => goto::selection_page(db, &None, &Some(session), event).await,
                RsvpSession::ATTENDEES => goto::attendees_page(event),
                RsvpSession::CONTRIBUTION => goto::contribution_page(event),
                RsvpSession::PAYMENT_PENDING
                    | RsvpSession::PAYMENT_CONFIRMED
                    | RsvpSession::REFUND_PENDING
                    | RsvpSession::REFUND_CONFIRMED
                    => goto::manage_page(&session, event),
                _ => unreachable!(),
            }
        }

        pub fn guestlist_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/guestlist", event.slug)).into_response())
        }
        pub async fn selection_page(db: &Db, user: &Option<User>, session: &Option<RsvpSession>, event: &Event) -> HtmlResult {
            let redirect = Redirect::to(&format!("/e/{}/rsvp/selection", event.slug));
            match session {
                Some(_) => Ok(redirect.into_response()),
                None => {
                    let session = RsvpSession::create(db, event.id, user, None).await?;

                    let cookie = session.cookie(&format!("/e/{}", event.slug), None);
                    let header = (header::SET_COOKIE, cookie);

                    Ok(([header], redirect).into_response())
                }
            }
        }
        pub fn attendees_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/attendees", event.slug)).into_response())
        }
        pub fn contribution_page(event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/contribution", event.slug)).into_response())
        }
        pub fn manage_page(session: &RsvpSession, event: &Event) -> HtmlResult {
            Ok(Redirect::to(&format!("/e/{}/rsvp/manage?reservation={}", event.slug, session.token)).into_response())
        }
        pub fn error_not_on_guestlist() -> HtmlResult {
            let error = ErrorHtml { user: None, message: "Sorry, you're not on the list.".into() };
            Ok(error.into_response())
        }
        pub fn error_wrong_password() -> HtmlResult {
            let error = ErrorHtml { user: None, message: "Incorrect password.".into() };
            Ok(error.into_response())
        }
        pub async fn error_at_capacity(db: &Db, stripe: &Stripe, session: &Option<RsvpSession>) -> HtmlResult {
            if let Some(session) = session {
                session.delete(db, stripe).await?;
            }
            Ok(MessageHtml {
                user: None,
                title: "Sorry".into(),
                message: "This event has reached capacity.".into(),
            }
            .into_response())
        }
        pub async fn error_registration_closed(db: &Db, stripe: &Stripe, session: &Option<RsvpSession>) -> HtmlResult {
            if let Some(session) = session {
                session.delete(db, stripe).await?;
            }
            Ok(MessageHtml {
                user: None,
                title: "Sorry".into(),
                message: "Registration for this event is now closed.".into(),
            }
            .into_response())
        }
        pub async fn error_spot_taken(db: &Db, stripe: &Stripe, session: &RsvpSession) -> HtmlResult {
            session.delete(db, stripe).await?;
            Ok(ErrorHtml {
                user: None,
                message: "Sorry, a spot you selected was taken. Please try again.".to_string(),
            }
            .into_response())
        }
        #[rustfmt::skip]
        pub fn error_conflict(conflict: &validate::Conflict) -> HtmlResult {
            use validate::Conflict;
            let message = match conflict {
                // A guest the user is adding is already reserved.
                Conflict::Guest { email, status } => match status.as_str() {
                    RsvpSession::SELECTION | RsvpSession::ATTENDEES => unreachable!("spots are only reserved and can conflict in CONTRIBUTION and later"),
                    RsvpSession::CONTRIBUTION =>
                        format!("{email} is currently in the process of RSVPing for this event."),
                    RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED | RsvpSession::REFUND_PENDING | RsvpSession::REFUND_CONFIRMED =>
                        format!("{email} has already RSVPed for this event."),
                    _ => unreachable!(),
                },
                // Their own committed session. Pre-payment drafts were taken over, so they never reach here.
                Conflict::Primary { is_own_session: true, status, .. } => match status.as_str() {
                    RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED | RsvpSession::REFUND_PENDING | RsvpSession::REFUND_CONFIRMED =>
                        "You're already confirmed for this event! Manage your RSVP via the link sent to your email.".to_string(),
                    _ => unreachable!(),
                },
                // They are listed as a guest on someone else's reservation.
                Conflict::Primary { is_own_session: false, status, owner_name, .. } => {
                    let who = owner_name.as_deref().unwrap_or("another attendee");
                    let contact = config().email.contact_to.as_ref().unwrap_or(&config().email.from);
                    match status.as_str() {
                        RsvpSession::PAYMENT_PENDING | RsvpSession::PAYMENT_CONFIRMED | RsvpSession::REFUND_PENDING | RsvpSession::REFUND_CONFIRMED =>
                            format!("You've already been RSVPed to this event by {who}! Check your email for your confirmation."),
                        RsvpSession::SELECTION | RsvpSession::ATTENDEES => unreachable!("spots are only reserved and can conflict in CONTRIBUTION and later"),
                        RsvpSession::CONTRIBUTION =>
                            format!("{who} is currently RSVPing with you listed as a guest. Please check with them, otherwise contact {contact}."),
                        _ => unreachable!(),
                    }
                }
            };
            Ok(ErrorHtml {
                message,
                user: None,
            }.into_response())
        }
        pub fn error_rsvp_not_found() -> HtmlResult {
            let error = ErrorHtml {
                user: None,
                message: "Reservation not found.".into()
            };
            Ok(error.into_response())
        }
        pub fn error_rsvp_deleted(session: &RsvpSession) -> HtmlResult {
            let contact = &config().email.contact_to.as_ref().unwrap_or(&config().email.from).email;
            let contact_link = format!(r#"<a href="mailto:{contact}?subject=Help%20with%20reservation%20{}">contact us</a>"#, session.token);
            let error = ErrorHtmlSafe {
                user: None,
                message: format!("Your reservation has been cancelled. If you believe this is an error, please {contact_link}."),
            };
            Ok(error.into_response())
        }
    }

    mod validate {
        use super::*;
        use crate::db::rsvp::HeldRsvp;

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
            // The primary's email is already reserved. is_own_session distinguishes whether they
            // own it (their prior attempt) or are a guest on owner_name's reservation.
            Primary {
                email: String,
                status: String,
                is_own_session: bool,
                owner_name: Option<String>,
            },
            Guest {
                email: String,
                status: String,
            },
        }
        #[rustfmt::skip]
        pub fn no_conflicts(
            held_rsvps: &[HeldRsvp], primary: Option<&CreateUser>, guests: &[parse::ParsedAttendee],
        ) -> Option<Conflict> {
            for rsvp in held_rsvps {
                let Some(email) = &rsvp.email else { continue };
                if primary.is_some_and(|u| &u.email == email) {
                    let is_own_session = rsvp.owner_user_id == rsvp.attendee_user_id;
                    let owner_name = match (&rsvp.owner_first_name, &rsvp.owner_last_name) {
                        (Some(f), Some(l)) => Some(format!("{f} {l}")),
                        (Some(f), None) => Some(f.clone()),
                        _ => None,
                    };
                    return Some(Conflict::Primary {
                        email: email.clone(),
                        status: rsvp.status.clone(),
                        is_own_session,
                        owner_name,
                    })
                }

                for guest in guests {
                    if &guest.user.email == email {
                        return Some(Conflict::Guest { email: email.clone(), status: rsvp.status.clone() });
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
            #[error("failed to parse request: {0}")]
            Parse(#[from] serde_json::Error),
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

            let rsvps: Vec<RsvpForm> = serde_json::from_str(selection)?;
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
                    parsed.push(EventRsvp { rsvp_id: 0, spot_id, contribution, discount: 0 })
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
            #[error("failed to parse request: {0}")]
            Parse(#[from] serde_json::Error),

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
            #[error("invalid email: {email}")]
            InvalidEmail { email: String },
            #[error("duplicate email: {email}")]
            DuplicateEmail { email: String },
            #[error("duplicate phone: {phone}")]
            DuplicatePhone { phone: String },
        }
        pub async fn attendees_form(
            session_user: &Option<User>, rsvps: &[EventRsvp], attendees: &str, is_adding_guests: bool,
            lock_primary: bool,
        ) -> Result<(Option<ParsedAttendee>, Vec<ParsedAttendee>), ParseAttendeesError> {
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
            let attendees: Vec<AttendeeForm> = serde_json::from_str(attendees)?;

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
                if email.parse::<lettre::Address>().is_err() {
                    return Err(Error::InvalidEmail { email });
                }
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
                    // When the primary attendee is locked on the frontend (private events, or Edit mode),
                    // reject any attempt to change the email from the session user's email.
                    if lock_primary && session_user.as_ref().is_some_and(|u| attendee.user.email != u.email) {
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
            // Ensure exactly one primary attendee (unless adding guests to an existing session).
            if !is_adding_guests && primary_attendee.is_none() {
                return Err(Error::MissingPrimary);
            }

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

// Build and send event emails.
pub mod email {
    use lettre::Message;
    use lettre::message::header::{ContentDisposition, ContentTransferEncoding, ContentType};
    use lettre::message::{MultiPart, SinglePart};

    use super::*;
    use crate::db::rsvp::Rsvp;
    use crate::utils::calendar;

    pub fn format_confirmation_email(
        state: &SharedAppState, event: &Event, flyer: &Option<EventFlyer>, address: &str,
        session_token: &str, email_token: &str, is_guest: bool,
    ) -> Result<Message> {
        #[derive(Template, WebTemplate)]
        #[template(path = "emails/event_confirmation.html")]
        struct ConfirmationEmailHtml<'a> {
            email_token: &'a str,
            event: &'a Event,
            token: &'a str,
            flyer: &'a Option<EventFlyer>,
            is_guest: bool,
        }
        let html =
            ConfirmationEmailHtml { email_token, event, token: session_token, flyer, is_guest }.render()?;

        let subject = event
            .confirmation_subject
            .clone()
            .unwrap_or_else(|| format!("Confirmation for {}", event.title));
        let from = &state.config.email.from;
        let reply_to = state.config.email.contact_to.as_ref().unwrap_or(from);

        let cal = calendar::CalendarEvent::from_event(event);
        let organizer_email = from.email.to_string();
        let organizer = calendar::Organizer { name: from.name.as_deref(), email: &organizer_email };
        let ics = cal.email_ics(&organizer);

        // The calendar part must sit inside multipart/alternative to render natively.
        let body = MultiPart::mixed()
            .multipart(
                MultiPart::alternative().singlepart(SinglePart::html(html)).singlepart(
                    SinglePart::builder()
                        .header(ContentType::parse("text/calendar; charset=utf-8; method=PUBLISH")?)
                        .header(ContentTransferEncoding::Base64)
                        .body(ics.clone()),
                ),
            )
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::parse("application/ics")?)
                    .header(ContentDisposition::attachment("event.ics"))
                    .header(ContentTransferEncoding::Base64)
                    .body(ics),
            );

        let message = state
            .mailer
            .builder(email_token)
            .to(address.parse()?)
            .reply_to(reply_to.clone())
            .subject(subject)
            .multipart(body)
            .unwrap();

        Ok(message)
    }

    /// Ensure confirmation emails are sent to all attendees of a reservation, if not already.
    pub async fn ensure_sent_confirmation_emails(
        state: &SharedAppState, event: &Event, session: &RsvpSession, trigger: &str,
    ) -> Result<()> {
        if event.is_over() {
            return Ok(());
        }

        let primary_id = session.user_id.unwrap();
        let attendees = Rsvp::list_family_users(&state.db, event, primary_id).await?;

        let flyer = EventFlyer::lookup(&state.db, event.id).await?;
        let mut messages = vec![];
        let mut sent_to = vec![];
        for user in attendees {
            let is_guest = user.id != primary_id;
            let Some(email) = Email::try_create_confirmation(&state.db, event.id, user.id).await? else {
                // we've already sent a confirmation email
                continue;
            };

            match format_confirmation_email(
                state,
                event,
                &flyer,
                &user.email,
                &session.token,
                &email.token,
                is_guest,
            ) {
                Ok(message) => {
                    messages.push(message);
                    sent_to.push((email.id, user.email));
                }
                Err(e) => {
                    let e = e.message();
                    Email::mark_error(&state.db, email.id, e).await?;
                    alert!(
                        "Error formatting confirmation for event_id={} to email={:?} trigger={trigger}: {e}",
                        event.id,
                        user.email
                    );
                }
            }
        }

        let results = state.mailer.send_batch(Arc::clone(state), messages).await;
        let mut results = Box::pin(results.zip(futures::stream::iter(sent_to)));
        while let Some((result, (email_id, address))) = results.next().await {
            match result {
                Ok(_) => {
                    Email::mark_sent(&state.db, email_id).await?;
                    tracing::info!(
                        "Confirmation for event_id={} sent to email={address:?} trigger={trigger}",
                        event.id,
                    );
                }
                Err(e) => {
                    let e = e.message();
                    Email::mark_error(&state.db, email_id, e).await?;
                    alert!(
                        "Error sending confirmation for event_id={} to email={address:?} trigger={trigger}: {e}",
                        event.id,
                    );
                }
            }
        }

        Ok(())
    }

    pub fn format_dayof_email(
        state: &SharedAppState, event: &Event, flyer: &Option<EventFlyer>, address: &str, email_token: &str,
    ) -> Result<Message> {
        #[derive(Template, WebTemplate)]
        #[template(path = "emails/event_dayof.html")]
        struct DayofEmailHtml<'a> {
            email_token: &'a str,
            event: &'a Event,
            flyer: &'a Option<EventFlyer>,
        }
        let html = DayofEmailHtml { email_token, event, flyer }.render()?;

        let subject = event.dayof_subject.as_deref().expect("missing dayof_subject");
        let from = &state.config.email.from;
        let reply_to = state.config.email.contact_to.as_ref().unwrap_or(from);
        let message = state
            .mailer
            .builder(email_token)
            .to(address.parse().unwrap())
            .reply_to(reply_to.clone())
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .unwrap();

        Ok(message)
    }

    /// Ensure a dayof email is sent to a user, if not already.
    pub async fn ensure_sent_dayof_email(
        state: &SharedAppState, event: &Event, user: &User, trigger: &str,
    ) -> Result<()> {
        let Some(email) = Email::try_create_send_dayof_single(&state.db, event.id, user.id).await? else {
            return Ok(()); // Already sent a dayof email
        };

        let flyer = EventFlyer::lookup(&state.db, event.id).await?;
        let message = format_dayof_email(state, event, &flyer, &user.email, &email.token)?;

        match state.mailer.send(&message).await {
            Ok(_) => {
                Email::mark_sent(&state.db, email.id).await?;
                tracing::info!(
                    "Day-of for event_id={} sent to email={:?} trigger={trigger}",
                    event.id,
                    user.email
                );
            }
            Err(e) => {
                let e = e.message();
                Email::mark_error(&state.db, email.id, e).await?;
                alert!(
                    "Error sending day-of for event_id={} to email={:?} trigger={trigger}: {e}",
                    event.id,
                    user.email
                );
            }
        }

        Ok(())
    }
}

pub fn add_middleware(router: AxumRouter, state: SharedAppState) -> AxumRouter {
    return router
        .layer(axum::middleware::from_fn(rsvp_disable_bfcache))
        .layer(axum::middleware::from_fn_with_state(state, rsvp_parse_session_cookie));

    /// Resolve the `rsvp_session` cookie and populate an `RsvpSession` for use through an extractor.
    pub async fn rsvp_parse_session_cookie(
        State(state): State<SharedAppState>, cookies: CookieJar, mut request: Request, next: Next,
    ) -> HtmlResult {
        // Lookup an RsvpSession via token stored in the cookie.
        if let Some(cookie) = cookies.get("rsvp_session")
            && let (token, _link) = RsvpSession::parse_cookie(cookie.value())
            && let Some(mut session) = RsvpSession::lookup_by_token(&state.db, token).await?
        {
            // If session has an active child "Add Guests" session, replace it with that.
            if let Some(child_session) = RsvpSession::lookup_unconfirmed_child(&state.db, session.id).await? {
                session = child_session;
            };
            request.extensions_mut().insert(session);
        }

        Ok(next.run(request).await)
    }

    /// Prevent browsers from storing stateful RSVP pages in the back-forward cache.
    pub async fn rsvp_disable_bfcache(request: Request, next: Next) -> HtmlResult {
        let is_rsvp_path = request.uri().path().contains("/rsvp");
        let mut res = next.run(request).await;
        if is_rsvp_path {
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        Ok(res)
    }
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
                RsvpSession::PAYMENT_PENDING
                | RsvpSession::PAYMENT_CONFIRMED
                | RsvpSession::REFUND_PENDING
                | RsvpSession::REFUND_CONFIRMED => {
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
            None => {
                let cookies = CookieJar::from_headers(&parts.headers);
                match cookies
                    .get("rsvp_session")
                    .map(|cookie| RsvpSession::parse_cookie(cookie.value()))
                {
                    Some((_, Some(link_slug))) => Err(Redirect::to(&format!("/e/{slug}/{link_slug}"))),
                    Some((_, None)) | None => Err(Redirect::to(&format!("/e/{slug}"))),
                }
            }
        }
    }
}

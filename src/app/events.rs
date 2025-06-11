use axum::Json;
use reqwest::header::CONTENT_TYPE;
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::db::event::{Event, ReservationType, UpdateEvent};
use crate::prelude::*;

/// Add all `events` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/events", get(list_events_page))
            .route("/events/new", get(create_event_page).post(create_event_form))
            .route(
                "/events/{slug}/edit",
                get(update_event_page).post(update_event_form).delete(delete_event),
            )
            .route("/events/{slug}", get(view_event_page))
            .route("/events/{slug}/checkout", get(checkout_page))
            .route("/create-checkout-session", post(checkout_session))
    })
}

async fn view_event_page(
    State(state): State<SharedAppState>,
    Path(slug): Path<String>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/view.html")]
    struct Html {
        event: Event,
    }
    Ok(Html { event })
}

struct SliderMarker {
    text: String,
    description: Option<String>,
    value_cents: i64,
}

#[derive(Deserialize)]
struct CheckoutQuery {
    reservation_type: String,
}

async fn checkout_page(
    State(state): State<SharedAppState>,
    Path(slug): Path<String>,
    Query(query): Query<CheckoutQuery>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;

    let reservation_type =
        Event::lookup_reservation_type_by_name(&state.db, event.id, &query.reservation_type)
            .await?
            .ok_or(AppError::NotFound)?;

    #[derive(Template, WebTemplate)]
    #[template(path = "events/checkout.html")]
    struct Html {
        event: Event,
        reservation_type: ReservationType,
        markers: Vec<SliderMarker>,
    }

    let markers = vec![
        SliderMarker { text: "Median".into(), description: Some("50%".into()), value_cents: 1000 },
        SliderMarker {
            text: reservation_type.name.clone(),
            description: reservation_type.details.clone(),
            value_cents: reservation_type.recommended_contribution,
        },
    ];

    Ok(Html { event, reservation_type, markers })
}

#[derive(Debug, Deserialize)]
struct CheckoutSessionData {
    price: i64,
}

async fn checkout_session(
    // user: Option<User>,
    Path(id): Path<i64>,
    State(state): State<SharedAppState>,
    Json(CheckoutSessionData { price }): Json<CheckoutSessionData>,
) -> AppResult<String> {
    let price_cents = price * 100;

    // Gross but there doesn't seem to be any other supported way to build form data in the way
    // that stripe expects in particular for lists of objects.
    // The v2 APIs will allow sending JSON data but currently checkout API doesn't support v2
    // as of 2025-06-10.
    // See https://docs.stripe.com/api/checkout/sessions/create?api-version=2025-05-28.basil
    let form_data = format!("ui_mode=custom&return_url={}/events/{}/confirm?session_id={{CHECKOUT_SESSION_ID}}&mode=payment&currency=usd&allow_promotion_codes=true&payment_method_types[]=card&payment_method_types[]=cashapp&line_items[0][quantity]=1&line_items[0][price_data][currency]=usd&line_items[0][price_data][unit_amount]={}&line_items[0][price_data][product_data][name]=Contribution&line_items[0][price_data][product_data][description]=Contribute to this event", state.config.app.url, id,price_cents);

    // if let Some(user) = user {
    //     form_data = format!("{}&customer_email={}", form_data, user.email);
    // }

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .header(
            "Authorization",
            format!("Bearer {}", state.secrets.stripe_secret_key.expose_secret()),
        )
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form_data)
        .send()
        .await
        .unwrap();

    tracing::info!("{res:?}");

    let json: serde_json::Value = res.json().await?;
    let client_secret = json["client_secret"]
        .as_str()
        .context("Cannot parse client_secret as string")?
        .to_string();

    Ok(client_secret)
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
            true => e.start_date < now,
            // TODO: Don't filter out in-progress events until they're over.
            // Need to add an `end_date` field.
            false => e.start_date >= now,
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

#[derive(Template, WebTemplate)]
#[template(path = "events/edit.html")]
pub struct EditHtml {
    pub event: Event,
}

/// Display the form to create a new event.
async fn create_event_page() -> AppResult<impl IntoResponse> {
    Ok(EditHtml {
        event: Event {
            id: 0,
            title: "".into(),
            url: "".into(),
            artist: "".into(),
            description: "".into(),
            start_date: Utc::now().naive_utc(),
            target_revenue: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
    })
}

/// Process the form and create a new event.
async fn create_event_form(
    State(state): State<SharedAppState>,
    Form(form): Form<UpdateEvent>,
) -> AppResult<impl IntoResponse> {
    let _ = Event::create(&state.db, &form).await?;
    // TODO: Redirect to event page.
    Ok("Event created.")
}

/// Display the form to update an event.
async fn update_event_page(
    State(state): State<SharedAppState>,
    Path(slug): Path<String>,
) -> AppResult<impl IntoResponse> {
    let event = Event::lookup_by_slug(&state.db, &slug).await?.ok_or(AppError::NotFound)?;
    Ok(EditHtml { event })
}

/// Process the form and update an event.
async fn update_event_form(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateEvent>,
) -> AppResult<impl IntoResponse> {
    Event::update(&state.db, id, &form).await?;
    // TODO: Redirect to event page.
    Ok("Event updated.")
}

/// Delete an event.
async fn delete_event(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    Event::delete(&state.db, id).await?;
    Ok(Redirect::to("/events"))
}

use crate::db::coupon::{Coupon, CouponWithUses, UpdateCoupon};
use crate::db::event::{Event, EventSelect};
use crate::prelude::*;

/// Add all `coupons` routes to the router. Coupons are called "codes" user-facing.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/coupons", get(list_page))
         .route("/coupons/new", get(new_page))
         .route("/coupons/{id}", get(edit_page).post(edit_form))
         .route("/coupons/{id}/disable", post(disable_form))
    })
}

#[derive(Template, WebTemplate)]
#[template(path = "coupons/edit.html")]
struct EditHtml {
    user: Option<User>,
    coupon: Coupon,
    users: Vec<User>,
    events: Vec<EventSelect>,
}

// Display all coupons.
async fn list_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
    let coupons = Coupon::list(&state.db).await?;
    let show_expires = coupons.iter().any(|c| c.expires_at.is_some());

    #[derive(Template, WebTemplate)]
    #[template(path = "coupons/list.html")]
    struct Html {
        user: Option<User>,
        coupons: Vec<CouponWithUses>,
        show_expires: bool,
    }
    Ok(Html { user: Some(user), coupons, show_expires }.into_response())
}

// Display the form to create a new coupon.
async fn new_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
    let events = Event::list_upcoming_for_select(&state.db).await?;
    Ok(EditHtml {
        user: Some(user),
        coupon: Coupon {
            id: 0,
            creator_user_id: 0,
            event_id: None,
            code: "".into(),
            description: "".into(),
            expires_at: None,
            max_uses: None,
            max_uses_per_user: None,
            max_uses_per_event: None,
            kind: Coupon::SPOT.into(),
            percent_off: None,
            dollars_off: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
        users: vec![],
        events,
    }
    .into_response())
}

// Display the form to edit a coupon.
async fn edit_page(user: User, State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
    let coupon = Coupon::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
    let users = User::lookup_by_coupon_id(&state.db, id).await?;
    let events = Event::list_upcoming_for_select(&state.db).await?;
    Ok(EditHtml { user: Some(user), coupon, users, events }.into_response())
}

// Handle create/edit submission.
#[derive(serde::Deserialize)]
struct EditForm {
    coupon: String,
}
#[derive(serde::Deserialize)]
struct EditJson {
    #[serde(flatten)]
    coupon: UpdateCoupon,
    user_ids: Vec<i64>,
}
async fn edit_form(
    admin: User, State(state): State<SharedAppState>, Path(id): Path<i64>, Form(form): Form<EditForm>,
) -> HtmlResult {
    let EditJson { coupon, user_ids } = serde_json::from_str(&form.coupon).map_err(|_| invalid())?;

    // Check for duplicates
    let code = &coupon.code;
    if let Some(existing) = Coupon::lookup_by_code(&state.db, code).await?
        && existing.id != id
    {
        bail_bad_request!("Code '{code}' already exists.");
    }

    match id {
        0 => {
            tracing::info!("create coupon={coupon:?} users={user_ids:?}");
            Coupon::create(&state.db, admin.id, &coupon).await?;
        }
        id => {
            tracing::info!("edit coupon={coupon:?} users={user_ids:?}");
            Coupon::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
            Coupon::update(&state.db, id, &coupon).await?;
        }
    };
    Coupon::set_users(&state.db, id, &user_ids).await?;

    Ok(Redirect::to("/coupons").into_response())
}

// Disable a coupon by expiring it now.
async fn disable_form(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
    Coupon::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;
    Coupon::disable(&state.db, id).await?;
    Ok(Redirect::to("/coupons").into_response())
}

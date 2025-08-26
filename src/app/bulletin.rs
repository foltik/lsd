use axum::routing::post;
use axum::Json;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::prelude::*;

// TODO(sam) ensure minimal data serialization
// TODO(sam) implement remove-after logic for remove_after_time
// TODO(sam) honestly maybe disable moving the flyers at all?
// TODO(sam) event description etc that shows when the flyer is clicked?
// TODO(sam) event URL that links to the event details?
#[derive(Debug, Serialize, Deserialize)]
struct Flyer {
    id: i64,
    user_id: i64,
    flyer_name: Option<String>,
    x: i64,
    y: i64,
    rotation: i64,
    image_url: String,
    remove_after_time: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateFlyerForm {
    image_url: String,
    flyer_name: Option<String>,
    remove_after_time: NaiveDateTime,
    x: i64,
    y: i64,
}

async fn create_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Form(flyer): Form<CreateFlyerForm>,
) -> AppResult<impl IntoResponse> {
    sqlx::query!(
        r#"INSERT INTO flyers (user_id, x, y, image_url, flyer_name, remove_after_time)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        user.id,
        flyer.x,
        flyer.y,
        flyer.image_url,
        flyer.flyer_name,
        flyer.remove_after_time
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/bulletin"))
}

#[derive(Debug, Serialize, Deserialize)]
struct FlyerUpdate {
    image_url: String,
    flyer_name: String,
    remove_after_time: NaiveDateTime,
}

async fn update_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Form(update): Form<FlyerUpdate>,
) -> AppResult<impl IntoResponse> {
    if user.has_role(&state.db, User::ADMIN).await? {
        sqlx::query!(
            r#"UPDATE flyers
               SET image_url = ?, flyer_name = ?, remove_after_time = ?
               WHERE id = ?"#,
            update.image_url,
            update.flyer_name,
            update.remove_after_time,
            id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            r#"UPDATE flyers
               SET image_url = ?, flyer_name = ?, remove_after_time = ?
               WHERE id = ? AND user_id = ?"#,
            update.image_url,
            update.flyer_name,
            update.remove_after_time,
            id,
            user.id
        )
        .execute(&state.db)
        .await?;
    }

    Ok(Redirect::to("/bulletin"))
}

#[derive(Debug, Serialize, Deserialize)]
struct FlyerMoveUpdate {
    x: i64,
    y: i64,
    rotation: i64,
}

async fn move_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(update): Json<FlyerMoveUpdate>,
) -> AppResult<()> {
    if user.has_role(&state.db, User::ADMIN).await? {
        sqlx::query!(
            r#"UPDATE flyers
               SET x = ?, y = ?, rotation = ?
               WHERE id = ?"#,
            update.x,
            update.y,
            update.rotation,
            id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            r#"UPDATE flyers
               SET x = ?, y = ?, rotation = ?
               WHERE id = ? AND user_id = ?"#,
            update.x,
            update.y,
            update.rotation,
            id,
            user.id
        )
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

async fn delete_flyer(user: User, State(state): State<SharedAppState>, Path(id): Path<i64>) -> AppResult<()> {
    if user.has_role(&state.db, User::ADMIN).await? {
        sqlx::query!("DELETE FROM flyers WHERE id = ?", id).execute(&state.db).await?;
    } else {
        sqlx::query!("DELETE FROM flyers WHERE id = ? AND user_id = ?", id, user.id)
            .execute(&state.db)
            .await?;
    }

    Ok(())
}

async fn flyer_details(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let flyer = sqlx::query_as!(
        FlyerUpdate,
        "SELECT flyer_name, image_url, remove_after_time FROM flyers WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(flyer))
}

async fn bulletin_page(
    user: Option<User>,
    State(state): State<SharedAppState>,
) -> AppResult<impl IntoResponse> {
    let flyers = sqlx::query_as!(
        Flyer,
        "SELECT id, user_id, flyer_name, x, y, rotation, image_url, remove_after_time FROM flyers"
    )
    .fetch_all(&state.db)
    .await?;

    let (editable_flyers, read_only_flyers) = if let Some(ref u) = user {
        let is_admin = u.has_role(&state.db, User::ADMIN).await?;
        if is_admin {
            (flyers.into_iter().collect(), Vec::new())
        } else {
            flyers.into_iter().partition(|f| f.user_id == u.id)
        }
    } else {
        (Vec::new(), flyers.into_iter().collect())
    };

    #[derive(Template, WebTemplate)]
    #[template(path = "bulletin/index.html")]
    struct Html {
        read_only_flyers: Vec<Flyer>,
        editable_flyers: Vec<Flyer>,
        user: Option<User>,
    }

    Ok(Html { read_only_flyers, editable_flyers, user })
}

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/bulletin", get(bulletin_page))
            .route("/bulletin/flyer/new", post(create_flyer))
            .route("/bulletin/flyer/{id}", delete(delete_flyer).get(flyer_details))
            .route("/bulletin/flyer/{id}/edit", post(update_flyer))
            .route("/bulletin/flyer/{id}/move", post(move_flyer))
    })
}

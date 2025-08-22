use axum::routing::{post, put};
use axum::Json;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct Flyer {
    id: i64,
    x: i64,
    y: i64,
    rotation: i64,
    z_index: i64,
    image_url: String,
    event_url: Option<String>,
    event_end: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlyerForm {
    image_url: String,
    event_name: Option<String>,
    event_url: Option<String>,
    event_end: NaiveDateTime,
    x: i64,
    y: i64,
}

async fn create_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Form(flyer): Form<FlyerForm>,
) -> AppResult<()> {
    sqlx::query!(
        r#"INSERT INTO flyers (user_id, x, y, image_url, event_url, event_name, event_end)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        user.id,
        flyer.x,
        flyer.y,
        flyer.image_url,
        flyer.event_url,
        flyer.event_name,
        flyer.event_end
    )
    .execute(&state.db)
    .await?;

    tracing::info!("Created flyer!");

    Ok(())
}

async fn update_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(update): Json<FlyerForm>,
) -> AppResult<()> {
    if user.has_role(&state.db, User::ADMIN).await? {
        sqlx::query!(
            r#"UPDATE flyers
               SET image_url = ?, event_name = ?, event_url = ?, event_end = ?
               WHERE id = ?"#,
            update.image_url,
            update.event_name,
            update.event_url,
            update.event_end,
            id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            r#"UPDATE flyers
               SET image_url = ?, event_name = ?, event_url = ?, event_end = ?
               WHERE id = ? AND user_id = ?"#,
            update.image_url,
            update.event_name,
            update.event_url,
            update.event_end,
            id,
            user.id
        )
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

async fn move_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(update): Json<Flyer>,
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

async fn bulletin_page(
    user: Option<User>,
    State(state): State<SharedAppState>,
) -> AppResult<impl IntoResponse> {
    let flyers = sqlx::query_as!(
        Flyer,
        "SELECT id, x, y, rotation, z_index, image_url, event_url, event_end FROM flyers"
    )
    .fetch_all(&state.db)
    .await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "bulletin/index.html")]
    struct Html {
        flyers: Vec<Flyer>,
        user: Option<User>,
    }

    Ok(Html { flyers, user })
}

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/bulletin", get(bulletin_page))
            .route("/bulletin/flyer/new", post(create_flyer))
            .route("/bulletin/flyer/{id}", put(update_flyer).delete(delete_flyer))
            .route("/bulletin/flyer/{id}/move", post(move_flyer))
    })
}

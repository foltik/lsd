use axum::routing::post;
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

async fn create_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(flyer): Json<Flyer>,
) -> AppResult<()> {
    todo!()
}

async fn update_flyer(
    user: User,
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    Json(update): Json<Flyer>,
) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE flyers
           SET x = ?, y = ?, rotation = ?
           WHERE id = ?"#,
        update.x,
        update.y,
        update.rotation,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn delete_flyer(user: User, State(state): State<SharedAppState>, Path(id): Path<i64>) -> AppResult<()> {
    sqlx::query!("DELETE FROM flyers WHERE id = ?", id).execute(&state.db).await?;

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
    }

    Ok(Html { flyers })
}

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/bulletin", get(bulletin_page)).route(
            "/bulletin/flyer/{id}",
            post(create_flyer).put(update_flyer).delete(delete_flyer),
        )
    })
}

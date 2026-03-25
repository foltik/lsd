use axum::Json;
use axum::extract::Query;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::utils::error::not_found;

#[derive(Debug, Serialize, Deserialize)]
struct Flyer {
    id: i64,
    user_id: i64,
    flyer_name: Option<String>,
    x: i64,
    y: i64,
    rotation: i64,
    link_url: Option<String>,
    remove_after_time: NaiveDateTime,
}

async fn create_flyer(
    user: User, State(state): State<SharedAppState>, mut multipart: axum::extract::Multipart,
) -> HtmlResult {
    let mut image_data: Option<Vec<u8>> = None;
    let mut link_url: Option<String> = None;
    let mut flyer_name: Option<String> = None;
    let mut remove_after_time: Option<NaiveDateTime> = None;
    let mut x: Option<i64> = None;
    let mut y: Option<i64> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("") {
            "image_file" => {
                let data = field.bytes().await?;
                if !data.is_empty() {
                    let img = crate::utils::image::decode(&data).await?;
                    image_data = Some(crate::utils::image::encode_jpeg(&img, Some(1200)).await);
                }
            }
            "link_url" => {
                let v = field.text().await?;
                link_url = if v.is_empty() { None } else { Some(v) };
            }
            "flyer_name" => {
                flyer_name = Some(field.text().await?);
            }
            "remove_after_time" => {
                let v = field.text().await?;
                remove_after_time = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .or_else(|| NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M").ok());
            }
            "x" => {
                x = field.text().await?.parse().ok();
            }
            "y" => {
                y = field.text().await?.parse().ok();
            }
            _ => {}
        }
    }

    let image_data = image_data.ok_or_else(|| crate::utils::error::invalid())?;
    let flyer_name = flyer_name.unwrap_or_default();
    let remove_after_time = remove_after_time.ok_or_else(|| crate::utils::error::invalid())?;
    let x = x.unwrap_or(0);
    let y = y.unwrap_or(0);

    sqlx::query!(
        r#"INSERT INTO flyers (user_id, x, y, image_data, link_url, flyer_name, remove_after_time)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        user.id,
        x,
        y,
        image_data,
        link_url,
        flyer_name,
        remove_after_time
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/bulletin").into_response())
}

#[derive(Debug, Serialize, Deserialize)]
struct FlyerUpdate {
    link_url: Option<String>,
    flyer_name: String,
    remove_after_time: NaiveDateTime,
}

async fn update_flyer(
    user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    mut multipart: axum::extract::Multipart,
) -> HtmlResult {
    let mut image_data: Option<Vec<u8>> = None;
    let mut link_url: Option<String> = None;
    let mut flyer_name: Option<String> = None;
    let mut remove_after_time: Option<NaiveDateTime> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("") {
            "image_file" => {
                let data = field.bytes().await?;
                if !data.is_empty() {
                    let img = crate::utils::image::decode(&data).await?;
                    image_data = Some(crate::utils::image::encode_jpeg(&img, Some(1200)).await);
                }
            }
            "link_url" => {
                let v = field.text().await?;
                link_url = if v.is_empty() { None } else { Some(v) };
            }
            "flyer_name" => {
                flyer_name = Some(field.text().await?);
            }
            "remove_after_time" => {
                let v = field.text().await?;
                remove_after_time = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .or_else(|| NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M").ok());
            }
            _ => {}
        }
    }

    let flyer_name = flyer_name.unwrap_or_default();
    let remove_after_time = remove_after_time.ok_or_else(|| crate::utils::error::invalid())?;

    if user.has_role(User::ADMIN) {
        if let Some(data) = image_data {
            sqlx::query!(
                "UPDATE flyers SET image_data = ?, link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ?",
                data, link_url, flyer_name, remove_after_time, id,
            )
            .execute(&state.db)
            .await?;
        } else {
            sqlx::query!(
                "UPDATE flyers SET link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ?",
                link_url,
                flyer_name,
                remove_after_time,
                id,
            )
            .execute(&state.db)
            .await?;
        }
    } else if let Some(data) = image_data {
        sqlx::query!(
            "UPDATE flyers SET image_data = ?, link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ? AND user_id = ?",
            data, link_url, flyer_name, remove_after_time, id, user.id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            "UPDATE flyers SET link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ? AND user_id = ?",
            link_url, flyer_name, remove_after_time, id, user.id,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(Redirect::to("/bulletin").into_response())
}

async fn admin_update_flyer(
    user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
    mut multipart: axum::extract::Multipart,
) -> HtmlResult {
    let _ = user;
    let mut image_data: Option<Vec<u8>> = None;
    let mut link_url: Option<String> = None;
    let mut flyer_name: Option<String> = None;
    let mut remove_after_time: Option<NaiveDateTime> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("") {
            "image_file" => {
                let data = field.bytes().await?;
                if !data.is_empty() {
                    let img = crate::utils::image::decode(&data).await?;
                    image_data = Some(crate::utils::image::encode_jpeg(&img, Some(1200)).await);
                }
            }
            "link_url" => {
                let v = field.text().await?;
                link_url = if v.is_empty() { None } else { Some(v) };
            }
            "flyer_name" => {
                flyer_name = Some(field.text().await?);
            }
            "remove_after_time" => {
                let v = field.text().await?;
                remove_after_time = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .or_else(|| NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M").ok());
            }
            _ => {}
        }
    }

    let flyer_name = flyer_name.unwrap_or_default();
    let remove_after_time = remove_after_time.ok_or_else(|| crate::utils::error::invalid())?;

    if let Some(data) = image_data {
        sqlx::query!(
            "UPDATE flyers SET image_data = ?, link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ?",
            data, link_url, flyer_name, remove_after_time, id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            "UPDATE flyers SET link_url = ?, flyer_name = ?, remove_after_time = ? WHERE id = ?",
            link_url,
            flyer_name,
            remove_after_time,
            id,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(Redirect::to("/bulletin/admin").into_response())
}

#[derive(Debug, Serialize, Deserialize)]
struct FlyerMoveUpdate {
    x: i64,
    y: i64,
    rotation: i64,
}

async fn move_flyer(
    user: User, State(state): State<SharedAppState>, Path(id): Path<i64>, Json(update): Json<FlyerMoveUpdate>,
) -> JsonResult<()> {
    if user.has_role(User::ADMIN) {
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

    Ok(Json(()))
}

async fn delete_flyer(
    user: User, State(state): State<SharedAppState>, Path(id): Path<i64>,
) -> JsonResult<()> {
    if user.has_role(User::ADMIN) {
        sqlx::query!("DELETE FROM flyers WHERE id = ?", id).execute(&state.db).await?;
    } else {
        sqlx::query!("DELETE FROM flyers WHERE id = ? AND user_id = ?", id, user.id)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(()))
}

async fn flyer_details(State(state): State<SharedAppState>, Path(id): Path<i64>) -> JsonResult<FlyerUpdate> {
    let flyer = sqlx::query_as!(
        FlyerUpdate,
        "SELECT flyer_name, link_url, remove_after_time FROM flyers WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(flyer))
}

async fn serve_flyer_image(State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
    let data = sqlx::query_scalar!("SELECT image_data FROM flyers WHERE id = ?", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(not_found)?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        data,
    )
        .into_response())
}

async fn bulletin_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    let flyers = sqlx::query_as!(
        Flyer,
        "SELECT id, user_id, flyer_name, x, y, rotation, link_url, remove_after_time FROM flyers"
    )
    .fetch_all(&state.db)
    .await?;

    let (editable_flyers, read_only_flyers) = if let Some(ref u) = user {
        let is_admin = u.has_role(User::ADMIN);
        if is_admin {
            (flyers, Vec::new())
        } else {
            flyers.into_iter().partition(|f| f.user_id == u.id)
        }
    } else {
        (Vec::new(), flyers)
    };

    #[derive(Template, WebTemplate)]
    #[template(path = "bulletin/index.html")]
    struct Html {
        read_only_flyers: Vec<Flyer>,
        editable_flyers: Vec<Flyer>,
        user: Option<User>,
    }

    Ok(Html { read_only_flyers, editable_flyers, user }.into_response())
}

#[derive(Deserialize)]
struct AdminListParams {
    page: Option<i64>,
}

const PAGE_SIZE: i64 = 25;

async fn admin_flyer_list(
    user: User, State(state): State<SharedAppState>, Query(params): Query<AdminListParams>,
) -> HtmlResult {
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let total = sqlx::query_scalar!("SELECT COUNT(*) FROM flyers").fetch_one(&state.db).await?;

    let flyers = sqlx::query_as!(
        Flyer,
        "SELECT id, user_id, flyer_name, x, y, rotation, link_url, remove_after_time
         FROM flyers ORDER BY id DESC LIMIT ? OFFSET ?",
        PAGE_SIZE,
        offset
    )
    .fetch_all(&state.db)
    .await?;

    let total_pages = (total + PAGE_SIZE - 1) / PAGE_SIZE;

    #[derive(Template, WebTemplate)]
    #[template(path = "bulletin/admin.html")]
    struct Html {
        user: Option<User>,
        flyers: Vec<Flyer>,
        page: i64,
        total_pages: i64,
    }

    Ok(Html { user: Some(user), flyers, page, total_pages }.into_response())
}

pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/bulletin", get(bulletin_page))
                .route("/bulletin/flyer/new", post(create_flyer))
                .route("/bulletin/flyer/{id}", delete(delete_flyer).get(flyer_details))
                .route("/bulletin/flyer/{id}/edit", post(update_flyer))
                .route("/bulletin/flyer/{id}/move", post(move_flyer))
                .route("/bulletin/flyer/{id}/image", get(serve_flyer_image))
        })
        .restricted_routes(User::ADMIN, |r| {
            r.route("/bulletin/admin", get(admin_flyer_list))
                .route("/bulletin/admin/flyer/{id}/edit", post(admin_update_flyer))
        })
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use chrono::Local;

use crate::utils::{
    db::Post,
    types::{AppResult, AppRouter, SharedAppState},
};

/// Add all `post` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/p/new", get(create_post_page))
        .route("/p/:post", get(view_post_page))
        .route("/p/:post/edit", get(edit_post_page).post(edit_post_form))
}

/// Display a single post.
async fn view_post_page(
    State(state): State<SharedAppState>,
    Path(post): Path<String>,
) -> AppResult<Response> {
    let Some(post) = state.db.lookup_post_by_url(&post).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let html = state.templates.render("post.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to create a new post.
async fn create_post_page(State(state): State<SharedAppState>) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert(
        "post",
        &Post {
            id: 0,
            title: "".into(),
            url: "".into(),
            author: "".into(),
            content: "".into(),
            created_at: Local::now(),
            updated_at: Local::now(),
        },
    );

    let html = state.templates.render("post-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to create a new post.
async fn edit_post_page(
    State(state): State<SharedAppState>,
    Path(post): Path<String>,
) -> AppResult<Response> {
    let Some(post) = state.db.lookup_post_by_url(&post).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let html = state.templates.render("post-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Process the form and create or edit a post.
async fn edit_post_form(
    State(state): State<SharedAppState>,
    Form(form): Form<EditPost>,
) -> AppResult<impl IntoResponse> {
    match form.id {
        None => {
            state
                .db
                .create_post(&form.title, &form.url, &form.author, &form.content)
                .await?;
        }
        Some(id) => {
            state
                .db
                .update_post(&id, &form.title, &form.url, &form.author, &form.content)
                .await?;
        }
    }
    Ok(Redirect::to(&format!("{}/p/{}", state.config.app.url, &form.url)))
}
#[derive(serde::Deserialize)]
struct EditPost {
    id: Option<String>,
    title: String,
    url: String,
    author: String,
    content: String,
}

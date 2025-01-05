use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use chrono::Local;

use crate::db::post::{Post, UpdatePost};
use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `post` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/p/new", get(create_post_page))
        .route("/p/:post", get(view_post_page))
        .route("/p/:post/edit", get(edit_post_page).post(edit_post_form))
}

/// Display a single post.
async fn view_post_page(State(state): State<SharedAppState>, Path(url): Path<String>) -> AppResult<Response> {
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
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
async fn edit_post_page(State(state): State<SharedAppState>, Path(url): Path<String>) -> AppResult<Response> {
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
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
        Some(id) => Post::update(&state.db, id, &form.post).await?,
        None => {
            Post::create(&state.db, &form.post).await?;
        }
    }
    Ok(Redirect::to(&format!("{}/p/{}", state.config.app.url, &form.post.url)))
}
#[derive(serde::Deserialize)]
struct EditPost {
    id: Option<i64>,
    #[serde(flatten)]
    post: UpdatePost,
}

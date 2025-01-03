use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};

use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `post` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/p/new", get(create_post_page).post(create_post_form))
        .route("/p/:post", get(view_post_page))
}

/// Display a single post.
async fn view_post_page(
    State(state): State<SharedAppState>,
    Path(post): Path<String>,
) -> AppResult<Response> {
    let Some(post) = state.db.lookup_post_by_slug(&post).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let html = state.templates.render("post.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to create a new post.
async fn create_post_page(State(state): State<SharedAppState>) -> AppResult<Response> {
    let ctx = tera::Context::new();
    let html = state.templates.render("post-create.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Process the form and create a new post.
async fn create_post_form(
    State(state): State<SharedAppState>,
    Form(form): Form<CreatePost>,
) -> AppResult<impl IntoResponse> {
    let _event_id = state.db.create_post(&form.title, &form.slug, &form.author, &form.body).await?;
    Ok(Redirect::to(&format!("{}/p/{}", state.config.app.url, form.slug)))
}
#[derive(serde::Deserialize)]
struct CreatePost {
    title: String,
    slug: String,
    author: String,
    body: String,
}

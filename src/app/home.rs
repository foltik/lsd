use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
    routing::get,
};

use crate::db::user::User;
use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `home` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router.route("/", get(home_page))
}

/// Display the front page.
async fn home_page(State(state): State<SharedAppState>, user: Option<User>) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("message", "Hello, world!");
    if let Some(user) = user {
        ctx.insert("user", &user);
    }

    let html = state.templates.render("home.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

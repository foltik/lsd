use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use axum_extra::extract::CookieJar;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `home` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router.route("/", get(home_page))
}

/// Display the front page.
async fn home_page(State(state): State<SharedAppState>, cookies: CookieJar) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert("message", "Hello, world!");

    if let Some(session_token) = cookies.get("session") {
        let Some(user) = state.db.lookup_user_from_session_token(session_token.value()).await? else {
            return Ok(StatusCode::FORBIDDEN.into_response());
        };
        ctx.insert("user", &user);
    }

    let html = state.templates.render("home.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

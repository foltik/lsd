use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
    routing::get,
};

use crate::utils::types::{AppResult, AppRouter, SharedAppState};
use crate::{db::user::User, views};

/// Add all `home` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router.route("/", get(home_page))
}

/// Display the front page.
async fn home_page(State(_state): State<SharedAppState>, user: Option<User>) -> AppResult<Response> {
    let index_template = views::index::IndexTemplate { user };

    Ok(Html(index_template.render()?).into_response())
}

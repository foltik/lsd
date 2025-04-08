use axum::{response::IntoResponse, routing::get};

use crate::{utils::types::AppRouter, views};

/// Add all `home` routes to the router.
pub fn routes() -> AppRouter {
    AppRouter::new().route("/", get(home_page))
}

/// Display the front page.
async fn home_page() -> impl IntoResponse {
    views::index::IndexTemplate
}

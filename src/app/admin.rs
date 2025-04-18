use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
};

use crate::utils::{
    error::AppResult,
    types::{AppRouter, SharedAppState},
};
use crate::{
    db::user::{ListUserQuery, User},
    views,
};

/// Add all `admins` routes to the router.
pub fn routes() -> AppRouter {
    AppRouter::new().route("/dashboard", get(admin_dashboard))
}

/// Display admin dashboard
async fn admin_dashboard(
    State(state): State<SharedAppState>,
    Query(query): Query<ListUserQuery>,
) -> AppResult<impl IntoResponse> {
    let users = User::list(&state.db, &query).await?;

    Ok(views::admin::AdminDashboard { users })
}

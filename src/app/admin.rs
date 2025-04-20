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
    AppRouter::new()
        .route("/dashboard/overview", get(admin_overview_dashboard))
        .route("/dashboard/users", get(admin_users_dashboard))
}

/// Display admin overview dashboard
async fn admin_overview_dashboard(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    let q = ListUserQuery { page: 0, page_size: 25 };

    let users_count = User::list(&state.db, &q).await?.len();

    Ok(views::admin::AdminDashboardOverview { users_count })
}

//
async fn admin_users_dashboard(
    State(state): State<SharedAppState>,
    Query(query): Query<ListUserQuery>,
) -> AppResult<impl IntoResponse> {
    let users = User::list(&state.db, &query).await?;

    Ok(views::admin::AdminDashboardUsersView { users })
}

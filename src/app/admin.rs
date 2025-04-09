use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
};

use crate::utils::types::{AppResult, AppRouter, SharedAppState};
use crate::{
    db::user::{ListUserQuery, User},
    views,
};

/// Add all `admins` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router.route("/admin/dashboard", get(admin_dashboard))
}

/// Display admin dashboard
async fn admin_dashboard(
    State(state): State<SharedAppState>,
    Query(query): Query<ListUserQuery>,
) -> AppResult<Response> {
    let users = User::list(&state.db, &query).await?;

    let dashboard_template = views::admin::AdminDashboard { users };

    Ok(Html(dashboard_template.render()?).into_response())
}

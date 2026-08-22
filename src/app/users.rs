use crate::db::user::{UserSearchBy, UserSearchResult};
use crate::prelude::*;

/// Add all `users` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/users/search", get(search))
    })
}

// Fuzzy user search, mainly for autocomplete.
#[derive(serde::Deserialize)]
struct UserSearchQuery {
    q: String,
    by: Option<UserSearchBy>,
    event_id: Option<i64>,
}
async fn search(
    State(state): State<SharedAppState>, Query(query): Query<UserSearchQuery>,
) -> JsonResult<Vec<UserSearchResult>> {
    Ok(Json(User::search(&state.db, &query.q, query.by, query.event_id).await?))
}

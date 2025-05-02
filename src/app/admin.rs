use crate::db::user::{ListUserQuery, User, UserView};
use crate::prelude::*;

/// Add all `admins` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/admin/dashboard/overview", get(view::overview))
            .route("/admin/dashboard/users", get(view::users))
    })
}

mod view {
    use super::*;
    /// Display admin overview dashboard
    pub async fn overview(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        let q = ListUserQuery { page: 0, page_size: 25 };

        let users_count = User::list(&state.db, &q).await?.len();

        #[derive(Template, WebTemplate)]
        #[template(path = "admin/dashboard/overview.html")]
        pub struct Html {
            pub users_count: usize,
        }
        Ok(Html { users_count })
    }

    // Display table of users dashboard
    pub async fn users(
        State(state): State<SharedAppState>,
        Query(query): Query<ListUserQuery>,
    ) -> AppResult<impl IntoResponse> {
        let users = User::list(&state.db, &query).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "admin/dashboard/users.html")]
        pub struct Html {
            pub users: Vec<UserView>,
        }
        Ok(Html { users })
    }
}

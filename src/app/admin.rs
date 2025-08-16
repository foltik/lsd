use crate::db::user::{ListUserQuery, User, UserView};
use crate::prelude::*;

/// Add all `admins` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| {
        r.route("/admin/dashboard/overview", get(view::overview))
            .route("/admin/dashboard/users", get(view::users))
            .route("/admin/action/users/{url}/changeRole", post(action::change_user_role))
            .route("/admin/action/users/{url}/remove", delete(action::remove_user))
    })
}

mod action {

    use super::*;

    // Change user role
    #[derive(serde::Deserialize)]
    pub struct ActionQuery {
        action: String,
        role: String,
    }

    // Change user's add/remove user role
    pub async fn change_user_role(
        State(state): State<SharedAppState>,
        Path(url): Path<i64>,
        Query(query): Query<ActionQuery>,
        user: User,
    ) -> AppResult<impl IntoResponse> {
        if user.has_role(&state.db, "admin").await? {
            let Some(_) = User::lookup_by_id(&state.db, url).await? else {
                return Err(AppError::NotFound);
            };

            match query.action.as_str() {
                "remove" => {
                    User::remove_role(&state.db, url, &query.role).await?;
                    Ok(())
                }
                "add" => {
                    User::add_role(&state.db, url, &query.role).await?;
                    Ok(())
                }
                _ => Err(AppError::NotAuthorized),
            }
        } else {
            Err(AppError::NotAuthorized)
        }
    }

    pub async fn remove_user(
        State(state): State<SharedAppState>,
        Path(url): Path<i64>,
        user: User,
    ) -> AppResult<impl IntoResponse> {
        if user.has_role(&state.db, "admin").await? {
            let Some(_) = User::lookup_by_id(&state.db, url).await? else {
                return Err(AppError::NotFound);
            };
            User::remove(&state.db, url).await?;
            Ok(())
        } else {
            Err(AppError::NotAuthorized)
        }
    }
}

mod view {
    use super::*;
    /// Display admin overview dashboard
    pub async fn overview(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        let q = ListUserQuery { page: 0, page_size: 25 };

        let users_count = User::list(&state.db, &q).await?.users.len();

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
        let query_result = User::list(&state.db, &query).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "admin/dashboard/users.html")]
        pub struct Html {
            pub users: Vec<UserView>,
            pub has_next_page: bool,
        }
        Ok(Html { users: query_result.users, has_next_page: query_result.has_next_page })
    }
}

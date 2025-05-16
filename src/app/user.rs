use askama::Template;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Form;

use crate::db::user::{UpdateUser, User};
use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/user/edit", get(user_profile).post(edit_user)))
}

async fn user_profile(user: User) -> impl IntoResponse {
    #[derive(Template, WebTemplate)]
    #[template(path = "user/edit.html")]
    struct Html {
        user: Option<User>,
    }
    Html { user: Some(user) }
}

// TODO: Confirm email change (here and in register API)
// TODO: validate email address regex (client-side and server-side)
async fn edit_user(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<UpdateUser>,
) -> AppResult<impl IntoResponse> {
    user.update(&state.db, &form).await?;

    Ok(Redirect::to("/user/edit"))
}

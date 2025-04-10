use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Form,
};

use crate::{
    db::user::{UpdateUser, User},
    utils::types::{AppResult, AppRouter, SharedAppState},
    views,
};

pub fn register_routes(router: AppRouter) -> AppRouter {
    router.route("/user/edit", get(user_profile).post(edit_user))
}

async fn user_profile(user: User) -> AppResult<impl IntoResponse> {
    Ok(Html(views::user::UserProfile { user: Some(user) }.render()?))
}

async fn edit_user(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<UpdateUser>,
) -> AppResult<impl IntoResponse> {
    user.update(&state.db, &form).await?;

    Ok(Redirect::to("/user/edit"))
}

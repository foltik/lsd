use anyhow::anyhow;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get},
    Form,
};
use chrono::Utc;
use lettre::message::Mailbox;

use crate::{
    db::list::{List, UpdateList},
    utils::types::AppError,
    views,
};
use crate::{
    db::user::User,
    utils::types::{AppResult, AppRouter, SharedAppState},
};

/// Add all `lists` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/lists", get(list_lists_page))
        .route("/lists/new", get(create_list_page))
        .route("/lists/{id}", get(edit_list_page).post(edit_list_form))
        .route("/lists/{id}/{email}", delete(remove_list_member))
}

/// Display a list of all lists
async fn list_lists_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let lists = List::list(&state.db).await?;

    let list_template = views::lists::Lists { lists };

    Ok(Html(list_template.render()?).into_response())
}

/// Display the form to view and edit a list.
async fn edit_list_page(
    State(state): State<SharedAppState>,
    user: User,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let Some(list) = List::lookup_by_id(&state.db, id).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let members = List::list_members(&state.db, id).await?;

    let edit_template = views::lists::ListEdit { list, members };

    Ok(Html(edit_template.render()?).into_response())
}

/// Display the form to create a new list.
async fn create_list_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let create_template = views::lists::ListEdit {
        list: List {
            id: 0,
            name: "".into(),
            description: "".into(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
        members: Vec::new(),
    };

    Ok(Html(create_template.render()?).into_response())
}

/// Process the form and create or edit a list.
async fn edit_list_form(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<UpdateList>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let id = match form.id {
        Some(id) => {
            List::update(&state.db, id, &form).await?;
            id
        }
        None => List::create(&state.db, &form).await?,
    };

    let emails = form.emails.split_whitespace().collect::<Vec<_>>();
    for email in &emails {
        if let Err(e) = email.parse::<Mailbox>() {
            return Err(AppError(anyhow!("email {email:?} is invalid: {e}")));
        }
    }
    if !emails.is_empty() {
        List::add_members(&state.db, id, &emails).await?;
    }

    Ok(Redirect::to(&format!("{}/lists/{}", state.config.app.url, id)).into_response())
}

async fn remove_list_member(
    State(state): State<SharedAppState>,
    user: User,
    Path((id, email)): Path<(i64, String)>,
) -> AppResult<StatusCode> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN);
    }

    List::remove_member(&state.db, id, &email).await?;
    Ok(StatusCode::OK)
}

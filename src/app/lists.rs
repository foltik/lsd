use anyhow::anyhow;
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
    db::list::{CreateList, List, UpdateList},
    utils::types::AppError,
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

    let mut ctx = tera::Context::new();
    ctx.insert("lists", &lists);

    let html = state.templates.render("lists.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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

    let mut ctx = tera::Context::new();
    ctx.insert("list", &list);
    ctx.insert("members", &members);

    let html = state.templates.render("list-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to create a new list.
async fn create_list_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let mut ctx = tera::Context::new();
    ctx.insert(
        "list",
        &List {
            id: 0,
            name: "".into(),
            description: "".into(),
            created_at: Utc::now(),
            updated_at: None,
        },
    );
    ctx.insert::<[String], _>("members", &[]);

    let html = state.templates.render("list-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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
            let Some(list) = List::lookup(&state.db, id).await? else {
                return Ok(StatusCode::NOT_FOUND.into_response());
            };
            list.update(&state.db, &form.name, &form.description).await?;
            id
        }
        None => {
            let create = CreateList {
                name: form.name.clone(),
                description: form.description.clone(),
            };
            List::create(&state.db, &create).await?
        }
    };

    let emails = form.emails.split_whitespace().collect::<Vec<_>>();
    let mut user_ids = Vec::new();
    for email in &emails {
        if let Err(e) = email.parse::<Mailbox>() {
            return Err(AppError(anyhow!("email {email:?} is invalid: {e}")));
        }
        let Some(user) = User::lookup_by_email(&state.db, email).await? else {
            return Err(AppError(anyhow!("user with email {email:?} not found")));
        };
        user_ids.push(user.id);
    }
    if !user_ids.is_empty() {
        List::add_members(&state.db, id, &user_ids).await?;
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

    let Some(member) = User::lookup_by_email(&state.db, &email).await? else {
        return Ok(StatusCode::NOT_FOUND);
    };
    List::remove_member(&state.db, id, member.id).await?;
    Ok(StatusCode::OK)
}

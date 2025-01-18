use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get},
    Form,
};
use chrono::Utc;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};
use crate::{
    db::list::{List, UpdateList},
    utils::types::AppError,
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
async fn list_lists_page(State(state): State<SharedAppState>) -> AppResult<Response> {
    let lists = List::list(&state.db).await?;

    let mut ctx = tera::Context::new();
    ctx.insert("lists", &lists);

    let html = state.templates.render("lists.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to view and edit a list.
async fn edit_list_page(State(state): State<SharedAppState>, Path(id): Path<i64>) -> AppResult<Response> {
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
async fn create_list_page(State(state): State<SharedAppState>) -> AppResult<Response> {
    let mut ctx = tera::Context::new();
    ctx.insert(
        "list",
        &List {
            id: 0,
            name: "".into(),
            description: "".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    );
    ctx.insert::<[String], _>("members", &[]);

    let html = state.templates.render("list-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Process the form and create or edit a list.
async fn edit_list_form(
    State(state): State<SharedAppState>,
    Form(form): Form<UpdateList>,
) -> AppResult<impl IntoResponse> {
    let id = match form.id {
        Some(id) => {
            List::update(&state.db, id, &form).await?;
            id
        }
        None => List::create(&state.db, &form).await?,
    };

    let emails = form.emails.split_whitespace().collect::<Vec<_>>();
    for email in &emails {
        if !email.contains('@') && !email.contains('.') {
            return Err(AppError(anyhow!("email {email:?} is not in the format \"mailbox@domain.tld\"")));
        }
    }
    if !emails.is_empty() {
        List::add_members(&state.db, id, &emails).await?;
    }

    Ok(Redirect::to(&format!("{}/lists/{}", state.config.app.url, id)))
}

async fn remove_list_member(
    State(state): State<SharedAppState>,
    Path((id, email)): Path<(i64, String)>,
) -> AppResult<impl IntoResponse> {
    List::remove_member(&state.db, id, &email).await?;
    Ok(StatusCode::OK)
}

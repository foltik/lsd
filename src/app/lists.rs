use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get},
    Form,
};
use chrono::Utc;
use lettre::message::Mailbox;

use crate::{
    db::{
        list::{List, UpdateList},
        user::User,
    },
    utils::{
        error::{AppError, AppResult},
        types::{AppRouter, SharedAppState},
    },
    views,
};

/// Add all `lists` routes to the router.
pub fn routes() -> AppRouter {
    AppRouter::new()
        .route("/", get(list_lists_page))
        .route("/new", get(create_list_page))
        .route("/{id}", get(edit_list_page).post(edit_list_form))
        .route("/{id}/signup", get(signup_page).post(signup_form))
        .route("/{id}/{email}", delete(remove_list_member))
}

/// Display a list of all lists
async fn list_lists_page(State(state): State<SharedAppState>, user: User) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Err(AppError::NotAuthorized);
    }

    let lists = List::list(&state.db).await?;

    Ok(views::lists::Lists { lists })
}

/// Display the form to view and edit a list.
async fn edit_list_page(
    State(state): State<SharedAppState>,
    user: User,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Err(AppError::NotAuthorized);
    }

    let list = List::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    let members = List::list_members(&state.db, id).await?;

    Ok(views::lists::ListEdit { list, members })
}

/// Display the form to create a new list.
async fn create_list_page(State(state): State<SharedAppState>, user: User) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Err(AppError::NotAuthorized);
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

    Ok(create_template)
}

/// Process the form and create or edit a list.
async fn edit_list_form(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<UpdateList>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Err(AppError::NotAuthorized);
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
            tracing::debug!("Invalid email: {e}");
            return Ok(StatusCode::BAD_REQUEST.into_response());
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
) -> AppResult<()> {
    if !user.has_role(&state.db, User::ADMIN).await? {
        return Err(AppError::NotAuthorized);
    }

    List::remove_member(&state.db, id, &email).await?;
    Ok(())
}

/// Display the newsletter signup page.
// XXX: Hard coded to list with id=1.
pub async fn newsletter_signup_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    signup_page(State(state), Path(1)).await
}

/// Display the list signup page.
async fn signup_page(
    State(state): State<SharedAppState>,
    Path(list_id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    // XXX: Hard code only allow id 1 to be signed up to.
    // A flag should be added to List whether it's public or not, and what the signup page looks like.
    if list_id != 1 {
        return Err(AppError::NotAuthorized);
    }

    let Some(list) = List::lookup_by_id(&state.db, list_id).await? else {
        return Err(AppError::NotFound);
    };

    Ok(views::lists::Signup { list })
}

/// Process the list signup form.
//
// XXX: We really should rate limit this.
async fn signup_form(
    State(state): State<SharedAppState>,
    Form(form): Form<NewsletterForm>,
) -> AppResult<impl IntoResponse> {
    // XXX: Hard code only allow id 1 to be signed up to.
    // A flag should be added to List whether it's public or not, and what the signup page looks like.
    if form.list_id != 1 {
        return Err(AppError::NotAuthorized);
    }

    let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
        return Err(AppError::NotFound);
    };
    List::add_members(&state.db, list.id, &[form.email.email.as_ref()]).await?;

    Ok(views::lists::Confirmation { list, email: state.config.email.from.email.to_string() })
}
#[derive(serde::Deserialize)]
struct NewsletterForm {
    list_id: i64,
    email: Mailbox,
}

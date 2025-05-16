use lettre::message::Mailbox;

use crate::db::list::{List, ListMember, UpdateList};
use crate::prelude::*;

/// Add all `lists` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/newsletter", get(newsletter_signup_page))
         .route("/lists/{id}/signup", get(signup_page).post(signup_form))
         .route("/lists/confirm", get(signup_confirmation_page))
    })
    .restricted_routes(User::ADMIN, |r| {
        r.route("/lists", get(list_lists_page))
         .route("/lists/new", get(create_list_page))
         .route("/lists/{id}", get(edit_list_page).post(edit_list_form))
         .route("/lists/{id}/{email}", delete(remove_list_member))
    })
}

#[derive(Template, WebTemplate)]
#[template(path = "lists/edit.html")]
struct ListEditHtml {
    list: List,
    members: Vec<ListMember>,
    user: Option<User>,
}

/// Display a list of all lists
async fn list_lists_page(
    State(state): State<SharedAppState>,
    user: Option<User>,
) -> AppResult<impl IntoResponse> {
    let lists = List::list(&state.db).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/view.html")]
    struct Html {
        lists: Vec<List>,
        user: Option<User>,
    }
    Ok(Html { lists, user })
}

/// Display the form to view and edit a list.
async fn edit_list_page(
    State(state): State<SharedAppState>,
    Path(id): Path<i64>,
    user: Option<User>,
) -> AppResult<impl IntoResponse> {
    let list = List::lookup_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;

    let members = List::list_members(&state.db, id).await?;

    Ok(ListEditHtml { list, members, user })
}

/// Display the form to create a new list.
async fn create_list_page(user: Option<User>) -> AppResult<impl IntoResponse> {
    let list = List {
        id: 0,
        name: "".into(),
        description: "".into(),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };

    Ok(ListEditHtml { list, members: vec![], user })
}

/// Process the form and create or edit a list.
async fn edit_list_form(
    State(state): State<SharedAppState>,
    Form(form): Form<UpdateList>,
) -> AppResult<Response> {
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
    Path((id, email)): Path<(i64, String)>,
) -> AppResult<()> {
    List::remove_member(&state.db, id, &email).await?;
    Ok(())
}

/// Display the newsletter signup page.
// XXX: Hard coded to list with id=1.
pub async fn newsletter_signup_page(
    State(state): State<SharedAppState>,
    user: Option<User>,
) -> AppResult<impl IntoResponse> {
    signup_page(State(state), Path(1), user).await
}

/// Display the list signup page.
async fn signup_page(
    State(state): State<SharedAppState>,
    Path(list_id): Path<i64>,
    user: Option<User>,
) -> AppResult<impl IntoResponse> {
    // XXX: Hard code only allow id 1 to be signed up to.
    // A flag should be added to List whether it's public or not, and what the signup page looks like.
    if list_id != 1 {
        return Err(AppError::NotAuthorized);
    }

    let Some(list) = List::lookup_by_id(&state.db, list_id).await? else {
        return Err(AppError::NotFound);
    };

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/signup.html")]
    struct Html {
        list: List,
        user: Option<User>,
    }
    Ok(Html { list, user })
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

    Ok(Redirect::to(&format!("/lists/confirmation?list_id={}", list.id)))
}
#[derive(serde::Deserialize)]
struct NewsletterForm {
    list_id: i64,
    email: Mailbox,
}

async fn signup_confirmation_page(
    State(state): State<SharedAppState>,
    Query(list_id): Query<i64>,
    user: Option<User>,
) -> AppResult<impl IntoResponse> {
    let Some(list) = List::lookup_by_id(&state.db, list_id).await? else {
        return Err(AppError::NotFound);
    };

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/confirmation.html")]
    struct Html {
        list: List,
        email: String,
        user: Option<User>,
    }
    Ok(Html { list, email: state.config.email.from.email.to_string(), user })
}

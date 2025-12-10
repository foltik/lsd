use lettre::message::Mailbox;

use crate::db::list::{List, UpdateList};
use crate::prelude::*;

/// Add all `lists` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/newsletter", get(newsletter_signup_page))
         .route("/lists/{id}/signup", get(signup_page).post(signup_form))
    })
    .restricted_routes(User::ADMIN, |r| {
        r.route("/lists", get(list_lists_page))
         .route("/lists/new", get(create_list_page))
         .route("/lists/{id}", get(edit_list_page).post(edit_list_form))
         .route("/lists/{id}/{user_id}", delete(remove_list_member))
    })
}

#[derive(Template, WebTemplate)]
#[template(path = "lists/edit.html")]
struct ListEditHtml {
    user: Option<User>,
    list: List,
    members: Vec<User>,
}

/// Display a list of all lists
async fn list_lists_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
    let lists = List::list(&state.db).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/list.html")]
    struct Html {
        user: Option<User>,
        lists: Vec<List>,
    }
    Ok(Html { user: Some(user), lists }.into_response())
}

/// Display the form to view and edit a list.
async fn edit_list_page(user: User, State(state): State<SharedAppState>, Path(id): Path<i64>) -> HtmlResult {
    let list = List::lookup_by_id(&state.db, id).await?.ok_or_else(not_found)?;

    let members = List::list_members(&state.db, id).await?;

    Ok(ListEditHtml { user: Some(user), list, members }.into_response())
}

/// Display the form to create a new list.
async fn create_list_page(user: User) -> HtmlResult {
    let list = List {
        id: 0,
        name: "".into(),
        description: "".into(),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };

    Ok(ListEditHtml { user: Some(user), list, members: vec![] }.into_response())
}

/// Process the form and create or edit a list.
async fn edit_list_form(State(state): State<SharedAppState>, Form(form): Form<UpdateList>) -> HtmlResult {
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

    Ok(Redirect::to("/lists").into_response())
}

async fn remove_list_member(
    State(state): State<SharedAppState>, Path((id, user_id)): Path<(i64, i64)>,
) -> JsonResult<()> {
    List::remove_member(&state.db, id, user_id).await?;
    Ok(Json(()))
}

/// Display the newsletter signup page.
// XXX: Hard coded to list with id=1.
pub async fn newsletter_signup_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    signup_page(user, State(state), Path(1)).await
}

/// Display the list signup page.
async fn signup_page(
    user: Option<User>, State(state): State<SharedAppState>, Path(list_id): Path<i64>,
) -> HtmlResult {
    // XXX: Hard code only allow id 1 to be signed up to.
    // A flag should be added to List whether it's public or not, and what the signup page looks like.
    if list_id != 1 {
        bail_unauthorized!()
    }

    let Some(list) = List::lookup_by_id(&state.db, list_id).await? else {
        bail_not_found!();
    };

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/signup.html")]
    struct Html {
        user: Option<User>,
        list: List,
    }
    Ok(Html { user, list }.into_response())
}

/// Process the list signup form.
//
// XXX: We really should rate limit this.
async fn signup_form(
    user: Option<User>, State(state): State<SharedAppState>, Form(form): Form<NewsletterForm>,
) -> HtmlResult {
    // XXX: Hard code only allow id 1 to be signed up to.
    // A flag should be added to List whether it's public or not, and what the signup page looks like.
    if form.list_id != 1 {
        bail_unauthorized!()
    }

    let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
        bail_not_found!();
    };
    List::add_members(&state.db, list.id, &[form.email.email.as_ref()]).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "lists/confirmation.html")]
    struct Html {
        user: Option<User>,
        list: List,
        email: String,
    }
    Ok(Html { user, list, email: state.config.email.from.email.to_string() }.into_response())
}
#[derive(serde::Deserialize)]
struct NewsletterForm {
    list_id: i64,
    email: Mailbox,
}

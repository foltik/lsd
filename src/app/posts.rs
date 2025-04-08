use std::{collections::HashMap, time::Duration};

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Form,
};
use chrono::Utc;
use lettre::message::header::ContentType;
use tokio::time::sleep;

use crate::{
    db::{
        email::Email,
        list::{List, ListMember},
        post::{Post, UpdatePost},
        user::User,
    },
    utils::{
        error::{AppError, AppResult},
        types::{AppRouter, SharedAppState},
    },
    views,
};

/// Add all `post` routes to the router.
pub fn routes() -> AppRouter {
    AppRouter::new()
        .route("/", get(list_posts_page))
        .route("/new", get(create_post_page))
        .route("/{url}", get(view_post_page))
        .route("/{url}/edit", get(edit_post_page).post(edit_post_form))
        .route("/{url}/send", get(send_post_page).post(send_post_form))
        .route("/{url}/delete", post(delete_post_form))
}

/// Display a list of posts.
async fn list_posts_page(State(state): State<SharedAppState>, user: User) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        tracing::info!("no writer role");
        return Err(AppError::NotAuthorized);
    }

    let posts = Post::list(&state.db).await?;

    Ok(views::posts::PostList { posts })
}

/// Display a single post.
pub async fn view_post_page(
    State(state): State<SharedAppState>,
    Path(url): Path<String>,
) -> AppResult<impl IntoResponse> {
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Err(AppError::NotFound);
    };

    Ok(views::posts::PostView { post })
}

/// Display the form to create a new post.
async fn create_post_page(State(state): State<SharedAppState>, user: User) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }

    let create_template = views::posts::PostEdit {
        post: Post {
            id: 0,
            title: "".into(),
            url: "".into(),
            author: "".into(),
            content: "".into(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
    };

    Ok(create_template)
}

/// Display the form to create a new post.
async fn edit_post_page(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Err(AppError::NotFound);
    };

    Ok(views::posts::PostEdit { post })
}

/// Process the form and create or edit a post.
async fn edit_post_form(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<EditPost>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }

    match form.id {
        Some(id) => Post::update(&state.db, id, &form.post).await?,
        None => {
            Post::create(&state.db, &form.post).await?;
        }
    }

    Ok(())
}
#[derive(serde::Deserialize)]
struct EditPost {
    id: Option<i64>,
    #[serde(flatten)]
    post: UpdatePost,
}

/// Display the form to send a post.
async fn send_post_page(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Err(AppError::NotFound);
    };
    let lists = List::list(&state.db).await?;

    Ok(views::posts::PostSend { post, lists })
}

/// Process the form and create or edit a post.
async fn send_post_form(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
    Form(form): Form<SendPost>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Err(AppError::NotFound);
    };
    let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
        return Err(AppError::NotFound);
    };
    let members = List::list_members(&state.db, form.list_id).await?;

    let mut email_template =
        views::posts::PostEmail { post: post.clone(), opened_url: "".into(), unsub_url: "".into() };

    let mut num_sent = 0;
    let mut num_skipped = 0;
    let mut errors = HashMap::new();
    let batch_size = state.config.email.ratelimit.unwrap_or(members.len());
    for members in members.chunks(batch_size) {
        for ListMember { email, .. } in members {
            // If this post was already sent to this address in this list, skip sending it again.
            if Email::lookup_post(&state.db, &email, post.id, list.id).await?.is_some() {
                num_skipped += 1;
                continue;
            }
            let email_id = Email::create_post(&state.db, email, post.id, list.id).await?;

            email_template.opened_url = format!("{}/emails/{email_id}/footer.gif", &state.config.app.url);
            email_template.unsub_url = format!("{}/emails/{email_id}/unsubscribe", &state.config.app.url);

            let msg = state
                .mailer
                .builder()
                .to(email.parse().unwrap())
                .subject(&post.title)
                .header(ContentType::TEXT_HTML)
                .body(email_template.render()?)
                .unwrap();

            match state.mailer.send(msg).await {
                Ok(_) => {
                    Email::mark_sent(&state.db, email_id).await?;
                    num_sent += 1;
                }
                Err(e) => {
                    let e = e.to_string();
                    Email::mark_error(&state.db, email_id, &e).await?;
                    errors.insert(email.clone(), e);
                }
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    let sent_template = views::posts::PostSent {
        post_title: post.title,
        list_name: list.name,
        num_sent,
        num_skipped,
        errors,
    };

    Ok(sent_template)
}

#[derive(serde::Deserialize)]
struct SendPost {
    list_id: i64,
}

/// Process the form and create or edit a post.
async fn delete_post_form(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Err(AppError::NotAuthorized);
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Err(AppError::NotFound);
    };

    Post::delete(&state.db, post.id).await?;

    // Redirect to the list page.
    Ok(Redirect::to("/posts"))
}

use std::{collections::HashMap, time::Duration};

use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form,
};
use chrono::Utc;
use lettre::message::header::ContentType;
use tokio::time::sleep;

use crate::utils::types::{AppResult, AppRouter, SharedAppState};
use crate::{
    db::{
        email::Email,
        list::{List, ListMember},
        post::{Post, UpdatePost},
        user::User,
    },
    views,
};

/// Add all `post` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/posts", get(list_posts_page))
        .route("/posts/new", get(create_post_page))
        .route("/posts/{url}", get(view_post_page))
        .route("/posts/{url}/edit", get(edit_post_page).post(edit_post_form))
        .route("/posts/{url}/send", get(send_post_page).post(send_post_form))
        .route("/posts/{url}/delete", post(delete_post_form))
        .route("/p/{url}", get(view_post_page))
}

/// Display a list of posts.
async fn list_posts_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let posts = Post::list(&state.db).await?;

    let list_template = views::posts::PostList { user: Some(user), posts };

    Ok(Html(list_template.render()?).into_response())
}

/// Display a single post.
async fn view_post_page(
    State(state): State<SharedAppState>,
    Path(url): Path<String>,
    user: Option<User>,
) -> AppResult<Response> {
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let view_template = views::posts::PostView { user, post };
    Ok(Html(view_template.render()?).into_response())
}

/// Display the form to create a new post.
async fn create_post_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let create_template = views::posts::PostEdit {
        user: Some(user),
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

    Ok(Html(create_template.render()?).into_response())
}

/// Display the form to create a new post.
async fn edit_post_page(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let edit_template = views::posts::PostEdit { user: Some(user), post };

    Ok(Html(edit_template.render()?).into_response())
}

/// Process the form and create or edit a post.
async fn edit_post_form(
    State(state): State<SharedAppState>,
    user: User,
    Form(form): Form<EditPost>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    match form.id {
        Some(id) => Post::update(&state.db, id, &form.post).await?,
        None => {
            Post::create(&state.db, &form.post).await?;
        }
    }
    Ok(().into_response())
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
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let lists = List::list(&state.db).await?;

    let send_template = views::posts::PostSend { user: Some(user), post, lists };

    Ok(Html(send_template.render()?).into_response())
}

/// Process the form and create or edit a post.
async fn send_post_form(
    State(state): State<SharedAppState>,
    user: User,
    Path(url): Path<String>,
    Form(form): Form<SendPost>,
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
            if Email::lookup_post(&state.db, email, post.id, list.id).await?.is_some() {
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
        user: Some(user),
        post_title: post.title,
        list_name: list.name,
        num_sent,
        num_skipped,
        errors,
    };

    Ok(Html(sent_template.render()?).into_response())
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
) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    Post::delete(&state.db, post.id).await?;

    // Redirect to the list page.
    Ok(Redirect::to("/posts").into_response())
}

use std::{collections::HashMap, time::Duration};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Form,
};
use chrono::Utc;
use lettre::message::header::ContentType;
use tokio::time::sleep;

use crate::db::{
    email::Email,
    list::{List, ListMember},
    post::{Post, UpdatePost},
    user::User,
};
use crate::utils::types::{AppResult, AppRouter, SharedAppState};

/// Add all `post` routes to the router.
pub fn register_routes(router: AppRouter) -> AppRouter {
    router
        .route("/p/new", get(create_post_page))
        .route("/p/{url}", get(view_post_page))
        .route("/p/{url}/edit", get(edit_post_page).post(edit_post_form))
        .route("/p/{url}/send", get(send_post_page).post(send_post_form))
}

/// Display a single post.
async fn view_post_page(State(state): State<SharedAppState>, Path(url): Path<String>) -> AppResult<Response> {
    let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let html = state.templates.render("post.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}

/// Display the form to create a new post.
async fn create_post_page(State(state): State<SharedAppState>, user: User) -> AppResult<Response> {
    if !user.has_role(&state.db, User::WRITER).await? {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }

    let mut ctx = tera::Context::new();
    ctx.insert(
        "post",
        &Post {
            id: 0,
            title: "".into(),
            url: "".into(),
            author: "".into(),
            content: "".into(),
            content_rendered: "".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    );

    let html = state.templates.render("post-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let html = state.templates.render("post-edit.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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
    Ok(Redirect::to(&format!("{}/p/{}", state.config.app.url, &form.post.url)).into_response())
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

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);
    ctx.insert("lists", &lists);

    let html = state.templates.render("post-send.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
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

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);
    ctx.insert("post_url", &format!("{}/p/{}", &state.config.app.url, &post.url));

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

            ctx.insert("opened_url", &format!("{}/emails/{email_id}/footer.gif", &state.config.app.url));
            ctx.insert("unsub_url", &format!("{}/emails/{email_id}/unsubscribe", &state.config.app.url));
            let html = state.templates.render("post-email.tera.html", &ctx).unwrap();

            let msg = state
                .mailer
                .builder()
                .to(email.parse().unwrap())
                .subject(&post.title)
                .header(ContentType::TEXT_HTML)
                .body(html)
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

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);
    ctx.insert("list", &list);
    ctx.insert("stats", &Stats { num_sent, num_skipped, errors });

    let html = state.templates.render("post-sent.tera.html", &ctx).unwrap();
    Ok(Html(html).into_response())
}
#[derive(serde::Deserialize)]
struct SendPost {
    list_id: i64,
}
#[derive(serde::Serialize)]
struct Stats {
    pub num_sent: usize,
    pub num_skipped: usize,
    pub errors: HashMap<String, String>,
}

use crate::db::list::List;
use crate::db::post::{Post, UpdatePost};
use crate::prelude::*;

/// Add all `post` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/p/{url}", get(read::view))
             .route("/posts/{url}", get(read::view))
             .route("/posts/{url}/preview", get(read::preview))
        })
        .restricted_routes(User::WRITER, |r| {
            r.route("/posts", get(read::list))
             .route("/posts/new", get(edit::new))
             .route("/posts/{url}/edit", get(edit::edit_page).post(edit::edit_form))
             .route("/posts/{url}/send", get(send::page).post(send::form))
             .route("/posts/{url}/delete", post(edit::delete_form))
        })
}

/// View and list posts.
mod read {
    use super::*;

    // Display a list of posts.
    pub async fn list(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        let posts = Post::list(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/list.html")]
        struct Html {
            pub posts: Vec<Post>,
        }
        Ok(Html { posts })
    }

    // Display a single post.
    pub async fn view(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/view.html")]
        struct Html {
            pub post: Post,
        }
        Ok(Html { post })
    }

    // Display a preview of a post as it would appear in an email.
    pub async fn preview(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/email.html")]
        struct Html {
            pub post: Post,
            pub opened_url: String,
            pub unsub_url: String,
        }
        Ok(Html { post, opened_url: "".into(), unsub_url: "".into() })
    }
}

/// Create and edit posts.
mod edit {
    use super::*;

    // New post page.
    pub async fn new() -> AppResult<impl IntoResponse> {
        let post = Post {
            id: 0,
            title: "".into(),
            url: "".into(),
            author: "".into(),
            content: "".into(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/edit.html")]
        pub struct Html {
            pub post: Post,
        }
        Ok(Html { post })
    }

    // Edit post page.
    pub async fn edit_page(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/edit.html")]
        pub struct Html {
            pub post: Post,
        }
        Ok(Html { post })
    }

    // Edit post form.
    #[derive(serde::Deserialize)]
    pub struct EditForm {
        id: Option<i64>,
        #[serde(flatten)]
        post: UpdatePost,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>,
        Form(form): Form<EditForm>,
    ) -> AppResult<impl IntoResponse> {
        match form.id {
            Some(id) => Post::update(&state.db, id, &form.post).await?,
            None => {
                Post::create(&state.db, &form.post).await?;
            }
        }

        Ok(())
    }

    // Delete post form.
    pub async fn delete_form(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };
        Post::delete(&state.db, post.id).await?;
        Ok(Redirect::to("/posts"))
    }
}

mod send {
    use axum::body::Body;
    use futures::StreamExt;
    use serde_json::json;

    use super::*;

    /// Display the form to send a post.
    pub async fn page(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(sqlx::FromRow)]
        struct ListExt {
            id: i64,
            name: String,
            count: i64,
            skip: i64,
        }
        let lists = sqlx::query_as!(
            ListExt,
            r#"
            SELECT l.id, l.name, COUNT(m.email) as count, COUNT(e.sent_at) as skip
            FROM lists l
            LEFT JOIN list_members m ON m.list_id = l.id
            LEFT JOIN emails e
                ON e.address = m.email
                AND e.post_id = ?
                AND e.list_id = l.id
                AND e.sent_at IS NOT NULL
            GROUP BY l.id;
            "#,
            post.id,
        )
        .fetch_all(&state.db)
        .await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/send.html")]
        pub struct Html {
            pub post: Post,
            pub lists: Vec<ListExt>,
            pub ratelimit: usize,
        }
        let ratelimit = state.config.email.ratelimit;
        Ok(Html { post, lists, ratelimit })
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "posts/email.html")]
    pub struct EmailHtml {
        pub post: Post,
        pub opened_url: String,
        pub unsub_url: String,
    }

    // Process the form and create or edit a post.
    #[derive(serde::Deserialize)]
    pub struct SendForm {
        list_id: i64,
    }
    pub async fn form(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
        Form(form): Form<SendForm>,
    ) -> AppResult<impl IntoResponse> {
        let Some(mut post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };
        let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
            return Err(AppError::NotFound);
        };

        let emails = Email::create_posts(&state.db, post.id, list.id).await?;

        // XXX: The `url` field is just a slug, not an absolute URL.
        // We can't yet access `config.app.url` within templates, so we just mutate
        // the URL here and rely on that behavior in the `email.html` template.
        post.url = format!("{}/p/{}", &state.config.app.url, &post.url);
        let mut email_template =
            EmailHtml { post: post.clone(), opened_url: "".into(), unsub_url: "".into() };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.opened_url = format!("{}/emails/{id}/footer.gif", &state.config.app.url);
            email_template.unsub_url = format!("{}/emails/{id}/unsubscribe", &state.config.app.url);

            let from = &state.config.email.from;
            let reply_to = state.config.email.reply_to.as_ref().unwrap_or(from);
            let message = state
                .mailer
                .builder()
                .to(address.parse().unwrap())
                .reply_to(reply_to.clone())
                .subject(&post.title)
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(email_template.render()?)
                .unwrap();

            messages.push(message);
            email_ids.push(id);
        }

        let email_ids = futures::stream::iter(email_ids);
        let results = state.mailer.send_batch(Arc::clone(&state), messages).await;

        let body = Body::from_stream(async_stream::stream! {
            let mut stream = Box::pin(results.zip(email_ids));
            while let Some((progress, email_id)) = stream.next().await {
                let json = match progress {
                    Ok(p) => {
                        Email::mark_sent(&state.db, email_id).await?;
                        json!({"sent": p.sent, "remaining": p.remaining})
                    }
                    Err(e) => {
                        let e = e.to_string();
                        Email::mark_error(&state.db, email_id, &e).await?;
                        json!({"error": e})
                    }
                }.to_string();
                yield Ok::<_, AppError>(format!("{json}\n"));
            }
        });

        Ok(body)
    }
}

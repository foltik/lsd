use crate::db::list::List;
use crate::db::post::{Post, UpdatePost};
use crate::prelude::*;

/// Add all `post` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| {
            r.route("/p/{slug}", get(read::view_page))
        })
        .restricted_routes(User::WRITER, |r| {
            r.route("/posts", get(read::list_page))
             .route("/posts/new", get(edit::new_page))
             .route("/posts/{slug}/edit", get(edit::edit_page).post(edit::edit_form))
             .route("/posts/{slug}/delete", post(edit::delete_form))
             .route("/posts/{slug}/send", get(send::page).post(send::send_form))
             .route("/posts/{slug}/preview", get(read::preview_page))
        })
}

/// View and list posts.
mod read {
    use super::*;

    // Display a list of posts.
    pub async fn list_page(user: User, State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
        let posts = Post::list(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/list.html")]
        struct Html {
            user: Option<User>,
            posts: Vec<Post>,
        }
        Ok(Html { user: Some(user), posts })
    }

    // Display a single post.
    pub async fn view_page(
        user: Option<User>, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/view.html")]
        struct Html {
            user: Option<User>,
            post: Post,
        }
        Ok(Html { user, post })
    }

    // Display a preview of a post as it would appear in an email.
    pub async fn preview_page(
        State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/email.html")]
        struct EmailHtml {
            post: Post,
            post_url: String,
            opened_url: String,
            unsub_url: String,
        }
        Ok(EmailHtml {
            post_url: format!("{}/p/{}", &state.config.app.url, &post.slug),
            opened_url: "".into(),
            unsub_url: "".into(),
            post,
        })
    }
}

/// Create and edit posts.
mod edit {
    use super::*;

    struct EditorContent {
        html: String,
        updated_at: NaiveDateTime,
    }
    struct Editor {
        /// Where the content gets POSTed to.
        /// The string "{id}" is replaced with the current entity id.
        /// Returns JSON, either {id: 123} or {error: ""}
        url: &'static str,
        snapshot_prefix: &'static str,

        entity_id: Option<i64>,
        content: Option<EditorContent>,
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "posts/edit.html")]
    struct EditHtml {
        user: Option<User>,
        post: Post,
        editor: Editor,
    }

    // New post page.
    pub async fn new_page(user: User) -> AppResult<impl IntoResponse> {
        Ok(EditHtml {
            user: Some(user),
            post: Post {
                id: 0,
                title: "".into(),
                slug: "".into(),
                author: "".into(),
                content: "".into(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            },
            editor: Editor {
                url: "/posts/{id}/edit",
                snapshot_prefix: "post",
                entity_id: None,
                content: None,
            },
        })
    }

    // Edit post page.
    pub async fn edit_page(
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            return Err(AppError::NotFound);
        };

        Ok(EditHtml {
            user: Some(user),
            editor: Editor {
                url: "/posts/{id}/edit",
                snapshot_prefix: "post",
                entity_id: Some(post.id),
                content: Some(EditorContent { html: post.content.clone(), updated_at: post.updated_at }),
            },
            post,
        })
    }

    // Edit post form.
    #[derive(serde::Deserialize)]
    pub struct EditForm {
        id: i64,
        #[serde(flatten)]
        post: UpdatePost,
    }
    #[derive(serde::Serialize)]
    pub struct EditResponse {
        error: Option<String>,
        id: Option<i64>,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>, Form(form): Form<EditForm>,
    ) -> Json<EditResponse> {
        let res = match form.id {
            0 => Post::create(&state.db, &form.post).await,
            id => Post::update(&state.db, id, &form.post).await.map(|_| id),
        };

        Json(match res {
            Ok(id) => EditResponse { id: Some(id), error: None },
            Err(e) => EditResponse { id: None, error: Some(format!("{e:#}")) },
        })
    }

    // Delete post form.
    pub async fn delete_form(
        State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
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
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            return Err(AppError::NotFound);
        };

        #[derive(sqlx::FromRow)]
        struct ListExt {
            id: i64,
            name: String,
            count: i64,
            sent: i64,
        }
        let lists = sqlx::query_as!(
            ListExt,
            r#"
            SELECT
                l.id,
                l.name,
                COUNT(lm.user_id) AS count,
                SUM(
                    CASE WHEN EXISTS (
                        SELECT 1
                        FROM emails e
                        WHERE e.user_id = u.id
                          AND e.list_id = l.id
                          AND e.post_id = ?
                          AND e.sent_at IS NOT NULL
                    )
                    THEN 1 ELSE 0 END
                ) AS sent
            FROM lists l
            LEFT JOIN list_members lm ON lm.list_id = l.id
            LEFT JOIN users u ON u.id = lm.user_id
            GROUP BY l.id;
            "#,
            post.id,
        )
        .fetch_all(&state.db)
        .await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/send.html")]
        struct Html {
            user: Option<User>,
            post: Post,
            lists: Vec<ListExt>,
            ratelimit: usize,
        }
        let ratelimit = state.config.email.ratelimit;
        Ok(Html { user: Some(user), post, lists, ratelimit })
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "posts/email.html")]
    struct EmailHtml {
        post: Post,
        post_url: String,
        opened_url: String,
        unsub_url: String,
    }

    // Process the form and create or edit a post.
    #[derive(serde::Deserialize)]
    pub struct SendForm {
        list_id: i64,
        resend: bool,
    }
    pub async fn send_form(
        State(state): State<SharedAppState>, Path(slug): Path<String>, Form(form): Form<SendForm>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            return Err(AppError::NotFound);
        };
        let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
            return Err(AppError::NotFound);
        };

        let emails = match form.resend {
            false => Email::create_send_posts(&state.db, post.id, list.id).await?,
            true => Email::create_resend_posts(&state.db, post.id, list.id).await?,
        };

        let mut email_template = EmailHtml {
            post: post.clone(),
            post_url: format!("{}/p/{}", &state.config.app.url, &post.slug),
            opened_url: "".into(),
            unsub_url: "".into(),
        };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.opened_url = format!("{}/emails/{id}/footer.gif", &state.config.app.url);
            email_template.unsub_url = format!("{}/emails/{id}/unsubscribe", &state.config.app.url);

            tracing::info!("id={id} addres={address:?}");

            let from = &state.config.email.from;
            let reply_to = state.config.email.newsletter_reply_to.as_ref().unwrap_or(from);
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

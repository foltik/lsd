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
    pub async fn list_page(user: User, State(state): State<SharedAppState>) -> HtmlResult {
        let posts = Post::list(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/list.html")]
        struct Html {
            user: Option<User>,
            posts: Vec<Post>,
        }
        Ok(Html { user: Some(user), posts }.into_response())
    }

    // Display a single post.
    pub async fn view_page(
        user: Option<User>, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/view.html")]
        struct Html {
            user: Option<User>,
            post: Post,
        }
        Ok(Html { user, post }.into_response())
    }

    // Display a preview of a post as it would appear in an email.
    pub async fn preview_page(State(state): State<SharedAppState>, Path(slug): Path<String>) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
        };

        #[derive(Template, WebTemplate)]
        #[template(path = "emails/post.html")]
        struct EmailHtml {
            email_id: i64,
            post: Post,
            post_url: String,
        }
        Ok(EmailHtml {
            post_url: format!("{}/p/{}", &state.config.app.url, &post.slug),
            email_id: 0,
            post,
        }
        .into_response())
    }
}

/// Create and edit posts.
mod edit {
    use super::*;
    use crate::utils::editor::{Editor, EditorContent};

    #[derive(Template, WebTemplate)]
    #[template(path = "posts/edit.html")]
    struct EditHtml {
        user: Option<User>,
        post: Post,
        editor: Editor,
    }

    // New post page.
    pub async fn new_page(user: User) -> HtmlResult {
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
        }
        .into_response())
    }

    // Edit post page.
    pub async fn edit_page(
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!()
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
        }
        .into_response())
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
        id: Option<i64>,
        updated_at: Option<i64>,
        error: Option<String>,
    }
    pub async fn edit_form(
        State(state): State<SharedAppState>, Form(form): Form<EditForm>,
    ) -> JsonResult<EditResponse> {
        let (id, updated_at) = match form.id {
            0 => {
                if Post::lookup_by_slug(&state.db, &form.post.slug).await?.is_some() {
                    return Ok(Json(EditResponse {
                        id: None,
                        updated_at: None,
                        error: Some("A post with that slug already exists.".into()),
                    }));
                }
                Post::create(&state.db, &form.post).await?
            }
            id => Post::update(&state.db, id, &form.post).await?,
        };

        let updated_at = updated_at.and_utc().timestamp_millis();

        Ok(Json(EditResponse { id: Some(id), updated_at: Some(updated_at), error: None }))
    }

    // Delete post form.
    pub async fn delete_form(State(state): State<SharedAppState>, Path(slug): Path<String>) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
        };
        Post::delete(&state.db, post.id).await?;
        Ok(Redirect::to("/posts").into_response())
    }
}

mod send {
    use axum::body::Body;
    use futures::StreamExt;

    use super::*;

    /// Display the form to send a post.
    pub async fn page(
        user: User, State(state): State<SharedAppState>, Path(slug): Path<String>,
    ) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
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
        Ok(Html { user: Some(user), post, lists, ratelimit }.into_response())
    }

    #[derive(Template, WebTemplate)]
    #[template(path = "emails/post.html")]
    struct EmailHtml {
        email_id: i64,
        post: Post,
        post_url: String,
    }

    // Process the form and create or edit a post.
    #[derive(serde::Deserialize)]
    pub struct SendForm {
        list_id: i64,
        resend: bool,
    }
    pub async fn send_form(
        State(state): State<SharedAppState>, Path(slug): Path<String>, Form(form): Form<SendForm>,
    ) -> HtmlResult {
        let Some(post) = Post::lookup_by_slug(&state.db, &slug).await? else {
            bail_not_found!();
        };
        let Some(list) = List::lookup_by_id(&state.db, form.list_id).await? else {
            bail_not_found!();
        };

        let emails = match form.resend {
            false => Email::create_send_posts(&state.db, post.id, list.id).await?,
            true => Email::create_resend_posts(&state.db, post.id, list.id).await?,
        };

        let mut email_template = EmailHtml {
            email_id: 0,
            post: post.clone(),
            post_url: format!("{}/p/{}", &state.config.app.url, &post.slug),
        };
        let mut messages = vec![];
        let mut email_ids = vec![];
        for Email { id, address, sent_at, .. } in emails {
            if sent_at.is_some() {
                continue;
            }

            email_template.email_id = id;

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
                        let e = e.message();
                        Email::mark_error(&state.db, email_id, e).await?;
                        json!({"error": e})
                    }
                }.to_string();
                yield Ok::<_, AnyError>(format!("{json}\n"));
            }
        });

        Ok(body.into_response())
    }
}

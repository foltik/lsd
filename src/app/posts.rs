use crate::db::list::{List, ListMember};
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
    use super::*;

    /// Display the form to send a post.
    pub async fn page(
        State(state): State<SharedAppState>,
        Path(url): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let Some(post) = Post::lookup_by_url(&state.db, &url).await? else {
            return Err(AppError::NotFound);
        };
        let lists = List::list(&state.db).await?;

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/send.html")]
        pub struct Html {
            pub post: Post,
            pub lists: Vec<List>,
        }
        Ok(Html { post, lists })
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
        let members = List::list_members(&state.db, form.list_id).await?;

        // XXX: The `url` field is just a slug, not an absolute URL.
        // We can't yet access `config.app.url` within templates, so we just mutate
        // the URL here and rely on that behavior in the `email.html` template.
        post.url = format!("{}/p/{}", &state.config.app.url, &post.url);
        let mut email_template =
            EmailHtml { post: post.clone(), opened_url: "".into(), unsub_url: "".into() };

        let mut num_sent = 0;
        let mut num_skipped = 0;
        let mut errors = HashMap::new();
        let batch_size = state.config.email.ratelimit.unwrap_or(members.len());
        for (i, members) in members.chunks(batch_size).enumerate() {
            tracing::info!(
                "Sending emails ({}..{} of {})",
                i * batch_size + 1,
                (i + 1) * batch_size + 1,
                members.len()
            );
            for ListMember { email, .. } in members {
                // If this post was already sent to this address in this list, skip sending it again.
                if Email::lookup_post(&state.db, email, post.id, list.id).await?.is_some() {
                    num_skipped += 1;
                    continue;
                }
                let email_id = Email::create_post(&state.db, email, post.id, list.id).await?;

                email_template.opened_url = format!("{}/emails/{email_id}/footer.gif", &state.config.app.url);
                email_template.unsub_url = format!("{}/emails/{email_id}/unsubscribe", &state.config.app.url);

                use lettre::message::header::ContentType;

                let from = &state.config.email.from;
                let reply_to = state.config.email.reply_to.as_ref().unwrap_or(from);
                let msg = state
                    .mailer
                    .builder()
                    .to(email.parse().unwrap())
                    .reply_to(reply_to.clone())
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
                        tracing::error!("Sending email: {e:#}");
                        let e = e.to_string();
                        Email::mark_error(&state.db, email_id, &e).await?;
                        errors.insert(email.clone(), e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        tracing::info!("Successfully sent {} emails", members.len());

        #[derive(Template, WebTemplate)]
        #[template(path = "posts/sent.html")]
        pub struct SentHtml {
            pub post_title: String,
            pub list_name: String,
            pub num_sent: i64,
            pub num_skipped: i64,
            pub errors: HashMap<String, String>,
        }
        Ok(SentHtml { post_title: post.title, list_name: list.name, num_sent, num_skipped, errors })
    }
}

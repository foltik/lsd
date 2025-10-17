use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use lettre::message::Mailbox;

use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/contact", get(contact_page).post(contact_form)))
}

async fn contact_page(
    user: Option<User>,
    State(state): State<SharedAppState>,
) -> AppResult<impl IntoResponse> {
    #[derive(Template, WebTemplate)]
    #[template(path = "contact/send.html")]
    struct Html {
        user: Option<User>,
        turnstile_site_key: String,
    };
    Ok(Html {
        user,
        turnstile_site_key: state.config.cloudflare.turnstile_site_key.clone(),
    })
}

#[derive(serde::Deserialize, Debug)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
    #[serde(rename = "cf-turnstile-response")]
    turnstile_token: String,
}
async fn contact_form(
    user: Option<User>,
    State(state): State<SharedAppState>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Form(form): Form<ContactForm>,
) -> AppResult<impl IntoResponse> {
    if !state.cloudflare.validate_turnstile(client.ip(), &form.turnstile_token).await? {
        return Err(AppError::BadRequest);
    }

    let name = Some(form.name).filter(|n| !n.is_empty());
    let email = Some(form.email).filter(|e| !e.is_empty());

    let to = state.config.email.contact_to.clone();
    let from = state.config.email.from.clone();
    let subject = match &name {
        Some(name) => format!("[{name}]: {}", form.subject),
        None => format!("[Anonymous]: {}", form.subject),
    };
    let reply_to = match email {
        Some(e) => Some(Mailbox::new(name, e.parse().map_err(|_| AppError::BadRequest)?)),
        None => None,
    };

    let mut message = state.mailer.builder().to(to.unwrap_or(from)).subject(subject);
    if let Some(reply_to) = reply_to {
        message = message.reply_to(reply_to);
    }
    let message = message.body(form.message)?;

    state.mailer.send(&message).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "contact/message_sent.html")]
    struct Html {
        user: Option<User>,
    };
    Ok(Html { user })
}

use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use lettre::Message;
use lettre::message::Mailbox;

use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/contact", get(contact_page).post(contact_form))
            .route("/ethics", get(ethics_page).post(ethics_form))
    })
}

async fn contact_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    #[derive(Template, WebTemplate)]
    #[template(path = "contact/send.html")]
    struct Html {
        user: Option<User>,
        turnstile_site_key: String,
    };
    Ok(Html {
        user,
        turnstile_site_key: state.config.cloudflare.turnstile_site_key.clone(),
    }
    .into_response())
}

#[derive(serde::Deserialize, Debug)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
    #[serde(rename = "cf-turnstile-response")]
    turnstile_token: String,

    /// Hidden honeypot field to catch bots.
    #[serde(default, rename = "website")]
    honeypot: String,
}
async fn contact_form(
    user: Option<User>, State(state): State<SharedAppState>, ConnectInfo(client): ConnectInfo<SocketAddr>,
    Form(form): Form<ContactForm>,
) -> HtmlResult {
    if !form.honeypot.is_empty() {
        tracing::info!("Honeypot caught client_ip={}: {:?}", client.ip(), form);
        bail_invalid!();
    }
    if !state.cloudflare.validate_turnstile(client.ip(), &form.turnstile_token).await? {
        bail_invalid!();
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
        Some(e) => Some(Mailbox::new(name, e.parse().map_err(|_| invalid())?)),
        None => None,
    };

    let mut message = Message::builder().from(from.clone()).to(to.unwrap_or(from)).subject(subject);
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
    Ok(Html { user }.into_response())
}

async fn ethics_page(user: Option<User>) -> HtmlResult {
    #[derive(Template, WebTemplate)]
    #[template(path = "ethics.html")]
    struct Html {
        user: Option<User>,
    }
    Ok(Html { user }.into_response())
}

#[derive(serde::Deserialize)]
struct EthicsForm {
    name: String,
    email: String,
    statement: String,
}
async fn ethics_form(
    user: Option<User>, State(state): State<SharedAppState>, Form(form): Form<EthicsForm>,
) -> HtmlResult {
    let name = form.name.trim();
    let email = form.email.trim();
    let from = state.config.email.from.clone();
    let to = state.config.email.contact_to.clone();
    let reply_to = Mailbox::new(Some(name.to_string()), email.parse().map_err(|_| invalid())?);

    let message = Message::builder()
        .from(state.config.email.from.clone())
        .to(to.unwrap_or(from))
        .reply_to(reply_to)
        .subject(format!("[Ethics nomination] {name}"))
        .body(format!("{name}\n{email}\n\n{}", form.statement.trim()))?;
    state.mailer.send(&message).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "ethics_sent.html")]
    struct Html {
        user: Option<User>,
    }
    Ok(Html { user }.into_response())
}

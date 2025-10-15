use axum::http::HeaderMap;
use lettre::message::Mailbox;
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::prelude::*;
use crate::utils::turnstile::validate_turnstile;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/contact", get(contact_us_page).post(contact_us_form)))
}

#[axum::debug_handler]
async fn contact_us_page(
    user: Option<User>,
    State(state): State<SharedAppState>,
) -> AppResult<impl IntoResponse> {
    #[derive(Template, WebTemplate)]
    #[template(path = "contact/send.html")]
    struct ContactUsTemplate {
        user: Option<User>,
        turnstile_site_key: String,
    };

    Ok(ContactUsTemplate { user, turnstile_site_key: state.config.app.turnstile_site_key.clone() })
}

#[derive(Deserialize, Debug)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
    #[serde(rename = "cf-turnstile-response")]
    turnstile_token: String,
}

async fn contact_us_form(
    user: Option<User>,
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    Form(form): Form<ContactForm>,
) -> AppResult<impl IntoResponse> {
    // Extract client IP: prefer CF-Connecting-IP, fallback to X-Forwarded-For
    // Note: If neither header exists, we pass None and rely on Turnstile's server-side detection
    let remote_ip = headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next()) // take first IP if multiple
        .map(|s| s.trim());

    let is_valid = validate_turnstile(
        &form.turnstile_token,
        state.config.app.turnstile_secret_key.expose_secret(),
        remote_ip,
        &state.config.app.domain,
        "/contact",
    )
    .await?;

    if !is_valid {
        tracing::warn!("Contact form submission rejected due to failed Turnstile validation");
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

use crate::prelude::*;

/// Add all webhook routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/webhooks/stripe", post(stripe::webhook)))
}

pub mod stripe {
    use axum::http::HeaderMap;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    use super::*;
    use crate::db::event::Event;
    use crate::db::rsvp_session::RsvpSession;

    type HmacSha256 = Hmac<Sha256>;

    pub async fn webhook(
        State(state): State<SharedAppState>, headers: HeaderMap, body: String,
    ) -> JsonResult<()> {
        let signature = headers
            .get("stripe-signature")
            .ok_or_else(invalid)?
            .to_str()
            .map_err(|_| invalid())?;

        // 1. Parse timestamp and signatures
        let mut timestamp: Option<&str> = None;
        let mut signatures: Vec<&str> = Vec::new();
        for part in signature.split(',') {
            let mut kv = part.split('=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                match k {
                    "t" => timestamp = Some(v),
                    "v1" => signatures.push(v),
                    _ => {}
                }
            }
        }
        let timestamp = timestamp.ok_or_else(invalid)?;
        if signatures.is_empty() {
            crate::bail_invalid!();
        }

        // 2. Reconstruct `signed_payload`
        let signed_payload = format!("{timestamp}.{body}");

        // 3. Compute expected signature
        let mut mac = HmacSha256::new_from_slice(state.config.stripe.webhook_key.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        let expected_signature = hex::encode(mac.finalize().into_bytes());

        // 4. Compare against provided signatures. We ignore the timestamp for now.
        let valid = signatures.iter().any(|sig| sig == &expected_signature);
        if !valid {
            crate::bail_unauthorized!();
        }

        // tracing::debug!("STRIPE:  {body}");

        // 5. Dispatch to the correct handler
        #[derive(serde::Deserialize)]
        struct Type {
            #[serde(rename = "type")]
            ty: String,
        }
        #[derive(serde::Deserialize)]
        struct Event<T> {
            data: EventData<T>,
        }
        #[derive(serde::Deserialize)]
        struct EventData<T> {
            object: T,
        }
        let event: Type = serde_json::from_str(&body).map_err(|_| invalid())?;
        fn parse<T: serde::de::DeserializeOwned>(body: &str) -> AppResult<T> {
            let event: Event<T> = serde_json::from_str(body).map_err(|_| invalid())?;
            Ok(event.data.object)
        }
        match event.ty.as_str() {
            "checkout.session.completed" => {
                checkout_session_completed(state, parse::<CheckoutSessionCompleted>(&body)?).await?
            }
            ty => tracing::debug!("Stripe: unhandled webhook of type={ty:?}"),
        }

        Ok(Json(()))
    }

    #[derive(Debug, serde::Deserialize)]
    struct CheckoutSessionCompleted {
        client_reference_id: String,
        payment_intent: String,
        payment_status: String,
    }
    async fn checkout_session_completed(
        state: SharedAppState, payload: CheckoutSessionCompleted,
    ) -> Result<()> {
        // unwrap(): we assume Stripe won't send us bogus data. RsvpSessions are never deleted.
        let session_id: i64 = payload.client_reference_id.parse().unwrap();
        let Some(session) = RsvpSession::lookup_by_id(&state.db, session_id).await? else {
            bail!(
                "Stripe: Unknown rsvp_session={session_id} while handling webhook for payment_intent={}",
                payload.payment_intent,
            );
        };
        let Some(user_id) = session.user_id else {
            bail!(
                "Stripe: Got rsvp_session={session_id} with empty user_id while handling webhook for payment_intent={}",
                payload.payment_intent,
            );
        };
        let Some(user) = User::lookup_by_id(&state.db, user_id).await? else {
            bail!(
                "Stripe: Got rsvp_session={session_id} with unknown user_id={} while handling webhook for payment_intent={}",
                user_id,
                payload.payment_intent,
            );
        };
        let Some(event) = Event::lookup_by_id(&state.db, session.event_id).await? else {
            bail!(
                "Stripe: Got rsvp_session={session_id} with nonexistant event_id={} while handling webhook for payment_intent={}",
                session.event_id,
                payload.payment_intent,
            );
        };

        match payload.payment_status.as_str() {
            "paid" => {
                tracing::info!(
                    "Stripe[checkout.session.completed]: session={session:?} intent={:?}",
                    payload.payment_intent
                );

                session.set_status(&state.db, RsvpSession::PAYMENT_CONFIRMED).await?;
                session.set_payment_intent_id(&state.db, &payload.payment_intent).await?;

                if !Email::have_sent_confirmation(&state.db, session.event_id, user_id).await? {
                    let email = Email::create_confirmation(&state.db, session.event_id, user_id).await?;

                    #[derive(Template, WebTemplate)]
                    #[template(path = "emails/event_confirmation.html")]
                    struct ConfirmationEmailHtml {
                        email_id: i64,
                        event: Event,
                        token: String,
                    }

                    let from = &state.config.email.from;
                    let reply_to = state.config.email.contact_to.as_ref().unwrap_or(from);
                    let subject = event
                        .confirmation_subject
                        .clone()
                        .unwrap_or_else(|| format!("Confirmation for {}", event.title));
                    let message = state
                        .mailer
                        .builder()
                        .to(user.email.parse().unwrap())
                        .reply_to(reply_to.clone())
                        .subject(subject)
                        .header(lettre::message::header::ContentType::TEXT_HTML)
                        .body(
                            ConfirmationEmailHtml { email_id: email.id, event, token: session.token }
                                .render()?,
                        )
                        .unwrap();

                    state.mailer.send(&message).await?;
                }
            }
            status => {
                tracing::error!(
                    "Stripe[checkout.session.completed]: unknown payment_status={status} for rsvp_session={session_id}"
                )
            }
        }

        Ok(())
    }
}

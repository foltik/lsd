use crate::prelude::*;

/// Add all webhook routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/webhooks/stripe", post(stripe::webhook))
         .route("/webhooks/telnyx", post(telnyx::webhook))
    })
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
        fn parse<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, AppError> {
            let event: Event<T> = serde_json::from_str(body).map_err(|_| invalid())?;
            Ok(event.data.object)
        }
        match event.ty.as_str() {
            "checkout.session.completed" => {
                checkout_session_completed(state, parse::<CheckoutSessionCompleted>(&body)?).await?
            }
            "charge.refunded" => charge_refunded(state, parse::<ChargeRefunded>(&body)?).await?,
            "refund.failed" => refund_failed(state, parse::<RefundFailed>(&body)?).await?,
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
            alert!(
                "Stripe: Unknown rsvp_session={session_id} while handling checkout_session_completed webhook: {payload:?}"
            );
            return Ok(());
        };
        let Some(user_id) = session.user_id else {
            alert!(
                "Stripe: Got rsvp_session={session_id} with empty user_id while handling checkout_session_completed webhook: {payload:?}"
            );
            return Ok(());
        };
        let Some(user) = User::lookup_by_id(&state.db, user_id).await? else {
            alert!(
                "Stripe: Got rsvp_session={session_id} with unknown user_id={user_id} while handling checkout_session_completed webhook: {payload:?}"
            );
            return Ok(());
        };
        let Some(event) = Event::lookup_by_id(&state.db, session.event_id).await? else {
            alert!(
                "Stripe: Got rsvp_session={session_id} with nonexistant event_id={} while handling checkout_session_completed webhook: {payload:?}",
                session.event_id,
            );
            return Ok(());
        };

        match payload.payment_status.as_str() {
            "paid" => {
                tracing::info!(
                    "Stripe[checkout.session.completed]: session={session:?} intent={:?}",
                    payload.payment_intent
                );

                session.set_status(&state.db, RsvpSession::PAYMENT_CONFIRMED).await?;
                session.set_payment_intent_id(&state.db, &payload.payment_intent).await?;

                crate::app::events::email::ensure_sent_confirmation_email(
                    &state,
                    &event,
                    &user,
                    &session.token,
                    "webhook",
                )
                .await?;
            }
            status => {
                tracing::error!(
                    "Stripe[checkout.session.completed]: unknown payment_status={status} for rsvp_session={session_id}"
                )
            }
        }

        Ok(())
    }

    #[derive(Debug, serde::Deserialize)]
    struct ChargeRefunded {
        payment_intent: String,
        refunded: bool,
    }
    async fn charge_refunded(state: SharedAppState, payload: ChargeRefunded) -> Result<()> {
        if !payload.refunded {
            tracing::warn!(
                "Stripe[charge.refunded]: refunded=false for payment_intent={}",
                payload.payment_intent
            );
            return Ok(());
        }

        let session = sqlx::query_as!(
            RsvpSession,
            r#"SELECT * FROM rsvp_sessions WHERE stripe_payment_intent_id = ?"#,
            payload.payment_intent,
        )
        .fetch_optional(&state.db)
        .await?;

        let Some(session) = session else {
            tracing::warn!(
                "Stripe[charge.refunded]: no session found for payment_intent={}",
                payload.payment_intent
            );
            return Ok(());
        };

        if session.status != RsvpSession::REFUND_PENDING {
            tracing::warn!(
                "Stripe[charge.refunded]: session={} has status={:?}, expected refund_pending",
                session.id,
                session.status
            );
            return Ok(());
        }

        tracing::info!(
            "Stripe[charge.refunded]: confirming refund for session={} payment_intent={}",
            session.id,
            payload.payment_intent
        );
        session.set_status(&state.db, RsvpSession::REFUND_CONFIRMED).await?;
        Ok(())
    }

    #[derive(Debug, serde::Deserialize)]
    struct RefundFailed {
        payment_intent: Option<String>,
        failure_reason: Option<String>,
    }
    async fn refund_failed(_state: SharedAppState, payload: RefundFailed) -> Result<()> {
        let payment_intent = payload.payment_intent.as_deref().unwrap_or("unknown");
        let failure_reason = payload.failure_reason.as_deref().unwrap_or("unknown");

        alert!("Stripe[refund.failed]: payment_intent={payment_intent} failure_reason={failure_reason}");

        Ok(())
    }
}

pub mod telnyx {
    use axum::http::HeaderMap;
    use base64::Engine;
    use base64::prelude::BASE64_STANDARD as BASE64;
    use ring::signature::{ED25519, UnparsedPublicKey};

    use super::*;
    use crate::utils::telnyx::Telnyx;

    const REPLY: &str = "You've reached Light and Sound Design! Visit https://lightandsound.design for info on upcoming events.";

    #[derive(serde::Deserialize)]
    struct Event {
        data: EventData,
    }
    #[derive(serde::Deserialize)]
    struct EventData {
        event_type: String,
        payload: serde_json::Value,
    }

    #[derive(Debug, serde::Deserialize)]
    struct MessageReceived {
        from: PhoneNumber,
        to: Vec<PhoneNumber>,
    }
    #[derive(Debug, serde::Deserialize)]
    struct PhoneNumber {
        phone_number: String,
    }

    pub async fn webhook(
        State(state): State<SharedAppState>, headers: HeaderMap, body: String,
    ) -> JsonResult<()> {
        let Some(telnyx) = &state.telnyx else {
            bail_not_found!();
        };
        let header = |name| headers.get(name).ok_or_else(invalid)?.to_str().map_err(|_| invalid());

        let timestamp = header("telnyx-timestamp")?.parse::<i64>().map_err(|_| invalid())?;
        let stale = (Utc::now().timestamp() - timestamp).abs() >= 5 * 60; // 5min
        if stale {
            bail_invalid!();
        }

        let signature = BASE64.decode(header("telnyx-signature-ed25519")?).map_err(|_| invalid())?;
        let pubkey = BASE64.decode(&telnyx.public_key).unwrap();
        let payload = format!("{timestamp}|{body}");
        let valid = UnparsedPublicKey::new(&ED25519, pubkey)
            .verify(payload.as_bytes(), &signature)
            .is_ok();
        if !valid {
            crate::bail_unauthorized!();
        }

        let event: Event = serde_json::from_str(&body).map_err(|_| invalid())?;
        match event.data.event_type.as_str() {
            "message.received" => {
                let payload = serde_json::from_value(event.data.payload).map_err(|_| invalid())?;
                on_message_received(telnyx, payload).await?
            }
            ty => tracing::debug!("Telnyx: unhandled webhook of type={ty:?}"),
        }

        Ok(Json(()))
    }

    async fn on_message_received(telnyx: &Telnyx, msg: MessageReceived) -> Result<()> {
        let from = &msg.from.phone_number;
        let to = &msg.to.first().unwrap().phone_number;
        tracing::info!("Telnyx[message.received]: from={from} to={to}");

        telnyx.send_sms(to, from, REPLY).await
    }
}

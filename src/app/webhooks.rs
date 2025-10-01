use tokio::sync::{Mutex, Notify};

use crate::prelude::*;

/// Add all webhook routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/webhooks/stripe", post(stripe::webhook)))
}

/// Webhook notification mechanism.
#[derive(Clone, Default)]
pub struct Webhooks(Arc<Mutex<HashMap<String, Arc<Notify>>>>);
impl Webhooks {
    pub const STRIPE_CHECKOUT_SESSION_COMPLETED: &str = "stripe.checkout.session.completed";

    pub async fn wait(&self, webhook: &str, timeout: Duration) -> Result<(), ()> {
        let notify = {
            let mut map = self.0.lock().await;
            let entry = map.entry(webhook.to_string()).or_default();
            Arc::clone(entry)
        };
        tokio::select! {
            _ = tokio::time::sleep(timeout) => Err(()),
            _ = notify.notified() => Ok(()),
        }
    }

    async fn notify(&self, webhook: &str) {
        let map = self.0.lock().await;
        if let Some(notify) = map.get(webhook) {
            notify.notify_waiters();
        }
    }
}

pub mod stripe {
    use axum::http::HeaderMap;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    use super::*;
    use crate::db::rsvp_session::RsvpSession;

    type HmacSha256 = Hmac<Sha256>;

    pub async fn webhook(
        State(state): State<SharedAppState>,
        headers: HeaderMap,
        body: String,
    ) -> AppResult<impl IntoResponse> {
        let signature = headers
            .get("stripe-signature")
            .ok_or(AppError::BadRequest)?
            .to_str()
            .map_err(|_| AppError::BadRequest)?;

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
        let timestamp = timestamp.ok_or(AppError::BadRequest)?;
        if signatures.is_empty() {
            return Err(AppError::BadRequest);
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
            return Err(AppError::Unauthorized);
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
        let event: Type = serde_json::from_str(&body).map_err(|_| AppError::BadRequest)?;
        fn parse<T: serde::de::DeserializeOwned>(body: &str) -> AppResult<T> {
            let event: Event<T> = serde_json::from_str(body).map_err(|_| AppError::BadRequest)?;
            Ok(event.data.object)
        }
        match event.ty.as_str() {
            "checkout.session.completed" => {
                checkout_session_completed(state, parse::<CheckoutSessionCompleted>(&body)?).await?
            }
            ty => tracing::debug!("Stripe: unhandled webhook of type={ty:?}"),
        }

        Ok((StatusCode::OK, "OK"))
    }

    #[derive(Debug, serde::Deserialize)]
    struct CheckoutSessionCompleted {
        client_reference_id: String,
        payment_intent: String,
        payment_status: String,
    }
    async fn checkout_session_completed(
        state: SharedAppState,
        event: CheckoutSessionCompleted,
    ) -> AppResult<()> {
        // unwrap(): we assume Stripe won't send us bogus data. RsvpSessions are never deleted.
        let session_id: i64 = event.client_reference_id.parse().unwrap();
        let session = RsvpSession::lookup_by_id(&state.db, session_id).await?.unwrap();

        match event.payment_status.as_str() {
            "paid" => {
                tracing::info!("STRIPE PAID: session={session:?} intent={:?}", event.payment_intent);
                session.set_paid(&state.db, &event.payment_intent).await?
            }
            status => {
                tracing::error!("Stripe: unknown payment_status={status} for rsvp_session={session_id}")
            }
        }

        state.webhooks.notify(Webhooks::STRIPE_CHECKOUT_SESSION_COMPLETED).await;
        Ok(())
    }
}

use crate::db::rsvp_session::RsvpSession;
use crate::prelude::*;

const API_VERSION: &str = "2025-07-30.basil";

pub struct Stripe {
    app_url: String,
    secret_key: String,
    http: reqwest::Client,
}

pub struct LineItem {
    /// Item name.
    pub name: String,
    /// Number of this item to purchase.
    pub quantity: i64,
    /// Item unit price in dollars.
    pub price: i64,
}

impl Stripe {
    pub fn new(config: &Config) -> Self {
        Self {
            app_url: config.app.url.clone(),
            secret_key: config.stripe.secret_key.clone(),
            http: reqwest::Client::new(),
        }
    }

    /// Begin a stripe transaction, returning the client secret.
    pub async fn create_session(
        &self,
        session_id: i64,
        email: &str,
        line_items: Vec<LineItem>,
        return_path: String,
    ) -> AppResult<String> {
        let return_url = format!("{}{}", self.app_url, return_path);

        // Gross but there doesn't seem to be any other supported way to build form data in the way
        // that stripe expects in particular for lists of objects.
        //
        // The v2 APIs will allow sending JSON data but currently checkout API doesn't support v2
        // as of 2025-06-10.
        //
        // See https://docs.stripe.com/api/checkout/sessions/create?api-version=2025-05-28.basil
        //
        // TODO: do we need?
        let mut form_data = format!(
            "client_reference_id={session_id}\
            &customer_email={email}\
            &ui_mode=custom\
            &mode=payment\
            &currency=usd\
            &allow_promotion_codes=false\
            &payment_method_types[]=card\
            &return_url={return_url}"
        );

        for (i, LineItem { name, quantity, price }) in line_items.into_iter().enumerate() {
            let price_cents = price * 100;
            write!(
                &mut form_data,
                "&line_items[{i}][quantity]={quantity}\
                &line_items[{i}][price_data][currency]=usd\
                &line_items[{i}][price_data][unit_amount]={price_cents}\
                &line_items[{i}][price_data][product_data][name]={name}"
            )
            .unwrap(); // write!() to a String can't fail
        }

        #[derive(serde::Deserialize)]
        struct Response {
            client_secret: String,
        }

        #[rustfmt::skip]
        let res: Response = self.http
            .post("https://api.stripe.com/v1/checkout/sessions")
            .header("Stripe-Version", API_VERSION)
            .header(header::AUTHORIZATION, format!("Bearer {}", &self.secret_key))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(form_data)
            .send().await?.json().await?;

        Ok(res.client_secret)
    }

    pub async fn wait_for_payment(db: &Db, webhooks: &Webhooks, session_id: i64) -> Result<(), AppError> {
        const TIMEOUT: Duration = Duration::from_secs(3);
        const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(3);

        let start = Instant::now();
        loop {
            let webhook = webhooks.wait(Webhooks::STRIPE_CHECKOUT_SESSION_COMPLETED, WEBHOOK_TIMEOUT);

            let status = RsvpSession::lookup_status(db, session_id).await?.unwrap();
            if status == "paid" {
                return Ok(());
            }

            let _ = webhook.await;

            let status = RsvpSession::lookup_status(db, session_id).await?.unwrap();
            if status == "paid" {
                return Ok(());
            }

            if start.elapsed() > TIMEOUT {
                return Err(StripeError::PaymentTimeout { session_id, timeout: TIMEOUT }.into());
            }
        }
    }
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum StripeError {
    #[error("timed out waiting for payment for session={session_id} timeout={timeout:?}")]
    PaymentTimeout { session_id: i64, timeout: Duration },
}

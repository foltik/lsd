use crate::db::rsvp_session::RsvpSession;
use crate::prelude::*;

const API_VERSION: &str = "2025-07-30.basil";

pub struct Stripe {
    app_url: String,
    secret_key: String,
    http: reqwest::Client,
}

#[derive(Debug)]
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
        &self, session_id: i64, email: &str, line_items: Vec<LineItem>, return_path: String,
    ) -> Result<String> {
        let return_url = format!("{}{}", self.app_url, return_path);

        // Log line_items for debugging before we consume them
        let line_items_debug = format!("{:?}", &line_items);

        // Gross but there doesn't seem to be any other supported way to build form data in the way
        // that stripe expects in particular for lists of objects.
        //
        // The v2 APIs will allow sending JSON data but currently checkout API doesn't support v2
        // as of 2025-06-10.
        //
        // See https://docs.stripe.com/api/checkout/sessions/create?api-version=2025-05-28.basil
        //
        let expires_at = Utc::now().timestamp() + (RsvpSession::STRIPE_EXPIRY_MINUTES * 60);
        let mut form_data = format!(
            "client_reference_id={session_id}\
            &customer_email={email}\
            &ui_mode=custom\
            &mode=payment\
            &currency=usd\
            &expires_at={expires_at}\
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
            client_secret: Option<String>,
            error: Option<StripeError>,
        }

        #[derive(serde::Deserialize)]
        struct StripeError {
            message: String,
            #[serde(rename = "type")]
            error_type: String,
        }

        #[rustfmt::skip]
        let res: Response = self.http
            .post("https://api.stripe.com/v1/checkout/sessions")
            .header("Stripe-Version", API_VERSION)
            .header(header::AUTHORIZATION, format!("Bearer {}", &self.secret_key))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(form_data)
            .send().await?.json().await?;

        if let Some(err) = res.error {
            let msg = format!(
                "Stripe::create_session(): {} (type={}), session_id={session_id}, email={email}, line_items={line_items_debug}",
                err.message, err.error_type
            );
            crate::utils::sentry::report(msg.clone());
            bail!(msg);
        }

        res.client_secret.ok_or_else(|| {
            let msg = format!(
                "Stripe::create_session(): response missing client_secret, session_id={session_id}, email={email}, line_items={line_items_debug}"
            );
            crate::utils::sentry::report(msg.clone());
            any!(msg)
        })
    }
}

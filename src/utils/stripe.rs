use crate::prelude::*;
use crate::utils::config::StripeConfig;

pub struct Stripe {
    secret_key: String,
    http: reqwest::Client,
}

pub struct Item {
    /// Item name.
    pub name: String,
    /// Item description.
    pub description: String,
    /// Number of this item to purchase.
    pub quantity: usize,
    /// Item unit price in dollars.
    pub price: i64,
}

impl Stripe {
    pub fn new(config: StripeConfig) -> Self {
        Self { secret_key: config.secret_key, http: reqwest::Client::new() }
    }

    /// Begin a stripe transaction, returning the client secret.
    pub async fn begin_transaction(
        &self,
        user: &User,
        items: Vec<Item>,
        return_url: String,
    ) -> AppResult<String> {
        // Gross but there doesn't seem to be any other supported way to build form data in the way
        // that stripe expects in particular for lists of objects.
        //
        // The v2 APIs will allow sending JSON data but currently checkout API doesn't support v2
        // as of 2025-06-10.
        //
        // See https://docs.stripe.com/api/checkout/sessions/create?api-version=2025-05-28.basil
        let mut form_data = format!(
            "ui_mode=custom\
            &return_url={return_url}\
            &mode=payment\
            &currency=usd\
            &allow_promotion_codes=false\
            &payment_method_types[]=card\
            &customer_email={}",
            &user.email,
        );
        for (i, Item { name, description, quantity, price }) in items.into_iter().enumerate() {
            let price_cents = price * 100;
            write!(
                &mut form_data,
                "&line_items[{i}][quantity]={quantity}\
                &line_items[{i}][price_data][currency]=usd\
                &line_items[{i}][price_data][unit_amount]={price_cents}\
                &line_items[{i}][price_data][product_data][name]={name}\
                &line_items[{i}][price_data][product_data][description]={description}"
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
            .header(header::AUTHORIZATION, format!("Bearer {}", &self.secret_key))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(form_data)
            .send().await?.json().await?;

        Ok(res.client_secret)
    }
}

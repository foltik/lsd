use std::net::IpAddr;

use chrono::DateTime;

use crate::prelude::*;

pub struct Cloudflare {
    app_domain: String,
    turnstile_secret_key: String,
    http: reqwest::Client,
}

impl Cloudflare {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            app_domain: config.app.domain.clone(),
            turnstile_secret_key: config.cloudflare.turnstile_secret_key.clone(),
            http: reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?,
        })
    }

    /// Validates a Cloudflare Turnstile token from a client.
    pub async fn validate_turnstile(&self, client_ip: IpAddr, token: &str) -> Result<bool> {
        #[derive(serde::Serialize)]
        struct Request {
            secret: String,
            response: String,
            remoteip: String,
        }
        let req = self
            .http
            .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
            .json(&Request {
                secret: self.turnstile_secret_key.clone(),
                response: token.into(),
                remoteip: client_ip.to_string(),
            })
            .send()
            .await?;

        #[derive(serde::Deserialize, serde::Serialize, Debug)]
        struct Response {
            success: bool,
            hostname: String,
            challenge_ts: DateTime<Utc>,
            #[serde(rename = "error-codes")]
            error_codes: Vec<String>,
        }
        let res: Response = req.json().await?;

        // If things go wrong we could look in here, but no sense in printing bot spam.
        /* if !res.error_codes.is_empty() { ... } */

        let challenge_ok = res.success;
        let domain_ok = match self.app_domain.as_str() {
            "localhost" => true,
            domain => res.hostname == domain,
        };
        let age_ok = Utc::now().signed_duration_since(res.challenge_ts).num_minutes() < 5;

        Ok(challenge_ok && domain_ok && age_ok)
    }
}

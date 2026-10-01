use crate::prelude::*;
use crate::utils::config::TelnyxConfig;

pub struct Telnyx {
    api_key: String,
    pub public_key: String,
    http: reqwest::Client,
}

impl Telnyx {
    pub fn new(config: &TelnyxConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            public_key: config.public_key.clone(),
            http: reqwest::Client::new(),
        }
    }

    /// Numbers are in E.164 format, e.g. `+12125550123`.
    pub async fn send_sms(&self, from: &str, to: &str, text: &str) -> Result<()> {
        #[rustfmt::skip]
        let res = self.http
            .post("https://api.telnyx.com/v2/messages")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .json(&json!({ "from": from, "to": to, "text": text }))
            .send().await?;

        let status = res.status();
        if !status.is_success() {
            let msg =
                format!("Telnyx::send_sms(): status={status} from={from} to={to}: {}", res.text().await?);
            alert!("{msg}");
            bail!(msg);
        }
        Ok(())
    }
}

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::prelude::*;

#[derive(Deserialize, Serialize, Debug)]
struct TurnstileResponse {
    success: bool,
    challenge_ts: Option<String>,
    hostname: Option<String>,
    #[serde(rename = "error-codes")]
    error_codes: Option<Vec<String>>,
    action: Option<String>,
    cdata: Option<String>,
}

/// Validates a Cloudflare Turnstile token.
///
/// # Arguments
/// * `token` - The Turnstile response token from the client
/// * `secret` - The Turnstile secret key
/// * `remote_ip` - client IP address
/// * `expected_hostname` - expected hostname to validate against
/// * `expected_action` - expected action to validate against (from `data-action` HTML attribute)
///
/// # Returns
/// Returns `Ok(true)` if validation succeeds, `Ok(false)` if validation fails,
/// or an error if the API request fails.
pub async fn validate_turnstile(
    token: &str,
    secret: &str,
    remote_ip: Option<&str>,
    expected_hostname: &str,
    expected_action: &str,
) -> AppResult<bool> {
    let client = reqwest::Client::new();
    let url = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

    let idempotency_key = Uuid::new_v4().to_string();

    let mut form = HashMap::new();
    form.insert("secret", secret);
    form.insert("response", token);
    form.insert("idempotency_key", idempotency_key.as_str());

    if let Some(ip) = remote_ip {
        form.insert("remoteip", ip);
    }

    let response = client.post(url).form(&form).timeout(Duration::from_secs(10)).send().await?;

    let result: TurnstileResponse = response.json().await?;

    if !result.success {
        tracing::warn!("Turnstile validation failed. Error codes: {:?}", result.error_codes);
        return Ok(false);
    }

    // Check if using Cloudflare test keys (these always return "example.com" hostname)
    // https://developers.cloudflare.com/turnstile/troubleshooting/testing/
    let is_test_key = secret == "1x0000000000000000000000000000000AA"
        || secret == "2x0000000000000000000000000000000AA"
        || secret == "3x0000000000000000000000000000000AA";

    // Skip hostname/action validation for test keys since they return fixed values
    if !is_test_key {
        if let Some(actual_action) = &result.action
            && expected_action != actual_action
        {
            tracing::warn!(
                "Turnstile action mismatch. Expected: {}, Got: {}",
                expected_action,
                actual_action
            );
            return Ok(false);
        }

        if let Some(actual_hostname) = &result.hostname
            && expected_hostname != actual_hostname
        {
            tracing::warn!(
                "Turnstile hostname mismatch. Expected: {}, Got: {}",
                expected_hostname,
                actual_hostname
            );
            return Ok(false);
        }
    } else {
        tracing::debug!("Using Turnstile test key, skipping hostname/action validation");
    }

    if let Some(challenge_ts) = &result.challenge_ts
        && let Ok(challenge_time) = chrono::DateTime::parse_from_rfc3339(challenge_ts)
    {
        let now = Utc::now();
        let age = now.signed_duration_since(challenge_time);
        let age_minutes = age.num_minutes();

        if age_minutes > 4 {
            tracing::warn!("Turnstile token is {} minutes old", age_minutes);
        }
    }

    Ok(true)
}

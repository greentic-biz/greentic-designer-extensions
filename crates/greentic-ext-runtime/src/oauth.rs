//! OAuth broker HTTP client for the design-extension runtime.
//!
//! Ported from `greentic-runner` `greentic-runner-host/src/oauth.rs`. The broker
//! service performs token refresh; this client just relays a `resource-token`
//! request and returns the (already-fresh) access token.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use url::Url;

/// Configuration the host supplies so the extension runtime can reach the OAuth broker.
#[derive(Clone, Debug, Default)]
pub struct OAuthBrokerConfig {
    pub http_base_url: String,
    pub env: String,
    pub tenant: String,
    pub team: Option<String>,
    pub shared_secret: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourceTokenRequest {
    pub http_base_url: String,
    pub env: String,
    pub tenant: String,
    pub team: Option<String>,
    pub resource_id: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourceTokenResponse {
    pub access_token: String,
    pub expires_at: u64,
}

/// POST `{http_base_url}/resource-token` and return the resolved access token.
///
/// The broker refreshes the token if needed; the shared secret (when present)
/// authenticates this host→broker call and is never logged or returned.
pub fn request_resource_token_blocking(
    client: &reqwest::blocking::Client,
    request: &ResourceTokenRequest,
    shared_secret: Option<&str>,
) -> anyhow::Result<ResourceTokenResponse> {
    let base = Url::parse(&request.http_base_url)?;
    anyhow::ensure!(
        base.scheme() == "https",
        "oauth broker http_base_url must be https"
    );
    anyhow::ensure!(
        base.query().is_none(),
        "oauth broker http_base_url must not carry a query"
    );
    let url = base.join("resource-token")?;
    let mut rb = client.post(url).json(request);
    if let Some(secret) = shared_secret {
        rb = rb.bearer_auth(secret);
    }
    let response = rb.send()?.error_for_status()?;
    Ok(response.json()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_token_request_serializes_expected_shape() {
        let req = ResourceTokenRequest {
            http_base_url: "https://oauth.example/".into(),
            env: "dev".into(),
            tenant: "acme".into(),
            team: None,
            resource_id: "hubspot".into(),
            scopes: vec!["crm.objects.contacts.read".into()],
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["resource_id"], "hubspot");
        assert_eq!(v["tenant"], "acme");
        assert_eq!(v["scopes"][0], "crm.objects.contacts.read");
    }

    #[test]
    fn resource_token_response_deserializes() {
        let r: ResourceTokenResponse =
            serde_json::from_str(r#"{"access_token":"tok","expires_at":123}"#).unwrap();
        assert_eq!(r.access_token, "tok");
        assert_eq!(r.expires_at, 123);
    }
}

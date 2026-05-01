//! Configuration for the Miniflux provider.

use serde::{Deserialize, Serialize};

/// Configuration for the [`MinifluxProvider`](crate::MinifluxProvider).
///
/// `server_url` should point at the root of the Miniflux server (e.g.
/// `https://miniflux.example.com`). The `api_token` is a per-user token
/// generated in the Miniflux UI under Settings → API Keys and is sent to the
/// server in the `X-Auth-Token` header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinifluxProviderConfig {
    /// Base URL of the Miniflux server, e.g. `https://miniflux.example.com`.
    pub server_url: String,
    /// API token used for `X-Auth-Token` authentication.
    pub api_token: String,
}

impl MinifluxProviderConfig {
    /// Create a new configuration from a server URL and API token.
    pub fn new(server_url: impl Into<String>, api_token: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            api_token: api_token.into(),
        }
    }

    /// Construct a configuration by fetching the API token from Sigilforge.
    ///
    /// Looks up the token under the service identifier `"miniflux"` and the
    /// supplied `account` label.
    #[cfg(feature = "sigilforge")]
    pub async fn from_sigilforge(
        token_fetcher: &dyn scryforge_sigilforge_client::TokenFetcher,
        server_url: impl Into<String>,
        account: &str,
    ) -> std::result::Result<Self, scryforge_sigilforge_client::SigilforgeError> {
        let token = token_fetcher.fetch_token("miniflux", account).await?;
        Ok(Self::new(server_url, token))
    }
}

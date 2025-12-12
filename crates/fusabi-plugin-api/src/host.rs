//! Host functions exposed to plugins.
//!
//! Plugins can call these functions to interact with the Scryforge runtime.
//! All calls are capability-checked before execution.

use async_trait::async_trait;
use fusabi_runtime::{Capability, CapabilitySet, RuntimeError, RuntimeResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Host functions available to plugins.
#[async_trait]
pub trait HostFunctions: Send + Sync {
    /// Make an HTTP GET request.
    ///
    /// Requires: `Capability::Network`
    async fn http_get(
        &self,
        url: &str,
        headers: HashMap<String, String>,
    ) -> RuntimeResult<HttpResponse>;

    /// Make an HTTP POST request.
    ///
    /// Requires: `Capability::Network`
    async fn http_post(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        body: &str,
    ) -> RuntimeResult<HttpResponse>;

    /// Get a credential/token.
    ///
    /// Requires: `Capability::Credentials`
    async fn get_credential(&self, provider: &str, account: &str) -> RuntimeResult<String>;

    /// Read from cache.
    ///
    /// Requires: `Capability::CacheRead`
    async fn cache_get(&self, key: &str) -> RuntimeResult<Option<String>>;

    /// Write to cache.
    ///
    /// Requires: `Capability::CacheWrite`
    async fn cache_set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> RuntimeResult<()>;

    /// Log a message (always allowed).
    fn log(&self, level: LogLevel, message: &str);

    /// Get current timestamp in milliseconds.
    fn now_millis(&self) -> u64;
}

/// HTTP response from host.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Log level for plugin logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Default host functions implementation with capability checking.
pub struct DefaultHostFunctions {
    /// Capabilities granted to this plugin.
    capabilities: CapabilitySet,

    /// HTTP client for network requests.
    http_client: reqwest::Client,

    /// Token fetcher for credentials.
    #[allow(dead_code)]
    token_fetcher: Option<Arc<dyn TokenFetcher>>,

    /// In-memory cache for plugin data.
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Plugin ID for logging.
    plugin_id: String,
}

struct CacheEntry {
    value: String,
    expires_at: Option<std::time::Instant>,
}

/// Token fetcher trait for credential access.
#[async_trait]
pub trait TokenFetcher: Send + Sync {
    async fn fetch_token(&self, provider: &str, account: &str) -> RuntimeResult<String>;
}

impl DefaultHostFunctions {
    /// Create new host functions with the given capabilities.
    pub fn new(plugin_id: String, capabilities: CapabilitySet) -> Self {
        Self {
            capabilities,
            http_client: reqwest::Client::new(),
            token_fetcher: None,
            cache: Arc::new(RwLock::new(HashMap::new())),
            plugin_id,
        }
    }

    /// Set the token fetcher for credential access.
    pub fn with_token_fetcher(mut self, fetcher: Arc<dyn TokenFetcher>) -> Self {
        self.token_fetcher = Some(fetcher);
        self
    }

    /// Check if a capability is granted.
    fn check_capability(&self, cap: Capability) -> RuntimeResult<()> {
        if self.capabilities.has(&cap) {
            Ok(())
        } else {
            Err(RuntimeError::MissingCapability(cap.as_str().to_string()))
        }
    }
}

#[async_trait]
impl HostFunctions for DefaultHostFunctions {
    async fn http_get(
        &self,
        url: &str,
        headers: HashMap<String, String>,
    ) -> RuntimeResult<HttpResponse> {
        self.check_capability(Capability::Network)?;

        let mut request = self.http_client.get(url);
        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = request
            .send()
            .await
            .map_err(|e| RuntimeError::ExecutionError(format!("HTTP request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = response
            .text()
            .await
            .map_err(|e| RuntimeError::ExecutionError(format!("Failed to read response: {}", e)))?;

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }

    async fn http_post(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        body: &str,
    ) -> RuntimeResult<HttpResponse> {
        self.check_capability(Capability::Network)?;

        let mut request = self.http_client.post(url).body(body.to_string());
        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = request
            .send()
            .await
            .map_err(|e| RuntimeError::ExecutionError(format!("HTTP request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = response
            .text()
            .await
            .map_err(|e| RuntimeError::ExecutionError(format!("Failed to read response: {}", e)))?;

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }

    async fn get_credential(&self, provider: &str, account: &str) -> RuntimeResult<String> {
        self.check_capability(Capability::Credentials)?;

        match &self.token_fetcher {
            Some(fetcher) => fetcher.fetch_token(provider, account).await,
            None => Err(RuntimeError::ExecutionError(
                "Token fetcher not configured".to_string(),
            )),
        }
    }

    async fn cache_get(&self, key: &str) -> RuntimeResult<Option<String>> {
        self.check_capability(Capability::CacheRead)?;

        let cache = self.cache.read().await;
        match cache.get(key) {
            Some(entry) => {
                // Check expiration
                if let Some(expires) = entry.expires_at {
                    if expires < std::time::Instant::now() {
                        return Ok(None);
                    }
                }
                Ok(Some(entry.value.clone()))
            }
            None => Ok(None),
        }
    }

    async fn cache_set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> RuntimeResult<()> {
        self.check_capability(Capability::CacheWrite)?;

        let expires_at =
            ttl_seconds.map(|ttl| std::time::Instant::now() + std::time::Duration::from_secs(ttl));

        let mut cache = self.cache.write().await;
        cache.insert(
            key.to_string(),
            CacheEntry {
                value: value.to_string(),
                expires_at,
            },
        );

        Ok(())
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Trace => tracing::trace!(plugin = %self.plugin_id, "{}", message),
            LogLevel::Debug => tracing::debug!(plugin = %self.plugin_id, "{}", message),
            LogLevel::Info => tracing::info!(plugin = %self.plugin_id, "{}", message),
            LogLevel::Warn => tracing::warn!(plugin = %self.plugin_id, "{}", message),
            LogLevel::Error => tracing::error!(plugin = %self.plugin_id, "{}", message),
        }
    }

    fn now_millis(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capability_check() {
        let caps = CapabilitySet::from_strings(["network"]);
        let host = DefaultHostFunctions::new("test".to_string(), caps);

        // Should succeed - has network capability
        assert!(host.check_capability(Capability::Network).is_ok());

        // Should fail - no credentials capability
        assert!(host.check_capability(Capability::Credentials).is_err());
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let caps = CapabilitySet::from_strings(["cache_read", "cache_write"]);
        let host = DefaultHostFunctions::new("test".to_string(), caps);

        // Set a value
        host.cache_set("key1", "value1", None).await.unwrap();

        // Get the value
        let value = host.cache_get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Get non-existent key
        let missing = host.cache_get("nonexistent").await.unwrap();
        assert_eq!(missing, None);
    }
}

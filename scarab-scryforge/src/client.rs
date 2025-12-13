//! JSON-RPC client for communicating with scryforge-daemon.

use chrono::{DateTime, Utc};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Client for interacting with scryforge-daemon's JSON-RPC API.
pub struct ScryforgeClient {
    client: HttpClient,
}

/// Errors that can occur when interacting with the scryforge-daemon.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Failed to connect to daemon: {0}")]
    ConnectionError(String),

    #[error("RPC call failed: {0}")]
    RpcError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Sync status for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSyncState {
    pub last_sync: Option<DateTime<Utc>>,
    pub is_syncing: bool,
    pub error: Option<String>,
}

/// Statistics about unread items across all streams.
#[derive(Debug, Clone, Default)]
pub struct UnreadStats {
    pub total_unread: usize,
    pub by_provider: HashMap<String, usize>,
}

impl ScryforgeClient {
    /// Create a new client connected to the daemon at the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The daemon's JSON-RPC endpoint (e.g., "http://127.0.0.1:3030")
    pub async fn new(url: &str) -> Result<Self, ClientError> {
        let client = HttpClientBuilder::default()
            .build(url)
            .map_err(|e| ClientError::ConnectionError(e.to_string()))?;

        Ok(Self { client })
    }

    /// Get the total count of unread items across all streams.
    pub async fn get_unread_count(&self) -> Result<usize, ClientError> {
        // Get all streams
        let streams: Vec<serde_json::Value> = self
            .client
            .request("streams.list", rpc_params![])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))?;

        let mut total_unread = 0;

        // For each stream, get items and count unread
        for stream in streams {
            if let Some(stream_id) = stream.get("id").and_then(|v| v.as_str()) {
                let items: Vec<serde_json::Value> = self
                    .client
                    .request("items.list", rpc_params![stream_id])
                    .await
                    .map_err(|e| ClientError::RpcError(e.to_string()))?;

                total_unread += items
                    .iter()
                    .filter(|item| {
                        item.get("is_read")
                            .and_then(|v| v.as_bool())
                            .map(|is_read| !is_read)
                            .unwrap_or(false)
                    })
                    .count();
            }
        }

        Ok(total_unread)
    }

    /// Get detailed unread statistics by provider.
    pub async fn get_unread_stats(&self) -> Result<UnreadStats, ClientError> {
        let streams: Vec<serde_json::Value> = self
            .client
            .request("streams.list", rpc_params![])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))?;

        let mut stats = UnreadStats::default();

        for stream in streams {
            if let Some(stream_id) = stream.get("id").and_then(|v| v.as_str()) {
                let items: Vec<serde_json::Value> = self
                    .client
                    .request("items.list", rpc_params![stream_id])
                    .await
                    .map_err(|e| ClientError::RpcError(e.to_string()))?;

                let unread_count = items
                    .iter()
                    .filter(|item| {
                        item.get("is_read")
                            .and_then(|v| v.as_bool())
                            .map(|is_read| !is_read)
                            .unwrap_or(false)
                    })
                    .count();

                stats.total_unread += unread_count;

                // Extract provider ID from stream ID (format: "provider_id::stream_name")
                if let Some(provider_id) = stream_id.split("::").next() {
                    *stats.by_provider.entry(provider_id.to_string()).or_insert(0) += unread_count;
                }
            }
        }

        Ok(stats)
    }

    /// Get sync status for all providers.
    pub async fn get_sync_status(&self) -> Result<HashMap<String, ProviderSyncState>, ClientError> {
        self.client
            .request("sync.status", rpc_params![])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))
    }

    /// Trigger a manual sync for a specific provider.
    pub async fn trigger_sync(&self, provider_id: &str) -> Result<(), ClientError> {
        self.client
            .request("sync.trigger", rpc_params![provider_id])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))
    }

    /// Trigger sync for all providers.
    pub async fn sync_all(&self) -> Result<(), ClientError> {
        // Get all streams to find all providers
        let streams: Vec<serde_json::Value> = self
            .client
            .request("streams.list", rpc_params![])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))?;

        // Extract unique provider IDs
        let mut provider_ids = std::collections::HashSet::new();
        for stream in streams {
            if let Some(stream_id) = stream.get("id").and_then(|v| v.as_str()) {
                if let Some(provider_id) = stream_id.split("::").next() {
                    provider_ids.insert(provider_id.to_string());
                }
            }
        }

        // Trigger sync for each provider
        for provider_id in provider_ids {
            self.trigger_sync(&provider_id).await?;
        }

        Ok(())
    }

    /// Mark all items as read across all streams.
    pub async fn mark_all_read(&self) -> Result<(), ClientError> {
        // Get all streams
        let streams: Vec<serde_json::Value> = self
            .client
            .request("streams.list", rpc_params![])
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))?;

        // For each stream, get unread items and mark them as read
        for stream in streams {
            if let Some(stream_id) = stream.get("id").and_then(|v| v.as_str()) {
                let items: Vec<serde_json::Value> = self
                    .client
                    .request("items.list", rpc_params![stream_id])
                    .await
                    .map_err(|e| ClientError::RpcError(e.to_string()))?;

                for item in items {
                    let is_read = item
                        .get("is_read")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if !is_read {
                        if let Some(item_id) = item.get("id").and_then(|v| v.as_str()) {
                            // Mark item as read
                            let _: () = self
                                .client
                                .request("items.mark_read", rpc_params![item_id])
                                .await
                                .map_err(|e| ClientError::RpcError(e.to_string()))?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if the daemon is reachable and responding.
    pub async fn health_check(&self) -> bool {
        // Try to list streams as a simple health check
        self.client
            .request::<Vec<serde_json::Value>, _>("streams.list", rpc_params![])
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id_extraction() {
        let stream_id = "reddit::r/rust";
        let provider_id = stream_id.split("::").next().unwrap();
        assert_eq!(provider_id, "reddit");
    }

    #[test]
    fn test_unread_stats_default() {
        let stats = UnreadStats::default();
        assert_eq!(stats.total_unread, 0);
        assert!(stats.by_provider.is_empty());
    }
}

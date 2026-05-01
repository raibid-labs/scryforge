//! # provider-miniflux
//!
//! [Miniflux](https://miniflux.app) provider for Scryforge.
//!
//! This crate implements [`Provider`], [`HasFeeds`], and [`HasSavedItems`] by
//! delegating to a self-hosted Miniflux server's JSON API. It is the
//! always-on, multi-device counterpart to [`provider-rss`](../provider-rss):
//! Miniflux owns "fetch and cache feeds" while Scryforge becomes a
//! terminal-native client over the user's existing subscriptions.
//!
//! ## Configuration
//!
//! ```no_run
//! use provider_miniflux::{MinifluxProvider, MinifluxProviderConfig};
//!
//! let config = MinifluxProviderConfig::new(
//!     "https://miniflux.example.com",
//!     "your-api-token",
//! );
//! let provider = MinifluxProvider::new(config);
//! ```
//!
//! ## Sigilforge integration (optional)
//!
//! Enable the `sigilforge` cargo feature to fetch the API token from a
//! [Sigilforge] daemon instead of passing it inline:
//!
//! ```toml
//! [dependencies]
//! provider-miniflux = { version = "0.1", features = ["sigilforge"] }
//! ```
//!
//! ```no_run
//! # #[cfg(feature = "sigilforge")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use provider_miniflux::{MinifluxProvider, MinifluxProviderConfig};
//! use scryforge_sigilforge_client::MockTokenFetcher;
//!
//! let fetcher = MockTokenFetcher::empty();
//! let config = MinifluxProviderConfig::from_sigilforge(
//!     &fetcher,
//!     "https://miniflux.example.com",
//!     "personal",
//! ).await?;
//! let provider = MinifluxProvider::new(config);
//! # Ok(())
//! # }
//! ```
//!
//! [Sigilforge]: https://github.com/raibid-labs/sigilforge

pub mod api;
pub mod config;
pub mod mapping;

pub use config::MinifluxProviderConfig;

use async_trait::async_trait;
use chrono::Utc;
use scryforge_provider_core::prelude::*;
use std::any::Any;
use std::time::Instant;

use crate::api::EntryFilter;
use crate::mapping::{
    entry_to_item, feed_to_feed, parse_feed_id, parse_item_id, saved_stream_id, PROVIDER_ID,
};

pub use crate::api::{MinifluxApiError, MinifluxClient};

/// Miniflux provider implementing `Provider + HasFeeds + HasSavedItems`.
pub struct MinifluxProvider {
    client: MinifluxClient,
}

impl MinifluxProvider {
    /// Create a new provider from a [`MinifluxProviderConfig`].
    pub fn new(config: MinifluxProviderConfig) -> Self {
        let client = MinifluxClient::new(config.server_url, config.api_token);
        Self { client }
    }

    /// Create a new provider from a pre-constructed [`MinifluxClient`].
    /// Useful for tests that want to inject a custom `reqwest::Client`.
    pub fn with_client(client: MinifluxClient) -> Self {
        Self { client }
    }

    /// Borrow the underlying API client (mainly for tests).
    pub fn client(&self) -> &MinifluxClient {
        &self.client
    }
}

#[async_trait]
impl Provider for MinifluxProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Miniflux"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        match self.client.me().await {
            Ok(user) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some(format!("Connected to Miniflux as {}", user.username)),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Miniflux health check failed: {e}")),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = Instant::now();
        // Miniflux owns sync server-side; we just count how many feeds exist as
        // a sanity probe.
        match self.client.list_feeds().await {
            Ok(feeds) => Ok(SyncResult {
                success: true,
                items_added: feeds.len() as u32,
                items_updated: 0,
                items_removed: 0,
                errors: Vec::new(),
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Ok(SyncResult {
                success: false,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![e.to_string()],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: false,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
            Action {
                id: "open_browser".to_string(),
                name: "Open in Browser".to_string(),
                description: "Open the article URL in the default browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show article preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy the article URL to the clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Read".to_string(),
                description: "Mark the article as read on the Miniflux server".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
            Action {
                id: "mark_unread".to_string(),
                name: "Mark as Unread".to_string(),
                description: "Mark the article as unread on the Miniflux server".to_string(),
                kind: ActionKind::MarkUnread,
                keyboard_shortcut: Some("u".to_string()),
            },
            Action {
                id: "save".to_string(),
                name: "Star Article".to_string(),
                description: "Toggle the bookmark/star flag on the Miniflux server".to_string(),
                kind: ActionKind::Save,
                keyboard_shortcut: Some("s".to_string()),
            },
        ])
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser => Ok(match &item.url {
                Some(url) => ActionResult {
                    success: true,
                    message: Some(format!("Opening {url}")),
                    data: Some(serde_json::json!({ "url": url })),
                },
                None => ActionResult {
                    success: false,
                    message: Some("Item has no URL".to_string()),
                    data: None,
                },
            }),
            ActionKind::CopyLink => Ok(match &item.url {
                Some(url) => ActionResult {
                    success: true,
                    message: Some("Copied link to clipboard".to_string()),
                    data: Some(serde_json::json!({ "url": url })),
                },
                None => ActionResult {
                    success: false,
                    message: Some("Item has no URL".to_string()),
                    data: None,
                },
            }),
            ActionKind::MarkRead | ActionKind::MarkUnread => {
                let entry_id = parse_item_id(&item.id).ok_or_else(|| {
                    StreamError::ItemNotFound(format!(
                        "item id is not a Miniflux entry id: {}",
                        item.id.as_str()
                    ))
                })?;
                let target = if action.kind == ActionKind::MarkRead {
                    "read"
                } else {
                    "unread"
                };
                self.client
                    .update_entries_status(&[entry_id], target)
                    .await?;
                Ok(ActionResult {
                    success: true,
                    message: Some(format!("Marked entry {entry_id} as {target}")),
                    data: Some(serde_json::json!({
                        "entry_id": entry_id,
                        "status": target,
                    })),
                })
            }
            ActionKind::Save | ActionKind::Unsave => {
                let entry_id = parse_item_id(&item.id).ok_or_else(|| {
                    StreamError::ItemNotFound(format!(
                        "item id is not a Miniflux entry id: {}",
                        item.id.as_str()
                    ))
                })?;
                self.client.toggle_bookmark(entry_id).await?;
                Ok(ActionResult {
                    success: true,
                    message: Some(format!("Toggled bookmark on entry {entry_id}")),
                    data: Some(serde_json::json!({ "entry_id": entry_id })),
                })
            }
            _ => Ok(ActionResult {
                success: true,
                message: Some(format!("Executed action: {}", action.name)),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl HasFeeds for MinifluxProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let feeds = self.client.list_feeds().await?;
        Ok(feeds.iter().map(feed_to_feed).collect())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let numeric =
            parse_feed_id(feed_id).ok_or_else(|| StreamError::StreamNotFound(feed_id.0.clone()))?;

        let mut filter = EntryFilter {
            feed_id: Some(numeric),
            limit: options.limit,
            offset: options.offset,
            order: Some("published_at".to_string()),
            direction: Some("desc".to_string()),
            ..Default::default()
        };
        if !options.include_read {
            filter.status = Some("unread".to_string());
        }
        if let Some(since) = options.since {
            filter.published_after = Some(since.timestamp());
        }

        let response = self.client.list_entries(&filter).await?;
        let stream_id = mapping::feed_stream_id(numeric);
        let items = response
            .entries
            .into_iter()
            .map(|entry| entry_to_item(&entry, stream_id.clone()))
            .collect();
        Ok(items)
    }
}

#[async_trait]
impl HasSavedItems for MinifluxProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let filter = EntryFilter {
            starred: Some(true),
            limit: options.limit,
            offset: options.offset,
            order: Some("changed_at".to_string()),
            direction: Some("desc".to_string()),
            ..Default::default()
        };
        let response = self.client.list_entries(&filter).await?;
        let stream_id = saved_stream_id();
        let items = response
            .entries
            .into_iter()
            .map(|entry| entry_to_item(&entry, stream_id.clone()))
            .collect();
        Ok(items)
    }

    async fn is_saved(&self, item_id: &ItemId) -> Result<bool> {
        let entry_id =
            parse_item_id(item_id).ok_or_else(|| StreamError::ItemNotFound(item_id.0.clone()))?;

        // The simplest reliable signal: list starred entries and check whether
        // this id is present. Miniflux exposes a single-entry GET as well, but
        // staying on the listing endpoint keeps the API surface narrow and the
        // wiremock test surface small.
        let filter = EntryFilter {
            starred: Some(true),
            limit: Some(500),
            ..Default::default()
        };
        let response = self.client.list_entries(&filter).await?;
        Ok(response.entries.iter().any(|e| e.id == entry_id))
    }

    async fn save_item(&self, item_id: &ItemId) -> Result<()> {
        let entry_id =
            parse_item_id(item_id).ok_or_else(|| StreamError::ItemNotFound(item_id.0.clone()))?;
        // `PUT /v1/entries/<id>/bookmark` toggles. To make `save_item` an
        // idempotent "ensure starred", first check current state.
        if !self.is_saved(item_id).await? {
            self.client.toggle_bookmark(entry_id).await?;
        }
        Ok(())
    }

    async fn unsave_item(&self, item_id: &ItemId) -> Result<()> {
        let entry_id =
            parse_item_id(item_id).ok_or_else(|| StreamError::ItemNotFound(item_id.0.clone()))?;
        if self.is_saved(item_id).await? {
            self.client.toggle_bookmark(entry_id).await?;
        }
        Ok(())
    }
}

// ============================================================================
// Tests (unit only — wiremock-driven integration tests live in tests/)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> MinifluxProvider {
        MinifluxProvider::new(MinifluxProviderConfig::new(
            "http://localhost:0",
            "test-token",
        ))
    }

    #[tokio::test]
    async fn provider_basics() {
        let p = provider();
        assert_eq!(p.id(), "miniflux");
        assert_eq!(p.name(), "Miniflux");
        let caps = p.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_saved_items);
        assert!(!caps.has_collections);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn open_browser_action_uses_url() {
        let p = provider();
        let item = Item {
            id: ItemId::new(PROVIDER_ID, "1"),
            stream_id: mapping::feed_stream_id(1),
            title: "x".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };
        let action = Action {
            id: "open_browser".to_string(),
            name: "Open in Browser".to_string(),
            description: String::new(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: None,
        };
        let result = p.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn open_browser_action_without_url_fails() {
        let p = provider();
        let item = Item {
            id: ItemId::new(PROVIDER_ID, "1"),
            stream_id: mapping::feed_stream_id(1),
            title: "x".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };
        let action = Action {
            id: "open_browser".to_string(),
            name: "Open in Browser".to_string(),
            description: String::new(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: None,
        };
        let result = p.execute_action(&item, &action).await.unwrap();
        assert!(!result.success);
    }

    #[test]
    fn build_feed_id_format() {
        // Round-tripping a Miniflux feed id through `feed_stream_id` and the
        // exposed feed-id format keeps the encoding consistent.
        let stream = mapping::feed_stream_id(99);
        assert_eq!(stream.as_str(), "miniflux:feed:99");
    }
}

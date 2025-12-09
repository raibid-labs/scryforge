//! JSON-RPC API handlers for scryforge-daemon.
//!
//! This module defines the RPC interface and provides implementations
//! that return dummy data for now (Phase 2 will wire up actual providers).

use chrono::Utc;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use scryforge_provider_core::{
    Collection, CollectionId, Item, ItemContent, ItemId, Stream, StreamId, StreamType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cache::Cache;
use crate::sync::{ProviderSyncState, SyncManager};

// Re-export search types for use in TUI
pub use serde_json::Value as JsonValue;

/// Response object for a saved item with provider metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedItemResponse {
    /// The saved item
    pub item: Item,
    /// List of provider IDs where this item is saved
    pub provider_ids: Vec<String>,
    /// When the item was saved (earliest save date)
    pub saved_at: String,
}

/// The main JSON-RPC API interface for Scryforge.
///
/// This trait defines all available RPC methods that clients can call.
#[rpc(server)]
pub trait ScryforgeApi {
    /// List all available streams across all providers.
    #[method(name = "streams.list")]
    async fn list_streams(&self) -> RpcResult<Vec<Stream>>;

    /// List items for a specific stream.
    #[method(name = "items.list")]
    async fn list_items(&self, stream_id: String) -> RpcResult<Vec<Item>>;

    /// Get sync status for all providers.
    #[method(name = "sync.status")]
    async fn sync_status(&self) -> RpcResult<HashMap<String, ProviderSyncState>>;

    /// Manually trigger a sync for a specific provider.
    #[method(name = "sync.trigger")]
    async fn sync_trigger(&self, provider_id: String) -> RpcResult<()>;

    /// Search items across all streams or within a specific stream.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query text
    /// * `filters` - Optional JSON object with filters:
    ///   - `stream_id`: Filter by specific stream
    ///   - `content_type`: Filter by content type (e.g., "article", "email")
    ///   - `is_read`: Filter by read status (boolean)
    ///   - `is_saved`: Filter by saved status (boolean)
    #[method(name = "search.query")]
    async fn search_query(&self, query: String, filters: Option<JsonValue>)
        -> RpcResult<Vec<Item>>;

    /// Mark an item as read.
    #[method(name = "items.mark_read")]
    async fn mark_item_read(&self, item_id: String) -> RpcResult<()>;

    /// Mark an item as unread.
    #[method(name = "items.mark_unread")]
    async fn mark_item_unread(&self, item_id: String) -> RpcResult<()>;

    /// Archive an item.
    #[method(name = "items.archive")]
    async fn archive_item(&self, item_id: String) -> RpcResult<()>;

    /// Save an item (bookmark/star).
    #[method(name = "items.save")]
    async fn save_item(&self, item_id: String) -> RpcResult<()>;

    /// Unsave an item (remove bookmark/star).
    #[method(name = "items.unsave")]
    async fn unsave_item(&self, item_id: String) -> RpcResult<()>;

    /// List all collections across all providers.
    #[method(name = "collections.list")]
    async fn list_collections(&self) -> RpcResult<Vec<Collection>>;

    /// Get items in a specific collection.
    #[method(name = "collections.items")]
    async fn get_collection_items(&self, collection_id: String) -> RpcResult<Vec<Item>>;

    /// Add an item to a collection.
    #[method(name = "collections.add_item")]
    async fn add_to_collection(&self, collection_id: String, item_id: String) -> RpcResult<()>;

    /// Remove an item from a collection.
    #[method(name = "collections.remove_item")]
    async fn remove_from_collection(&self, collection_id: String, item_id: String)
        -> RpcResult<()>;

    /// Create a new collection.
    #[method(name = "collections.create")]
    async fn create_collection(&self, name: String) -> RpcResult<Collection>;
}

/// Implementation of the Scryforge API.
///
/// Currently returns hardcoded dummy data. In Phase 2, this will
/// delegate to the ProviderRegistry to fetch real data.
pub struct ApiImpl<C: Cache + 'static> {
    sync_manager: Option<Arc<RwLock<SyncManager<C>>>>,
    cache: Option<Arc<C>>,
}

impl<C: Cache + 'static> ApiImpl<C> {
    pub fn new() -> Self {
        Self {
            sync_manager: None,
            cache: None,
        }
    }

    pub fn with_sync_manager(sync_manager: Arc<RwLock<SyncManager<C>>>) -> Self {
        Self {
            sync_manager: Some(sync_manager),
            cache: None,
        }
    }

    pub fn with_cache(cache: Arc<C>) -> Self {
        Self {
            sync_manager: None,
            cache: Some(cache),
        }
    }

    pub fn with_sync_manager_and_cache(
        sync_manager: Arc<RwLock<SyncManager<C>>>,
        cache: Arc<C>,
    ) -> Self {
        Self {
            sync_manager: Some(sync_manager),
            cache: Some(cache),
        }
    }

    /// Extract provider ID from a collection ID string.
    /// Format expected: "provider:collection-id"
    fn extract_provider_id(id: &str) -> Option<&str> {
        id.split(':').next()
    }

    /// Generate dummy streams for testing.
    fn generate_dummy_streams() -> Vec<Stream> {
        vec![
            Stream {
                id: StreamId::new("email", "inbox", "gmail"),
                name: "Gmail Inbox".to_string(),
                provider_id: "email-imap".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("📧".to_string()),
                unread_count: Some(5),
                total_count: Some(150),
                last_updated: Some(Utc::now()),
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("rss", "feed", "hackernews"),
                name: "Hacker News".to_string(),
                provider_id: "rss".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("📰".to_string()),
                unread_count: Some(42),
                total_count: Some(100),
                last_updated: Some(Utc::now()),
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("reddit", "home", "default"),
                name: "Reddit Home".to_string(),
                provider_id: "reddit".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("🔴".to_string()),
                unread_count: None,
                total_count: None,
                last_updated: Some(Utc::now()),
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("spotify", "collection", "liked"),
                name: "Liked Songs".to_string(),
                provider_id: "spotify".to_string(),
                stream_type: StreamType::SavedItems,
                icon: Some("💚".to_string()),
                unread_count: None,
                total_count: Some(523),
                last_updated: Some(Utc::now()),
                metadata: HashMap::new(),
            },
        ]
    }

    /// Generate dummy items for a stream.
    fn generate_dummy_items(stream_id: &str) -> Vec<Item> {
        // Return different items based on stream
        if stream_id.starts_with("email:") {
            vec![
                Item {
                    id: ItemId::new("email", "msg-001"),
                    stream_id: StreamId(stream_id.to_string()),
                    title: "Meeting tomorrow at 10am".to_string(),
                    content: ItemContent::Email {
                        subject: "Meeting tomorrow at 10am".to_string(),
                        body_text: Some(
                            "Hi,\n\nJust a reminder about our meeting tomorrow.\n\nBest,\nJohn"
                                .to_string(),
                        ),
                        body_html: None,
                        snippet: "Just a reminder about our meeting...".to_string(),
                    },
                    author: Some(scryforge_provider_core::Author {
                        name: "John Doe".to_string(),
                        email: Some("john@example.com".to_string()),
                        url: None,
                        avatar_url: None,
                    }),
                    published: Some(Utc::now()),
                    updated: None,
                    url: None,
                    thumbnail_url: None,
                    is_read: false,
                    is_saved: false,
                    tags: vec![],
                    metadata: HashMap::new(),
                },
                Item {
                    id: ItemId::new("email", "msg-002"),
                    stream_id: StreamId(stream_id.to_string()),
                    title: "Your order has shipped".to_string(),
                    content: ItemContent::Email {
                        subject: "Your order has shipped".to_string(),
                        body_text: Some(
                            "Your order #12345 has shipped and will arrive by Friday.".to_string(),
                        ),
                        body_html: None,
                        snippet: "Your order #12345 has shipped...".to_string(),
                    },
                    author: Some(scryforge_provider_core::Author {
                        name: "Shop Support".to_string(),
                        email: Some("support@shop.com".to_string()),
                        url: None,
                        avatar_url: None,
                    }),
                    published: Some(Utc::now()),
                    updated: None,
                    url: None,
                    thumbnail_url: None,
                    is_read: true,
                    is_saved: false,
                    tags: vec![],
                    metadata: HashMap::new(),
                },
            ]
        } else if stream_id.starts_with("rss:") {
            vec![
                Item {
                    id: ItemId::new("rss", "article-001"),
                    stream_id: StreamId(stream_id.to_string()),
                    title: "Show HN: A new Rust TUI framework".to_string(),
                    content: ItemContent::Article {
                        summary: Some(
                            "I've been working on a new TUI framework in Rust...".to_string(),
                        ),
                        full_content: None,
                    },
                    author: Some(scryforge_provider_core::Author {
                        name: "rustdev".to_string(),
                        email: None,
                        url: None,
                        avatar_url: None,
                    }),
                    published: Some(Utc::now()),
                    updated: None,
                    url: Some("https://news.ycombinator.com/item?id=123".to_string()),
                    thumbnail_url: None,
                    is_read: false,
                    is_saved: false,
                    tags: vec![],
                    metadata: HashMap::new(),
                },
                Item {
                    id: ItemId::new("rss", "article-002"),
                    stream_id: StreamId(stream_id.to_string()),
                    title: "Rust 1.75 Released".to_string(),
                    content: ItemContent::Article {
                        summary: Some(
                            "The Rust team is happy to announce a new version of Rust..."
                                .to_string(),
                        ),
                        full_content: None,
                    },
                    author: Some(scryforge_provider_core::Author {
                        name: "Rust Blog".to_string(),
                        email: None,
                        url: Some("https://blog.rust-lang.org".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now()),
                    updated: None,
                    url: Some("https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html".to_string()),
                    thumbnail_url: None,
                    is_read: false,
                    is_saved: false,
                    tags: vec![],
                    metadata: HashMap::new(),
                },
            ]
        } else if stream_id.starts_with("spotify:") {
            vec![Item {
                id: ItemId::new("spotify", "track-001"),
                stream_id: StreamId(stream_id.to_string()),
                title: "Example Song".to_string(),
                content: ItemContent::Track {
                    album: Some("Example Album".to_string()),
                    duration_ms: Some(210000),
                    artists: vec!["Example Artist".to_string()],
                },
                author: Some(scryforge_provider_core::Author {
                    name: "Example Artist".to_string(),
                    email: None,
                    url: None,
                    avatar_url: None,
                }),
                published: None,
                updated: None,
                url: Some("https://open.spotify.com/track/example".to_string()),
                thumbnail_url: None,
                is_read: false,
                is_saved: true,
                tags: vec![],
                metadata: HashMap::new(),
            }]
        } else {
            // Generic items for other streams
            vec![Item {
                id: ItemId::new("generic", "item-001"),
                stream_id: StreamId(stream_id.to_string()),
                title: "Example Item".to_string(),
                content: ItemContent::Generic {
                    body: Some("This is a generic item from the daemon API.".to_string()),
                },
                author: None,
                published: Some(Utc::now()),
                updated: None,
                url: None,
                thumbnail_url: None,
                is_read: false,
                is_saved: false,
                tags: vec![],
                metadata: HashMap::new(),
            }]
        }
    }
}

#[jsonrpsee::core::async_trait]
impl<C: Cache + 'static> ScryforgeApiServer for ApiImpl<C> {
    async fn list_streams(&self) -> RpcResult<Vec<Stream>> {
        Ok(Self::generate_dummy_streams())
    }

    async fn list_items(&self, stream_id: String) -> RpcResult<Vec<Item>> {
        Ok(Self::generate_dummy_items(&stream_id))
    }

    async fn sync_status(&self) -> RpcResult<HashMap<String, ProviderSyncState>> {
        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let states = manager.get_sync_states().await;
            Ok(states)
        } else {
            // If sync manager is not available, return empty status
            Ok(HashMap::new())
        }
    }

    async fn sync_trigger(&self, provider_id: String) -> RpcResult<()> {
        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            manager.trigger_sync(&provider_id).await.map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to trigger sync: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn search_query(
        &self,
        query: String,
        filters: Option<JsonValue>,
    ) -> RpcResult<Vec<Item>> {
        // If cache is available, use it for search
        if let Some(ref cache) = self.cache {
            // Parse filters from JSON
            let mut stream_id: Option<String> = None;
            let mut content_type: Option<String> = None;
            let mut is_read: Option<bool> = None;
            let mut is_saved: Option<bool> = None;

            if let Some(filter_obj) = filters {
                if let Some(stream) = filter_obj.get("stream_id").and_then(|v| v.as_str()) {
                    stream_id = Some(stream.to_string());
                }
                if let Some(ctype) = filter_obj.get("content_type").and_then(|v| v.as_str()) {
                    content_type = Some(ctype.to_string());
                }
                if let Some(read) = filter_obj.get("is_read").and_then(|v| v.as_bool()) {
                    is_read = Some(read);
                }
                if let Some(saved) = filter_obj.get("is_saved").and_then(|v| v.as_bool()) {
                    is_saved = Some(saved);
                }
            }

            cache
                .search_items(
                    &query,
                    stream_id.as_deref(),
                    content_type.as_deref(),
                    is_read,
                    is_saved,
                )
                .map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        -32000,
                        format!("Search failed: {}", e),
                        None::<()>,
                    )
                })
        } else {
            // If no cache available, return empty results
            Ok(Vec::new())
        }
    }

    async fn mark_item_read(&self, item_id: String) -> RpcResult<()> {
        if let Some(ref cache) = self.cache {
            let id = ItemId(item_id);
            cache.mark_read(&id, true).map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to mark item as read: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Cache not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn mark_item_unread(&self, item_id: String) -> RpcResult<()> {
        if let Some(ref cache) = self.cache {
            let id = ItemId(item_id);
            cache.mark_read(&id, false).map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to mark item as unread: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Cache not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn archive_item(&self, item_id: String) -> RpcResult<()> {
        if let Some(ref cache) = self.cache {
            let id = ItemId(item_id);
            cache.mark_archived(&id, true).map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to archive item: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Cache not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn save_item(&self, item_id: String) -> RpcResult<()> {
        if let Some(ref cache) = self.cache {
            let id = ItemId(item_id);
            cache.mark_starred(&id, true).map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to save item: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Cache not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn unsave_item(&self, item_id: String) -> RpcResult<()> {
        if let Some(ref cache) = self.cache {
            let id = ItemId(item_id);
            cache.mark_starred(&id, false).map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Failed to unsave item: {}", e),
                    None::<()>,
                )
            })
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Cache not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn list_collections(&self) -> RpcResult<Vec<Collection>> {
        use scryforge_provider_core::HasCollections;

        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let registry = manager.get_registry();

            let mut all_collections = Vec::new();

            // Iterate through all providers and collect their collections
            for provider_id in registry.list() {
                if let Some(provider) = registry.get(provider_id) {
                    // Check if provider supports collections
                    if provider.capabilities().has_collections {
                        // Downcast to HasCollections trait
                        if let Some(collections_provider) = provider
                            .as_any()
                            .downcast_ref::<provider_dummy::DummyProvider>(
                        ) {
                            match collections_provider.list_collections().await {
                                Ok(collections) => all_collections.extend(collections),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to list collections from provider {}: {}",
                                        provider_id,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }

            Ok(all_collections)
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn get_collection_items(&self, collection_id: String) -> RpcResult<Vec<Item>> {
        use scryforge_provider_core::HasCollections;

        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let registry = manager.get_registry();

            // Extract provider ID from collection ID
            let provider_id = Self::extract_provider_id(&collection_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32002,
                    "Invalid collection ID format".to_string(),
                    None::<()>,
                )
            })?;

            let provider = registry.get(provider_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32003,
                    format!("Provider '{}' not found", provider_id),
                    None::<()>,
                )
            })?;

            // Check if provider supports collections
            if !provider.capabilities().has_collections {
                return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32004,
                    format!("Provider '{}' does not support collections", provider_id),
                    None::<()>,
                ));
            }

            // Downcast to HasCollections trait
            if let Some(collections_provider) = provider
                .as_any()
                .downcast_ref::<provider_dummy::DummyProvider>()
            {
                collections_provider
                    .get_collection_items(&CollectionId(collection_id))
                    .await
                    .map_err(|e| {
                        jsonrpsee::types::ErrorObjectOwned::owned(
                            -32000,
                            format!("Failed to get collection items: {}", e),
                            None::<()>,
                        )
                    })
            } else {
                Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32005,
                    format!(
                        "Provider '{}' does not implement HasCollections",
                        provider_id
                    ),
                    None::<()>,
                ))
            }
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn add_to_collection(&self, collection_id: String, item_id: String) -> RpcResult<()> {
        use scryforge_provider_core::HasCollections;

        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let registry = manager.get_registry();

            // Extract provider ID from collection ID
            let provider_id = Self::extract_provider_id(&collection_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32002,
                    "Invalid collection ID format".to_string(),
                    None::<()>,
                )
            })?;

            let provider = registry.get(provider_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32003,
                    format!("Provider '{}' not found", provider_id),
                    None::<()>,
                )
            })?;

            // Check if provider supports collections
            if !provider.capabilities().has_collections {
                return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32004,
                    format!("Provider '{}' does not support collections", provider_id),
                    None::<()>,
                ));
            }

            // Downcast to HasCollections trait
            if let Some(collections_provider) = provider
                .as_any()
                .downcast_ref::<provider_dummy::DummyProvider>()
            {
                collections_provider
                    .add_to_collection(&CollectionId(collection_id), &ItemId(item_id))
                    .await
                    .map_err(|e| {
                        jsonrpsee::types::ErrorObjectOwned::owned(
                            -32000,
                            format!("Failed to add item to collection: {}", e),
                            None::<()>,
                        )
                    })
            } else {
                Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32005,
                    format!(
                        "Provider '{}' does not implement HasCollections",
                        provider_id
                    ),
                    None::<()>,
                ))
            }
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn remove_from_collection(
        &self,
        collection_id: String,
        item_id: String,
    ) -> RpcResult<()> {
        use scryforge_provider_core::HasCollections;

        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let registry = manager.get_registry();

            // Extract provider ID from collection ID
            let provider_id = Self::extract_provider_id(&collection_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32002,
                    "Invalid collection ID format".to_string(),
                    None::<()>,
                )
            })?;

            let provider = registry.get(provider_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32003,
                    format!("Provider '{}' not found", provider_id),
                    None::<()>,
                )
            })?;

            // Check if provider supports collections
            if !provider.capabilities().has_collections {
                return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32004,
                    format!("Provider '{}' does not support collections", provider_id),
                    None::<()>,
                ));
            }

            // Downcast to HasCollections trait
            if let Some(collections_provider) = provider
                .as_any()
                .downcast_ref::<provider_dummy::DummyProvider>()
            {
                collections_provider
                    .remove_from_collection(&CollectionId(collection_id), &ItemId(item_id))
                    .await
                    .map_err(|e| {
                        jsonrpsee::types::ErrorObjectOwned::owned(
                            -32000,
                            format!("Failed to remove item from collection: {}", e),
                            None::<()>,
                        )
                    })
            } else {
                Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32005,
                    format!(
                        "Provider '{}' does not implement HasCollections",
                        provider_id
                    ),
                    None::<()>,
                ))
            }
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }

    async fn create_collection(&self, name: String) -> RpcResult<Collection> {
        use scryforge_provider_core::HasCollections;

        if let Some(ref sync_manager) = self.sync_manager {
            let manager = sync_manager.read().await;
            let registry = manager.get_registry();

            // For now, create collection in the dummy provider
            // In the future, this should accept a provider_id parameter
            let provider_id = "dummy";

            let provider = registry.get(provider_id).ok_or_else(|| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    -32003,
                    format!("Provider '{}' not found", provider_id),
                    None::<()>,
                )
            })?;

            // Check if provider supports collections
            if !provider.capabilities().has_collections {
                return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32004,
                    format!("Provider '{}' does not support collections", provider_id),
                    None::<()>,
                ));
            }

            // Downcast to HasCollections trait
            if let Some(collections_provider) = provider
                .as_any()
                .downcast_ref::<provider_dummy::DummyProvider>()
            {
                collections_provider
                    .create_collection(&name)
                    .await
                    .map_err(|e| {
                        jsonrpsee::types::ErrorObjectOwned::owned(
                            -32000,
                            format!("Failed to create collection: {}", e),
                            None::<()>,
                        )
                    })
            } else {
                Err(jsonrpsee::types::ErrorObjectOwned::owned(
                    -32005,
                    format!(
                        "Provider '{}' does not implement HasCollections",
                        provider_id
                    ),
                    None::<()>,
                ))
            }
        } else {
            Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "Sync manager not available".to_string(),
                None::<()>,
            ))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::SqliteCache;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_cache() -> anyhow::Result<SqliteCache> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("test.db");
        let cache = SqliteCache::open_at(&path)?;
        std::mem::forget(temp_dir);
        Ok(cache)
    }

    fn create_test_item(id: &str) -> Item {
        Item {
            id: ItemId(id.to_string()),
            stream_id: StreamId("test:stream:1".to_string()),
            title: "Test Item".to_string(),
            content: ItemContent::Text("Test content".to_string()),
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_save_item() -> anyhow::Result<()> {
        let cache = Arc::new(create_test_cache()?);
        let api = ApiImpl::with_cache(cache.clone());

        // Create stream first (required for foreign key constraint)
        let stream = scryforge_provider_core::Stream {
            id: StreamId("test:stream:1".to_string()),
            name: "Test Stream".to_string(),
            provider_id: "test".to_string(),
            stream_type: scryforge_provider_core::StreamType::Feed,
            icon: None,
            unread_count: None,
            total_count: None,
            last_updated: None,
            metadata: HashMap::new(),
        };
        cache.upsert_streams(&[stream])?;

        // Create and insert a test item
        let item = create_test_item("test:item:1");
        cache.upsert_items(&[item.clone()])?;

        // Verify item is not saved initially
        let items = cache.get_items(&item.stream_id, None)?;
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_saved);

        // Save the item via RPC
        let result = ScryforgeApiServer::save_item(&api, "test:item:1".to_string()).await;
        assert!(result.is_ok());

        // Verify item is now saved
        let items = cache.get_items(&item.stream_id, None)?;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_saved);

        Ok(())
    }

    #[tokio::test]
    async fn test_unsave_item() -> anyhow::Result<()> {
        let cache = Arc::new(create_test_cache()?);
        let api = ApiImpl::with_cache(cache.clone());

        // Create stream first (required for foreign key constraint)
        let stream = scryforge_provider_core::Stream {
            id: StreamId("test:stream:1".to_string()),
            name: "Test Stream".to_string(),
            provider_id: "test".to_string(),
            stream_type: scryforge_provider_core::StreamType::Feed,
            icon: None,
            unread_count: None,
            total_count: None,
            last_updated: None,
            metadata: HashMap::new(),
        };
        cache.upsert_streams(&[stream])?;

        // Create and insert a test item that's already saved
        let mut item = create_test_item("test:item:1");
        item.is_saved = true;
        cache.upsert_items(&[item.clone()])?;

        // Verify item is saved initially
        let items = cache.get_items(&item.stream_id, None)?;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_saved);

        // Unsave the item via RPC
        let result = ScryforgeApiServer::unsave_item(&api, "test:item:1".to_string()).await;
        assert!(result.is_ok());

        // Verify item is now unsaved
        let items = cache.get_items(&item.stream_id, None)?;
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_saved);

        Ok(())
    }

    #[tokio::test]
    async fn test_save_item_without_cache() {
        let api = ApiImpl::<SqliteCache>::new();

        // Try to save without cache configured
        let result = ScryforgeApiServer::save_item(&api, "test:item:1".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unsave_item_without_cache() {
        let api = ApiImpl::<SqliteCache>::new();

        // Try to unsave without cache configured
        let result = ScryforgeApiServer::unsave_item(&api, "test:item:1".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_toggle_save_multiple_times() -> anyhow::Result<()> {
        let cache = Arc::new(create_test_cache()?);
        let api = ApiImpl::with_cache(cache.clone());

        // Create stream first (required for foreign key constraint)
        let stream = scryforge_provider_core::Stream {
            id: StreamId("test:stream:1".to_string()),
            name: "Test Stream".to_string(),
            provider_id: "test".to_string(),
            stream_type: scryforge_provider_core::StreamType::Feed,
            icon: None,
            unread_count: None,
            total_count: None,
            last_updated: None,
            metadata: HashMap::new(),
        };
        cache.upsert_streams(&[stream])?;

        // Create and insert a test item
        let item = create_test_item("test:item:1");
        cache.upsert_items(&[item.clone()])?;

        // Save
        ScryforgeApiServer::save_item(&api, "test:item:1".to_string()).await?;
        let items = cache.get_items(&item.stream_id, None)?;
        assert!(items[0].is_saved);

        // Unsave
        ScryforgeApiServer::unsave_item(&api, "test:item:1".to_string()).await?;
        let items = cache.get_items(&item.stream_id, None)?;
        assert!(!items[0].is_saved);

        // Save again
        ScryforgeApiServer::save_item(&api, "test:item:1".to_string()).await?;
        let items = cache.get_items(&item.stream_id, None)?;
        assert!(items[0].is_saved);

        Ok(())
    }
}

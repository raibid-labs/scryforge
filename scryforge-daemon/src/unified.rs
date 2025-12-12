//! Unified views for aggregating content across multiple providers.
//!
//! This module provides unified views that combine content from multiple providers,
//! such as:
//! - Unified "Saved Items" view showing all saved content
//! - Unified "Collections" view aggregating playlists, folders, and boards
//! - Unified "All Feeds" view aggregating items from all feed streams

use chrono::{DateTime, Utc};
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error};

use crate::cache::Cache;
use crate::registry::ProviderRegistry;
/// A saved item with tracking of which providers it's saved on.
#[derive(Debug, Clone)]
pub struct UnifiedSavedItem {
    /// The underlying item
    pub item: Item,
    /// List of provider IDs where this item is saved
    pub provider_ids: Vec<String>,
    /// When the item was saved (earliest save date across providers)
    pub saved_at: DateTime<Utc>,
}

/// Options for fetching unified saved items.
#[derive(Debug, Clone, Default)]
pub struct UnifiedSavedOptions {
    /// Sort order for results
    pub sort: SortOrder,
    /// Maximum number of items to return
    pub limit: Option<u32>,
    /// Number of items to skip
    pub offset: Option<u32>,
    /// Filter by specific provider
    pub provider_filter: Option<String>,
    /// Filter by content type
    pub content_type_filter: Option<String>,
}

/// Sort order for saved items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    /// Newest saved items first (default)
    #[default]
    SavedDateDesc,
    /// Oldest saved items first
    SavedDateAsc,
    /// Newest published items first
    PublishedDateDesc,
    /// Oldest published items first
    PublishedDateAsc,
}

/// Unified view for saved items across all providers.
pub struct UnifiedSavedView {
    registry: Arc<ProviderRegistry>,
}

impl UnifiedSavedView {
    /// Create a new unified saved items view.
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self { registry }
    }

    /// Aggregate saved items from all providers that implement HasSavedItems.
    ///
    /// This method:
    /// 1. Queries all providers that have saved items capability
    /// 2. Deduplicates items by URL (items saved in multiple providers)
    /// 3. Tracks which providers each item is saved on
    /// 4. Sorts and filters according to the provided options
    pub async fn get_all_saved_items(
        &self,
        options: UnifiedSavedOptions,
    ) -> Result<Vec<UnifiedSavedItem>> {
        let mut url_to_item: HashMap<String, UnifiedSavedItem> = HashMap::new();
        let mut items_without_url: Vec<UnifiedSavedItem> = Vec::new();

        // Iterate through all registered providers
        for provider_id in self.registry.list() {
            // Skip if provider filter is set and doesn't match
            if let Some(ref filter) = options.provider_filter {
                if provider_id != filter {
                    continue;
                }
            }

            let provider = match self.registry.get(provider_id) {
                Some(p) => p,
                None => continue,
            };

            // Check if provider has saved items capability
            if !provider.capabilities().has_saved_items {
                continue;
            }

            // Try to downcast to HasSavedItems trait
            // Note: In production, we'd need a better way to handle dynamic dispatch
            // For now, we'll fetch saved items through a common interface
            let saved_items = match self.fetch_saved_items_from_provider(&provider).await {
                Ok(items) => items,
                Err(e) => {
                    // Log error but continue with other providers
                    eprintln!("Failed to fetch saved items from {}: {}", provider_id, e);
                    continue;
                }
            };

            // Process items and deduplicate by URL
            for item in saved_items {
                // Apply content type filter if specified
                if let Some(ref type_filter) = options.content_type_filter {
                    if !self.matches_content_type(&item, type_filter) {
                        continue;
                    }
                }

                let saved_at = item.updated.or(item.published).unwrap_or_else(Utc::now);

                if let Some(ref url) = item.url {
                    // Item has URL - use for deduplication
                    url_to_item
                        .entry(url.clone())
                        .and_modify(|unified_item| {
                            // Item already exists - add this provider to the list
                            if !unified_item.provider_ids.contains(&provider_id.to_string()) {
                                unified_item.provider_ids.push(provider_id.to_string());
                            }
                            // Keep the earliest saved date
                            if saved_at < unified_item.saved_at {
                                unified_item.saved_at = saved_at;
                            }
                        })
                        .or_insert_with(|| UnifiedSavedItem {
                            item: item.clone(),
                            provider_ids: vec![provider_id.to_string()],
                            saved_at,
                        });
                } else {
                    // Item has no URL - can't deduplicate, add as separate item
                    items_without_url.push(UnifiedSavedItem {
                        item: item.clone(),
                        provider_ids: vec![provider_id.to_string()],
                        saved_at,
                    });
                }
            }
        }

        // Combine all items
        let mut all_items: Vec<UnifiedSavedItem> = url_to_item.into_values().collect();
        all_items.extend(items_without_url);

        // Sort items according to specified order
        self.sort_items(&mut all_items, options.sort);

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        let result: Vec<UnifiedSavedItem> = all_items
            .into_iter()
            .skip(offset)
            .take(limit.unwrap_or(usize::MAX))
            .collect();

        Ok(result)
    }

    /// Fetch saved items from a provider.
    ///
    /// This is a helper method that attempts to get saved items from a provider.
    /// In the current implementation, this is a placeholder that would need to be
    /// integrated with the actual provider system.
    async fn fetch_saved_items_from_provider(
        &self,
        _provider: &Arc<dyn Provider>,
    ) -> Result<Vec<Item>> {
        // TODO: Implement actual provider querying
        // For now, return empty list as providers need to implement HasSavedItems
        // and we need a way to dynamically cast to that trait
        Ok(Vec::new())
    }

    /// Check if an item matches the content type filter.
    fn matches_content_type(&self, item: &Item, type_filter: &str) -> bool {
        let item_type = match &item.content {
            ItemContent::Text(_) | ItemContent::Markdown(_) | ItemContent::Html(_) => "text",
            ItemContent::Email { .. } => "email",
            ItemContent::Article { .. } => "article",
            ItemContent::Video { .. } => "video",
            ItemContent::Track { .. } => "track",
            ItemContent::Task { .. } => "task",
            ItemContent::Event { .. } => "event",
            ItemContent::Bookmark { .. } => "bookmark",
            ItemContent::Generic { .. } => "generic",
        };

        item_type.eq_ignore_ascii_case(type_filter)
    }

    /// Sort items according to the specified order.
    fn sort_items(&self, items: &mut [UnifiedSavedItem], sort_order: SortOrder) {
        match sort_order {
            SortOrder::SavedDateDesc => {
                items.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
            }
            SortOrder::SavedDateAsc => {
                items.sort_by(|a, b| a.saved_at.cmp(&b.saved_at));
            }
            SortOrder::PublishedDateDesc => {
                items.sort_by(|a, b| {
                    let a_date = a.item.published.unwrap_or(a.saved_at);
                    let b_date = b.item.published.unwrap_or(b.saved_at);
                    b_date.cmp(&a_date)
                });
            }
            SortOrder::PublishedDateAsc => {
                items.sort_by(|a, b| {
                    let a_date = a.item.published.unwrap_or(a.saved_at);
                    let b_date = b.item.published.unwrap_or(b.saved_at);
                    a_date.cmp(&b_date)
                });
            }
        }
    }
}

// ============================================================================
// Unified Feeds View
// ============================================================================

/// Options for fetching unified feed items.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedFeedOptions {
    /// Sort order for items (default: newest first)
    pub sort: FeedSortOrder,
    /// Maximum number of items to return
    pub limit: Option<u32>,
    /// Number of items to skip (for pagination)
    pub offset: Option<u32>,
    /// Filter by specific provider IDs
    pub provider_filter: Option<Vec<String>>,
    /// Filter by content type
    pub content_type_filter: Option<String>,
    /// Filter by date range (items published after this date)
    pub date_from: Option<DateTime<Utc>>,
    /// Filter by date range (items published before this date)
    pub date_to: Option<DateTime<Utc>>,
    /// Filter by read status
    pub is_read: Option<bool>,
    /// Filter by saved status
    pub is_saved: Option<bool>,
}

/// Sort order for unified feeds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedSortOrder {
    /// Newest items first (by published date)
    #[default]
    NewestFirst,
    /// Oldest items first (by published date)
    OldestFirst,
    /// Alphabetically by title
    Alphabetical,
}

/// A unified view that aggregates feed items from all providers.
///
/// This struct provides methods to fetch and merge items from multiple
/// providers' feeds into a single, chronologically-sorted view.
pub struct UnifiedFeedsView<C: Cache> {
    cache: Arc<C>,
}

impl<C: Cache> UnifiedFeedsView<C> {
    /// Create a new unified feeds view with the given cache.
    pub fn new(cache: Arc<C>) -> Self {
        Self { cache }
    }

    /// Get all feed items from all providers, merged and sorted.
    ///
    /// This method:
    /// 1. Fetches all streams from the cache
    /// 2. For each feed stream, fetches its items
    /// 3. Merges all items into a single list
    /// 4. Applies filters and sorting
    /// 5. Adds provider metadata to each item
    ///
    /// # Arguments
    ///
    /// * `options` - Options for filtering and sorting the unified feed
    ///
    /// # Returns
    ///
    /// A vector of items from all providers, sorted and filtered according to options.
    pub fn get_all_items(&self, options: UnifiedFeedOptions) -> Result<Vec<Item>> {
        // Fetch all streams from cache
        let all_streams = self
            .cache
            .get_streams(None)
            .map_err(|e| StreamError::Internal(format!("Failed to fetch streams: {}", e)))?;

        // Filter to only feed-type streams (not collections or saved items)
        let feed_streams: Vec<_> = all_streams
            .iter()
            .filter(|stream| matches!(stream.stream_type, StreamType::Feed))
            .collect();

        // If provider filter is specified, apply it
        let feed_streams: Vec<_> = if let Some(ref provider_filter) = options.provider_filter {
            feed_streams
                .into_iter()
                .filter(|stream| provider_filter.contains(&stream.provider_id))
                .collect()
        } else {
            feed_streams
        };

        // Collect all items from all feed streams
        let mut all_items = Vec::new();
        for stream in feed_streams {
            match self.cache.get_items(&stream.id, None) {
                Ok(mut items) => {
                    // Add provider metadata to each item
                    for item in &mut items {
                        item.metadata
                            .insert("provider_id".to_string(), stream.provider_id.clone());
                        item.metadata
                            .insert("provider_name".to_string(), stream.provider_id.clone());
                        item.metadata
                            .insert("stream_name".to_string(), stream.name.clone());
                        if let Some(ref icon) = stream.icon {
                            item.metadata
                                .insert("provider_icon".to_string(), icon.clone());
                        }
                    }
                    all_items.extend(items);
                }
                Err(e) => {
                    // Log error but continue with other streams
                    tracing::warn!(
                        "Failed to fetch items for stream {}: {}",
                        stream.id.as_str(),
                        e
                    );
                }
            }
        }

        // Apply filters
        all_items = self.apply_filters(all_items, &options);

        // Sort items
        self.sort_items(&mut all_items, options.sort);

        // Apply pagination (offset and limit)
        let offset = options.offset.unwrap_or(0) as usize;
        let total_items = all_items.len();

        if offset >= total_items {
            return Ok(Vec::new());
        }

        let items = if let Some(limit) = options.limit {
            let end = std::cmp::min(offset + limit as usize, total_items);
            all_items[offset..end].to_vec()
        } else {
            all_items[offset..].to_vec()
        };

        Ok(items)
    }

    /// Apply filters to the list of items.
    fn apply_filters(&self, items: Vec<Item>, options: &UnifiedFeedOptions) -> Vec<Item> {
        items
            .into_iter()
            .filter(|item| {
                // Filter by content type
                if let Some(ref content_type) = options.content_type_filter {
                    let item_type = match &item.content {
                        ItemContent::Email { .. } => "Email",
                        ItemContent::Article { .. } => "Article",
                        ItemContent::Video { .. } => "Video",
                        ItemContent::Track { .. } => "Track",
                        ItemContent::Task { .. } => "Task",
                        ItemContent::Event { .. } => "Event",
                        ItemContent::Bookmark { .. } => "Bookmark",
                        ItemContent::Text(_) => "Text",
                        ItemContent::Markdown(_) => "Markdown",
                        ItemContent::Html(_) => "Html",
                        ItemContent::Generic { .. } => "Generic",
                    };
                    if item_type != content_type {
                        return false;
                    }
                }

                // Filter by date range
                if let Some(published) = item.published {
                    if let Some(date_from) = options.date_from {
                        if published < date_from {
                            return false;
                        }
                    }
                    if let Some(date_to) = options.date_to {
                        if published > date_to {
                            return false;
                        }
                    }
                }

                // Filter by read status
                if let Some(is_read) = options.is_read {
                    if item.is_read != is_read {
                        return false;
                    }
                }

                // Filter by saved status
                if let Some(is_saved) = options.is_saved {
                    if item.is_saved != is_saved {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Sort items according to the specified sort order.
    fn sort_items(&self, items: &mut [Item], sort: FeedSortOrder) {
        match sort {
            FeedSortOrder::NewestFirst => {
                items.sort_by(|a, b| {
                    // Sort by published date, newest first
                    // Items without published date go to the end
                    match (a.published, b.published) {
                        (Some(a_pub), Some(b_pub)) => b_pub.cmp(&a_pub),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    }
                });
            }
            FeedSortOrder::OldestFirst => {
                items.sort_by(|a, b| {
                    // Sort by published date, oldest first
                    // Items without published date go to the end
                    match (a.published, b.published) {
                        (Some(a_pub), Some(b_pub)) => a_pub.cmp(&b_pub),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    }
                });
            }
            FeedSortOrder::Alphabetical => {
                items.sort_by(|a, b| a.title.cmp(&b.title));
            }
        }
    }

    /// Get statistics about the unified feed.
    pub fn get_stats(&self) -> Result<UnifiedFeedStats> {
        let all_streams = self
            .cache
            .get_streams(None)
            .map_err(|e| StreamError::Internal(format!("Failed to fetch streams: {}", e)))?;

        let feed_streams: Vec<_> = all_streams
            .iter()
            .filter(|stream| matches!(stream.stream_type, StreamType::Feed))
            .collect();

        let mut stats = UnifiedFeedStats {
            total_streams: feed_streams.len(),
            total_items: 0,
            unread_items: 0,
            providers: HashMap::new(),
        };

        for stream in feed_streams {
            let items = self
                .cache
                .get_items(&stream.id, None)
                .map_err(|e| StreamError::Internal(format!("Failed to fetch items: {}", e)))?;

            stats.total_items += items.len();
            stats.unread_items += items.iter().filter(|item| !item.is_read).count();

            let provider_stats = stats
                .providers
                .entry(stream.provider_id.clone())
                .or_insert_with(|| ProviderStats {
                    provider_id: stream.provider_id.clone(),
                    stream_count: 0,
                    item_count: 0,
                });

            provider_stats.stream_count += 1;
            provider_stats.item_count += items.len();
        }

        Ok(stats)
    }
}

/// Statistics about the unified feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedFeedStats {
    /// Total number of feed streams
    pub total_streams: usize,
    /// Total number of items across all feeds
    pub total_items: usize,
    /// Number of unread items
    pub unread_items: usize,
    /// Per-provider statistics
    pub providers: HashMap<String, ProviderStats>,
}

/// Statistics for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    /// Provider ID
    pub provider_id: String,
    /// Number of streams from this provider
    pub stream_count: usize,
    /// Number of items from this provider
    pub item_count: usize,
}

// ============================================================================
// Unified Collections View
// ============================================================================

/// Metadata for a collection with provider information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCollectionMetadata {
    /// The underlying collection data
    pub collection: Collection,

    /// ID of the provider this collection belongs to
    pub provider_id: String,

    /// Human-readable name of the provider
    pub provider_name: String,

    /// When this collection metadata was last updated
    pub last_updated: DateTime<Utc>,

    /// Collection type for grouping (e.g., "playlist", "folder", "board")
    pub collection_type: String,
}

/// Sort order for collections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionSortOrder {
    /// Sort by name (A-Z)
    #[default]
    NameAsc,
    /// Sort by name (Z-A)
    NameDesc,
    /// Sort by item count (ascending)
    ItemCountAsc,
    /// Sort by item count (descending)
    ItemCountDesc,
    /// Sort by last update time (newest first)
    UpdatedDesc,
    /// Sort by last update time (oldest first)
    UpdatedAsc,
    /// Sort by provider name
    Provider,
}

/// Filters for collection queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionFilters {
    /// Filter by specific provider IDs
    pub provider_ids: Option<Vec<String>>,

    /// Filter by collection type
    pub collection_type: Option<String>,

    /// Filter to only editable collections
    pub editable_only: Option<bool>,

    /// Filter by minimum item count
    pub min_item_count: Option<u32>,
}

/// Unified view for collections across all providers.
///
/// This aggregates playlists, folders, and other collection types
/// from all providers that implement HasCollections.
pub struct UnifiedCollectionsView {
    registry: Arc<ProviderRegistry>,
}

impl UnifiedCollectionsView {
    /// Create a new unified collections view.
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self { registry }
    }

    /// Get all collections from all providers.
    ///
    /// This method fetches collections from each provider that supports
    /// the HasCollections capability, aggregates them, and returns
    /// a unified list with metadata.
    ///
    /// # Arguments
    ///
    /// * `sort` - How to sort the results
    /// * `filters` - Optional filters to apply
    ///
    /// # Returns
    ///
    /// A sorted and filtered list of collection metadata
    pub async fn get_all_collections(
        &self,
        sort: CollectionSortOrder,
        filters: Option<CollectionFilters>,
    ) -> Result<Vec<UnifiedCollectionMetadata>> {
        let mut all_collections = Vec::new();

        // Fetch collections from each provider
        for provider_id in self.registry.list() {
            // Apply provider filter if specified
            if let Some(ref filter) = filters {
                if let Some(ref provider_ids) = filter.provider_ids {
                    if !provider_ids.contains(&provider_id.to_string()) {
                        continue;
                    }
                }
            }

            let provider = match self.registry.get(provider_id) {
                Some(p) => p,
                None => continue,
            };

            // Check if provider supports collections
            if !provider.capabilities().has_collections {
                debug!(
                    "Provider {} does not support collections, skipping",
                    provider_id
                );
                continue;
            }

            // Fetch collections from this provider
            match self
                .fetch_provider_collections(provider_id, &provider)
                .await
            {
                Ok(collections) => {
                    debug!(
                        "Fetched {} collections from provider {}",
                        collections.len(),
                        provider_id
                    );
                    all_collections.extend(collections);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch collections from provider {}: {}",
                        provider_id, e
                    );
                }
            }
        }

        // Apply filters
        if let Some(ref filters) = filters {
            all_collections = self.apply_filters(all_collections, filters);
        }

        // Sort collections
        self.sort_collections(&mut all_collections, sort);

        Ok(all_collections)
    }

    /// Fetch collections from a specific provider.
    ///
    /// This is a helper method that attempts to call list_collections
    /// on providers that implement HasCollections.
    async fn fetch_provider_collections(
        &self,
        provider_id: &str,
        _provider: &Arc<dyn Provider>,
    ) -> Result<Vec<UnifiedCollectionMetadata>> {
        // TODO: Implement actual provider querying
        // This requires a way to dynamically cast to HasCollections trait
        // For now, return empty list as a placeholder
        debug!(
            "fetch_provider_collections for {} not yet fully implemented",
            provider_id
        );
        Ok(Vec::new())
    }

    /// Apply filters to a collection list.
    fn apply_filters(
        &self,
        collections: Vec<UnifiedCollectionMetadata>,
        filters: &CollectionFilters,
    ) -> Vec<UnifiedCollectionMetadata> {
        collections
            .into_iter()
            .filter(|c| {
                // Filter by provider IDs (already handled in get_all_collections)
                // Filter by collection type
                if let Some(ref collection_type) = filters.collection_type {
                    if &c.collection_type != collection_type {
                        return false;
                    }
                }

                // Filter by editable status
                if let Some(editable_only) = filters.editable_only {
                    if editable_only && !c.collection.is_editable {
                        return false;
                    }
                }

                // Filter by minimum item count
                if let Some(min_count) = filters.min_item_count {
                    if c.collection.item_count < min_count {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Sort collections according to the specified order.
    fn sort_collections(
        &self,
        collections: &mut [UnifiedCollectionMetadata],
        sort: CollectionSortOrder,
    ) {
        match sort {
            CollectionSortOrder::NameAsc => {
                collections.sort_by(|a, b| a.collection.name.cmp(&b.collection.name));
            }
            CollectionSortOrder::NameDesc => {
                collections.sort_by(|a, b| b.collection.name.cmp(&a.collection.name));
            }
            CollectionSortOrder::ItemCountAsc => {
                collections.sort_by_key(|c| c.collection.item_count);
            }
            CollectionSortOrder::ItemCountDesc => {
                collections.sort_by(|a, b| b.collection.item_count.cmp(&a.collection.item_count));
            }
            CollectionSortOrder::UpdatedDesc => {
                collections.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
            }
            CollectionSortOrder::UpdatedAsc => {
                collections.sort_by(|a, b| a.last_updated.cmp(&b.last_updated));
            }
            CollectionSortOrder::Provider => {
                collections.sort_by(|a, b| {
                    a.provider_name
                        .cmp(&b.provider_name)
                        .then(a.collection.name.cmp(&b.collection.name))
                });
            }
        }
    }

    /// Group collections by provider.
    ///
    /// Returns a map of provider ID to list of collection metadata.
    pub fn group_by_provider(
        &self,
        collections: Vec<UnifiedCollectionMetadata>,
    ) -> HashMap<String, Vec<UnifiedCollectionMetadata>> {
        let mut grouped: HashMap<String, Vec<UnifiedCollectionMetadata>> = HashMap::new();

        for collection in collections {
            grouped
                .entry(collection.provider_id.clone())
                .or_default()
                .push(collection);
        }

        grouped
    }

    /// Group collections by type.
    ///
    /// Returns a map of collection type to list of collection metadata.
    pub fn group_by_type(
        &self,
        collections: Vec<UnifiedCollectionMetadata>,
    ) -> HashMap<String, Vec<UnifiedCollectionMetadata>> {
        let mut grouped: HashMap<String, Vec<UnifiedCollectionMetadata>> = HashMap::new();

        for collection in collections {
            grouped
                .entry(collection.collection_type.clone())
                .or_default()
                .push(collection);
        }

        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Duration;

    // Mock provider for testing
    #[allow(dead_code)]
    struct MockSavedItemsProvider {
        id: String,
        items: Vec<Item>,
    }

    #[async_trait]
    impl Provider for MockSavedItemsProvider {
        fn id(&self) -> &'static str {
            Box::leak(self.id.clone().into_boxed_str())
        }

        fn name(&self) -> &'static str {
            "Mock Saved Items Provider"
        }

        async fn health_check(&self) -> Result<ProviderHealth> {
            Ok(ProviderHealth {
                is_healthy: true,
                message: None,
                last_sync: None,
                error_count: 0,
            })
        }

        async fn sync(&self) -> Result<SyncResult> {
            Ok(SyncResult {
                success: true,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![],
                duration_ms: 0,
            })
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                has_feeds: false,
                has_collections: false,
                has_saved_items: true,
                has_communities: false,
            }
        }

        async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
            Ok(vec![])
        }

        async fn execute_action(&self, _item: &Item, _action: &Action) -> Result<ActionResult> {
            Ok(ActionResult {
                success: true,
                message: None,
                data: None,
            })
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_unified_saved_view_creation() {
        let registry = Arc::new(ProviderRegistry::new());
        let _view = UnifiedSavedView::new(registry);
        // Just ensure it compiles and creates successfully
    }

    #[test]
    fn test_sort_order_default() {
        assert_eq!(SortOrder::default(), SortOrder::SavedDateDesc);
    }

    #[test]
    fn test_matches_content_type() {
        let registry = Arc::new(ProviderRegistry::new());
        let view = UnifiedSavedView::new(registry);

        let article_item = Item {
            id: ItemId::new("test", "1"),
            stream_id: StreamId::new("test", "feed", "1"),
            title: "Test Article".to_string(),
            content: ItemContent::Article {
                summary: Some("Summary".to_string()),
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com/article".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: true,
            tags: vec![],
            metadata: HashMap::new(),
        };

        assert!(view.matches_content_type(&article_item, "article"));
        assert!(view.matches_content_type(&article_item, "ARTICLE"));
        assert!(!view.matches_content_type(&article_item, "video"));
    }

    #[tokio::test]
    async fn test_sort_items_by_saved_date() {
        let registry = Arc::new(ProviderRegistry::new());
        let view = UnifiedSavedView::new(registry);

        let now = Utc::now();
        let item1 = Item {
            id: ItemId::new("test", "1"),
            stream_id: StreamId::new("test", "feed", "1"),
            title: "Item 1".to_string(),
            content: ItemContent::Generic { body: None },
            author: None,
            published: Some(now - Duration::hours(2)),
            updated: None,
            url: Some("https://example.com/1".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: true,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let item2 = Item {
            id: ItemId::new("test", "2"),
            stream_id: StreamId::new("test", "feed", "2"),
            title: "Item 2".to_string(),
            content: ItemContent::Generic { body: None },
            author: None,
            published: Some(now - Duration::hours(1)),
            updated: None,
            url: Some("https://example.com/2".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: true,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let mut items = vec![
            UnifiedSavedItem {
                item: item1.clone(),
                provider_ids: vec!["test".to_string()],
                saved_at: now - Duration::hours(2),
            },
            UnifiedSavedItem {
                item: item2.clone(),
                provider_ids: vec!["test".to_string()],
                saved_at: now - Duration::hours(1),
            },
        ];

        // Sort descending (newest first)
        view.sort_items(&mut items, SortOrder::SavedDateDesc);
        assert_eq!(items[0].item.id.as_str(), "test:2");
        assert_eq!(items[1].item.id.as_str(), "test:1");

        // Sort ascending (oldest first)
        view.sort_items(&mut items, SortOrder::SavedDateAsc);
        assert_eq!(items[0].item.id.as_str(), "test:1");
        assert_eq!(items[1].item.id.as_str(), "test:2");
    }
}

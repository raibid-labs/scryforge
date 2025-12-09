# Provider Development Guide

This guide walks through creating a new provider for Scryforge. Providers integrate external services (RSS feeds, email, Spotify, etc.) into the unified Scryforge interface.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Provider Trait](#provider-trait)
4. [Capability Traits](#capability-traits)
5. [OAuth Integration](#oauth-integration)
6. [Creating a Provider](#creating-a-provider)
7. [Testing](#testing)
8. [Registration](#registration)
9. [Best Practices](#best-practices)

## Overview

Providers in Scryforge are Rust crates that implement the `Provider` trait and one or more capability traits (`HasFeeds`, `HasCollections`, `HasSavedItems`, `HasCommunities`, `HasTasks`). The daemon loads providers and exposes their functionality through a unified JSON-RPC API.

**Location**: `providers/provider-{name}/`

**Core Crate**: `scryforge-provider-core`

## Prerequisites

Before creating a provider, familiarize yourself with:

1. The `Provider` base trait in `scryforge-provider-core`
2. Capability traits and their semantics
3. The dummy provider reference implementation (`providers/provider-dummy`)
4. Sigilforge authentication (if your provider requires OAuth)

## Provider Trait

All providers must implement the `Provider` trait:

```rust
use async_trait::async_trait;
use scryforge_provider_core::prelude::*;

#[async_trait]
pub trait Provider: Send + Sync {
    /// Unique identifier for this provider type (e.g., "rss", "spotify", "email-imap")
    fn id(&self) -> &'static str;

    /// Human-readable name for display (e.g., "RSS Feeds", "Spotify")
    fn name(&self) -> &'static str;

    /// Check provider health and connectivity
    async fn health_check(&self) -> Result<ProviderHealth>;

    /// Trigger a sync operation to fetch new data
    async fn sync(&self) -> Result<SyncResult>;

    /// Declare which capabilities this provider supports
    fn capabilities(&self) -> ProviderCapabilities;

    /// Get available actions for a specific item
    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>>;

    /// Execute an action on an item
    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult>;

    /// Support for downcasting to concrete types (for capability traits)
    fn as_any(&self) -> &dyn Any;
}
```

### Method Implementations

#### `id()` and `name()`

These identify your provider. The ID should be kebab-case and unique.

```rust
fn id(&self) -> &'static str {
    "my-service"
}

fn name(&self) -> &'static str {
    "My Service"
}
```

#### `health_check()`

Verify that your provider can connect to its external service.

```rust
async fn health_check(&self) -> Result<ProviderHealth> {
    match self.client.ping().await {
        Ok(_) => Ok(ProviderHealth {
            is_healthy: true,
            message: Some("Connected successfully".to_string()),
            last_sync: self.last_sync_time,
            error_count: 0,
        }),
        Err(e) => Ok(ProviderHealth {
            is_healthy: false,
            message: Some(format!("Connection failed: {}", e)),
            last_sync: self.last_sync_time,
            error_count: self.error_count,
        }),
    }
}
```

#### `sync()`

Fetch new data from your external service. This method is called periodically by the daemon.

```rust
async fn sync(&self) -> Result<SyncResult> {
    let start = std::time::Instant::now();
    let mut items_added = 0;
    let mut items_updated = 0;
    let mut errors = Vec::new();

    // Fetch data from external API
    match self.client.fetch_recent_items().await {
        Ok(new_items) => {
            items_added = new_items.len() as u32;
            // Cache or process items...
        }
        Err(e) => {
            errors.push(format!("Fetch failed: {}", e));
        }
    }

    Ok(SyncResult {
        success: errors.is_empty(),
        items_added,
        items_updated,
        items_removed: 0,
        errors,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}
```

#### `capabilities()`

Declare which capability traits your provider implements.

```rust
fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities {
        has_feeds: true,
        has_collections: false,
        has_saved_items: true,
        has_communities: false,
    }
}
```

#### `available_actions()` and `execute_action()`

Define what actions users can perform on items.

```rust
async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
    let mut actions = vec![
        Action {
            id: "open".to_string(),
            name: "Open".to_string(),
            description: "Open in browser".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        },
    ];

    if !item.is_read {
        actions.push(Action {
            id: "mark_read".to_string(),
            name: "Mark as Read".to_string(),
            description: "Mark this item as read".to_string(),
            kind: ActionKind::MarkRead,
            keyboard_shortcut: Some("r".to_string()),
        });
    }

    Ok(actions)
}

async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
    match action.kind {
        ActionKind::MarkRead => {
            // Mark item as read in external service (if supported)
            Ok(ActionResult {
                success: true,
                message: Some("Marked as read".to_string()),
                data: None,
            })
        }
        _ => Err(StreamError::Provider("Action not supported".to_string())),
    }
}
```

#### `as_any()`

Required for downcasting when accessing capability trait methods.

```rust
fn as_any(&self) -> &dyn Any {
    self
}
```

## Capability Traits

Capability traits define specialized functionality beyond the base `Provider` trait.

### HasFeeds

For providers with time-based streams (inboxes, RSS feeds, subreddit posts).

```rust
#[async_trait]
pub trait HasFeeds: Provider {
    /// List all available feeds
    async fn list_feeds(&self) -> Result<Vec<Feed>>;

    /// Get items from a specific feed
    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>>;
}
```

**Example Implementation**:

```rust
#[async_trait]
impl HasFeeds for MyProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let feeds = self.client.get_feeds().await?;

        Ok(feeds.into_iter().map(|f| Feed {
            id: FeedId(format!("my-service:{}", f.id)),
            name: f.name,
            description: Some(f.description),
            icon: Some("📰".to_string()),
            unread_count: f.unread_count,
            total_count: Some(f.total_count),
        }).collect())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let stream_id = StreamId(format!("my-service:feed:{}", feed_id.0));
        let mut items = self.client.get_feed_items(&feed_id.0).await?;

        // Apply filters
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

        // Apply pagination
        let offset = options.offset.unwrap_or(0) as usize;
        let items: Vec<_> = items.into_iter().skip(offset).collect();

        let items = if let Some(limit) = options.limit {
            items.into_iter().take(limit as usize).collect()
        } else {
            items
        };

        Ok(items)
    }
}
```

### HasCollections

For providers with named, ordered collections (playlists, bookmark folders).

```rust
#[async_trait]
pub trait HasCollections: Provider {
    /// List all collections
    async fn list_collections(&self) -> Result<Vec<Collection>>;

    /// Get items in a collection (ordered)
    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>>;

    /// Add an item to a collection
    async fn add_to_collection(&self, collection_id: &CollectionId, item_id: &ItemId) -> Result<()>;

    /// Remove an item from a collection
    async fn remove_from_collection(&self, collection_id: &CollectionId, item_id: &ItemId) -> Result<()>;

    /// Create a new collection
    async fn create_collection(&self, name: &str) -> Result<Collection>;
}
```

**Example Implementation**:

```rust
#[async_trait]
impl HasCollections for MyProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let collections = self.client.get_collections().await?;

        Ok(collections.into_iter().map(|c| Collection {
            id: CollectionId(format!("my-service:{}", c.id)),
            name: c.name,
            description: Some(c.description),
            icon: Some("📁".to_string()),
            item_count: c.item_count,
            is_editable: c.is_editable,
            owner: Some(c.owner),
        }).collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let items = self.client.get_collection_items(&collection_id.0).await?;
        Ok(items)
    }

    async fn add_to_collection(&self, collection_id: &CollectionId, item_id: &ItemId) -> Result<()> {
        self.client.add_to_collection(&collection_id.0, &item_id.0).await?;
        Ok(())
    }

    async fn remove_from_collection(&self, collection_id: &CollectionId, item_id: &ItemId) -> Result<()> {
        self.client.remove_from_collection(&collection_id.0, &item_id.0).await?;
        Ok(())
    }

    async fn create_collection(&self, name: &str) -> Result<Collection> {
        let collection = self.client.create_collection(name).await?;

        Ok(Collection {
            id: CollectionId(format!("my-service:{}", collection.id)),
            name: collection.name,
            description: None,
            icon: Some("📁".to_string()),
            item_count: 0,
            is_editable: true,
            owner: Some(self.user_id.clone()),
        })
    }
}
```

### HasSavedItems

For providers with bookmarked/liked/saved items.

```rust
#[async_trait]
pub trait HasSavedItems: Provider {
    /// Get all saved items
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>>;

    /// Check if a specific item is saved
    async fn is_saved(&self, item_id: &ItemId) -> Result<bool>;

    /// Save/bookmark an item
    async fn save_item(&self, item_id: &ItemId) -> Result<()>;

    /// Unsave/remove bookmark from an item
    async fn unsave_item(&self, item_id: &ItemId) -> Result<()>;
}
```

### HasCommunities

For providers with subscriptions (subreddits, channels, publications).

```rust
#[async_trait]
pub trait HasCommunities: Provider {
    /// List subscribed communities
    async fn list_communities(&self) -> Result<Vec<Community>>;

    /// Get details for a specific community
    async fn get_community(&self, id: &CommunityId) -> Result<Community>;
}
```

### HasTasks

For task management providers (Microsoft To Do, Todoist).

```rust
#[async_trait]
pub trait HasTasks: Provider {
    /// Mark a task as completed
    async fn complete_task(&self, task_id: &str) -> Result<()>;

    /// Mark a task as not completed
    async fn uncomplete_task(&self, task_id: &str) -> Result<()>;
}
```

## OAuth Integration

If your provider requires authentication, use the Sigilforge client for token management.

### Setup

Add dependencies to your `Cargo.toml`:

```toml
[dependencies]
scryforge-provider-core = { path = "../../crates/scryforge-provider-core", features = ["sigilforge"] }
scryforge-sigilforge-client = { path = "../../scryforge-sigilforge-client" }
```

### Implementation

```rust
use scryforge_provider_core::auth::TokenFetcher;
use std::sync::Arc;

pub struct MyProvider {
    token_fetcher: Arc<dyn TokenFetcher>,
    service_name: String,
    account_name: String,
}

impl MyProvider {
    pub fn new(
        token_fetcher: Arc<dyn TokenFetcher>,
        service_name: String,
        account_name: String,
    ) -> Self {
        Self {
            token_fetcher,
            service_name,
            account_name,
        }
    }

    async fn get_authenticated_client(&self) -> Result<MyServiceClient> {
        // Fetch OAuth token from Sigilforge
        let token = self.token_fetcher
            .fetch_token(&self.service_name, &self.account_name)
            .await
            .map_err(|e| StreamError::AuthRequired(format!(
                "Failed to fetch token: {}", e
            )))?;

        // Create authenticated API client
        Ok(MyServiceClient::with_token(token))
    }

    pub async fn make_api_call(&self) -> Result<Response> {
        let client = self.get_authenticated_client().await?;
        client.get_data().await
            .map_err(|e| StreamError::Network(format!("API call failed: {}", e)))
    }
}
```

### Testing with MockTokenFetcher

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::auth::MockTokenFetcher;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_with_mock_auth() {
        let mut tokens = HashMap::new();
        tokens.insert(
            ("my-service".to_string(), "test".to_string()),
            "mock_token_123".to_string(),
        );

        let token_fetcher = Arc::new(MockTokenFetcher::new(tokens));
        let provider = MyProvider::new(
            token_fetcher,
            "my-service".to_string(),
            "test".to_string(),
        );

        // Test provider methods...
    }
}
```

## Creating a Provider

### Step 1: Create Crate Structure

```bash
cd /home/beengud/raibid-labs/scryforge/providers
cargo new --lib provider-myservice
cd provider-myservice
```

### Step 2: Update `Cargo.toml`

```toml
[package]
name = "provider-myservice"
version = "0.1.0"
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
async-trait.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true

scryforge-provider-core = { path = "../../crates/scryforge-provider-core", features = ["sigilforge"] }
scryforge-sigilforge-client = { path = "../../scryforge-sigilforge-client" }

# Add service-specific dependencies
# reqwest = { version = "0.12", features = ["json"] }
```

### Step 3: Implement Provider

Create `src/lib.rs`:

```rust
use async_trait::async_trait;
use scryforge_provider_core::prelude::*;
use std::collections::HashMap;

pub struct MyServiceProvider {
    // Configuration and state
}

impl MyServiceProvider {
    pub fn new() -> Self {
        Self {
            // Initialize
        }
    }
}

#[async_trait]
impl Provider for MyServiceProvider {
    fn id(&self) -> &'static str {
        "myservice"
    }

    fn name(&self) -> &'static str {
        "My Service"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Implementation
        Ok(ProviderHealth {
            is_healthy: true,
            message: None,
            last_sync: None,
            error_count: 0,
        })
    }

    async fn sync(&self) -> Result<SyncResult> {
        // Implementation
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
            has_feeds: true,
            has_collections: false,
            has_saved_items: false,
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

// Implement capability traits as needed
#[async_trait]
impl HasFeeds for MyServiceProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        // Implementation
        Ok(vec![])
    }

    async fn get_feed_items(&self, _feed_id: &FeedId, _options: FeedOptions) -> Result<Vec<Item>> {
        // Implementation
        Ok(vec![])
    }
}
```

## Testing

### Unit Tests

Test your provider implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_basics() {
        let provider = MyServiceProvider::new();

        assert_eq!(provider.id(), "myservice");
        assert_eq!(provider.name(), "My Service");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
    }

    #[tokio::test]
    async fn test_health_check() {
        let provider = MyServiceProvider::new();
        let health = provider.health_check().await.unwrap();

        assert!(health.is_healthy);
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let provider = MyServiceProvider::new();
        let feeds = provider.list_feeds().await.unwrap();

        assert!(!feeds.is_empty());
    }

    #[tokio::test]
    async fn test_get_feed_items() {
        let provider = MyServiceProvider::new();
        let feeds = provider.list_feeds().await.unwrap();

        let options = FeedOptions {
            limit: Some(10),
            include_read: false,
            ..Default::default()
        };

        let items = provider.get_feed_items(&feeds[0].id, options).await.unwrap();
        assert!(items.len() <= 10);
    }
}
```

Run tests:

```bash
cargo test --package provider-myservice
```

## Registration

### Step 1: Add to Workspace

Update `/home/beengud/raibid-labs/scryforge/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "providers/provider-myservice",
]
```

### Step 2: Add to Daemon Dependencies

Update `/home/beengud/raibid-labs/scryforge/scryforge-daemon/Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...
provider-myservice = { path = "../providers/provider-myservice" }
```

### Step 3: Register in Daemon

Update `/home/beengud/raibid-labs/scryforge/scryforge-daemon/src/main.rs`:

```rust
use provider_myservice::MyServiceProvider;

fn register_providers(registry: &mut ProviderRegistry) {
    // Existing providers
    registry.register(provider_dummy::DummyProvider::new());

    // Add your provider
    registry.register(MyServiceProvider::new());
}
```

### Step 4: Verify Registration

```bash
cargo build --package scryforge-daemon
cargo run --package scryforge-daemon
```

Check logs for successful registration:
```
INFO scryforge_daemon: Registered provider: myservice (My Service)
```

## Best Practices

### 1. Error Handling

Use appropriate error types from `StreamError`:

```rust
// Network errors
Err(StreamError::Network(format!("Connection failed: {}", e)))

// Authentication errors
Err(StreamError::AuthRequired("Token expired".to_string()))

// Not found errors
Err(StreamError::ItemNotFound(item_id.to_string()))

// Rate limiting
Err(StreamError::RateLimited(retry_after_seconds))

// Generic provider errors
Err(StreamError::Provider(format!("API error: {}", e)))
```

### 2. ID Formatting

Use consistent ID formats:

```rust
// Stream IDs: provider:type:local_id
StreamId(format!("myservice:feed:{}", feed_id))

// Item IDs: provider:local_id
ItemId(format!("myservice:{}", item_id))

// Collection IDs: provider:collection_id
CollectionId(format!("myservice:collection:{}", coll_id))
```

### 3. Logging

Use structured logging:

```rust
use tracing::{debug, info, warn, error};

info!(provider = "myservice", "Starting sync");
debug!(feed_id = %feed.id, "Fetching feed items");
warn!(error = %e, "API call failed, will retry");
error!(error = %e, "Sync failed");
```

### 4. Rate Limiting

Respect API rate limits:

```rust
use tokio::time::{sleep, Duration};

async fn make_request(&self) -> Result<Response> {
    // Check rate limit
    if self.is_rate_limited() {
        let wait_time = self.get_rate_limit_reset_time();
        sleep(Duration::from_secs(wait_time)).await;
    }

    self.client.request().await
}
```

### 5. Caching

Avoid redundant API calls:

```rust
use std::sync::RwLock;

pub struct MyServiceProvider {
    cache: RwLock<HashMap<String, CachedData>>,
}

impl MyServiceProvider {
    async fn get_data(&self) -> Result<Data> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get("key") {
                if !cached.is_expired() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch from API
        let data = self.client.fetch_data().await?;

        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert("key".to_string(), CachedData::new(data.clone()));
        }

        Ok(data)
    }
}
```

### 6. Testing

Provide comprehensive tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test basic functionality
    #[tokio::test]
    async fn test_provider_id() { /* ... */ }

    // Test capability traits
    #[tokio::test]
    async fn test_has_feeds() { /* ... */ }

    // Test error handling
    #[tokio::test]
    async fn test_network_error() { /* ... */ }

    // Test filtering and pagination
    #[tokio::test]
    async fn test_feed_options() { /* ... */ }

    // Test authentication (with mocks)
    #[tokio::test]
    async fn test_auth_flow() { /* ... */ }
}
```

### 7. Documentation

Document your provider:

```rust
//! # provider-myservice
//!
//! Provider for My Service, integrating feeds and collections.
//!
//! ## Features
//!
//! - Fetch feeds from My Service API
//! - OAuth authentication via Sigilforge
//! - Rate limiting and error recovery
//!
//! ## Configuration
//!
//! Requires Sigilforge credentials: `myservice/{account}`

/// Provider for My Service.
///
/// Implements `HasFeeds` and `HasCollections` traits.
pub struct MyServiceProvider {
    // ...
}
```

## Reference Implementation

See `providers/provider-dummy/src/lib.rs` for a complete reference implementation demonstrating:

- Basic `Provider` trait implementation
- `HasFeeds` and `HasCollections` traits
- Internal state management
- Comprehensive test coverage
- Proper error handling

## Next Steps

1. Implement your provider following this guide
2. Add comprehensive tests
3. Register with the daemon
4. Test end-to-end with the TUI
5. Update `docs/PROVIDERS.md` with your provider's capabilities
6. Submit a pull request

## Resources

- [Architecture Documentation](./ARCHITECTURE.md)
- [Provider Capability Matrix](./PROVIDERS.md)
- [API Reference](./API_REFERENCE.md)
- [Dummy Provider Source](../providers/provider-dummy/src/lib.rs)
- [Provider Core Crate](../crates/scryforge-provider-core/src/lib.rs)

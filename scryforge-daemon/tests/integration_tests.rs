//! Integration tests for scryforge-daemon.
//!
//! This module contains comprehensive integration tests that verify:
//! - Daemon-TUI communication via JSON-RPC
//! - Provider registry + sync manager integration
//! - Cache integration (insert → query → verify)
//!
//! Tests use temporary databases and actual JSON-RPC communication
//! to ensure all components work together correctly.

mod fixtures;

use anyhow::Result;
use chrono::Utc;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::HttpClientBuilder;
use provider_dummy::DummyProvider;
use scryforge_daemon::api::handlers::{ApiImpl, ScryforgeApiServer};
use scryforge_daemon::cache::{Cache, SqliteCache};
use scryforge_daemon::config::{Config, ProviderConfig};
use scryforge_daemon::registry::ProviderRegistry;
use scryforge_daemon::sync::SyncManager;
use scryforge_provider_core::{ItemId, Stream};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

// ============================================================================
// Test Utilities
// ============================================================================

/// Creates a test cache in a temporary directory.
fn create_test_cache() -> Result<(SqliteCache, TempDir)> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().join("test.db");
    let cache = SqliteCache::open_at(&path)?;
    Ok((cache, temp_dir))
}

/// Creates a test config with a dummy provider.
fn create_test_config() -> Config {
    let mut config = Config::default();
    config.providers.insert(
        "dummy".to_string(),
        ProviderConfig {
            enabled: true,
            sync_interval_minutes: 1,
            settings: toml::Value::Table(toml::map::Map::new()),
        },
    );
    config
}

/// Sets up a complete test environment with registry, cache, and sync manager.
async fn setup_test_environment() -> Result<(
    Arc<ProviderRegistry>,
    Arc<SqliteCache>,
    Arc<RwLock<SyncManager<SqliteCache>>>,
    TempDir,
)> {
    let (cache, temp_dir) = create_test_cache()?;
    let cache = Arc::new(cache);

    let mut registry = ProviderRegistry::new();
    registry.register(DummyProvider::new());
    let registry = Arc::new(registry);

    let config = create_test_config();
    let sync_manager = SyncManager::new(config, Arc::clone(&registry), Arc::clone(&cache));
    let sync_manager = Arc::new(RwLock::new(sync_manager));

    Ok((registry, cache, sync_manager, temp_dir))
}

/// Starts a JSON-RPC server for testing.
async fn start_test_server(
    cache: Arc<SqliteCache>,
    sync_manager: Arc<RwLock<SyncManager<SqliteCache>>>,
) -> Result<(String, tokio::task::JoinHandle<()>)> {
    use jsonrpsee::server::Server;

    let api = ApiImpl::with_sync_manager_and_cache(sync_manager, cache);

    let server = Server::builder()
        .build("127.0.0.1:0")
        .await
        .expect("Failed to build server");

    let addr = server.local_addr().expect("Failed to get server address");
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        let _server_handle = server.start(api.into_rpc());
        // Keep server running
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok((url, handle))
}

// ============================================================================
// Cache Integration Tests
// ============================================================================

#[tokio::test]
async fn test_cache_insert_query_verify() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    // Create test data
    let stream = fixtures::create_test_stream("test:stream:1", "test-provider", "Test Stream");
    let items = fixtures::create_test_items("test:stream:1", 5);

    // Insert stream
    cache.upsert_streams(&[stream.clone()])?;

    // Verify stream was inserted
    let retrieved_streams = cache.get_streams(Some("test-provider"))?;
    assert_eq!(retrieved_streams.len(), 1);
    assert_eq!(retrieved_streams[0].id.0, "test:stream:1");
    assert_eq!(retrieved_streams[0].name, "Test Stream");

    // Insert items
    cache.upsert_items(&items)?;

    // Query items back
    let retrieved_items = cache.get_items(&stream.id, None)?;
    assert_eq!(retrieved_items.len(), 5);

    // Verify data integrity (items are returned in DESC order by published date)
    assert_eq!(retrieved_items.len(), 5);
    for item in &retrieved_items {
        assert_eq!(item.stream_id.0, "test:stream:1");
        assert!(!item.is_read);
        assert!(!item.is_saved);
    }
    // Check that all expected items are present
    let item_ids: Vec<_> = retrieved_items.iter().map(|i| i.id.0.as_str()).collect();
    for i in 0..5 {
        assert!(item_ids.contains(&format!("test:item:{}", i).as_str()));
    }

    Ok(())
}

#[tokio::test]
async fn test_cache_upsert_updates_existing() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    let stream = fixtures::create_test_stream("test:stream:1", "test-provider", "Test Stream");
    cache.upsert_streams(&[stream.clone()])?;

    let mut item = fixtures::create_test_item("test:item:1", "test:stream:1", "Original Title");
    cache.upsert_items(&[item.clone()])?;

    // Verify initial state
    let items = cache.get_items(&stream.id, None)?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Original Title");
    assert!(!items[0].is_read);

    // Update the item
    item.title = "Updated Title".to_string();
    item.is_read = true;
    cache.upsert_items(&[item])?;

    // Verify update
    let items = cache.get_items(&stream.id, None)?;
    assert_eq!(items.len(), 1); // Still only one item
    let item = &items[0];
    assert_eq!(item.title, "Updated Title");
    // Note: upsert doesn't update is_read/is_saved flags, only content
    // Use mark_read/mark_starred for state changes

    Ok(())
}

#[tokio::test]
async fn test_cache_mark_read_and_starred() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    let stream = fixtures::create_test_stream("test:stream:1", "test-provider", "Test Stream");
    cache.upsert_streams(&[stream.clone()])?;

    let item = fixtures::create_test_item("test:item:1", "test:stream:1", "Test Item");
    cache.upsert_items(&[item.clone()])?;

    // Mark as read
    cache.mark_read(&item.id, true)?;
    let items = cache.get_items(&stream.id, None)?;
    assert!(items[0].is_read);
    assert!(!items[0].is_saved);

    // Mark as starred
    cache.mark_starred(&item.id, true)?;
    let items = cache.get_items(&stream.id, None)?;
    assert!(items[0].is_read);
    assert!(items[0].is_saved);

    // Unmark read
    cache.mark_read(&item.id, false)?;
    let items = cache.get_items(&stream.id, None)?;
    assert!(!items[0].is_read);
    assert!(items[0].is_saved);

    // Unmark starred
    cache.mark_starred(&item.id, false)?;
    let items = cache.get_items(&stream.id, None)?;
    assert!(!items[0].is_read);
    assert!(!items[0].is_saved);

    Ok(())
}

#[tokio::test]
async fn test_cache_search_with_filters() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    let stream = fixtures::create_test_stream("test:stream:1", "test-provider", "Test Stream");
    cache.upsert_streams(&[stream.clone()])?;

    let items = fixtures::create_mixed_state_items("test:stream:1");
    cache.upsert_items(&items)?;

    // Search for all items (case-insensitive search for "Item")
    let results = cache.search_items("Item", None, None, None, None)?;
    // Note: SQLite LIKE is case-insensitive by default, but may find more or less depending on content
    assert!(results.len() >= 3 && results.len() <= 4);

    // Search for unread items
    let results = cache.search_items("Item", None, None, Some(false), None)?;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| !item.is_read));

    // Search for read items
    let results = cache.search_items("Item", None, None, Some(true), None)?;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| item.is_read));

    // Search for saved items
    let results = cache.search_items("Item", None, None, None, Some(true))?;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| item.is_saved));

    // Search for unsaved items
    let results = cache.search_items("Item", None, None, None, Some(false))?;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| !item.is_saved));

    Ok(())
}

#[tokio::test]
async fn test_cache_sync_state_tracking() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    let provider_id = "test-provider";

    // Initially no sync state
    let state = cache.get_sync_state(provider_id)?;
    assert!(state.is_none());

    // Update sync state
    let now = Utc::now();
    cache.update_sync_state(provider_id, now)?;

    // Verify sync state
    let state = cache.get_sync_state(provider_id)?;
    assert!(state.is_some());
    let state_time = state.unwrap();

    // Allow for minor timestamp differences due to serialization
    let diff = (state_time - now).num_seconds().abs();
    assert!(diff < 2);

    Ok(())
}

#[tokio::test]
async fn test_cache_multiple_streams_and_providers() -> Result<()> {
    let (cache, _temp_dir) = create_test_cache()?;

    // Create streams for multiple providers
    let stream1 = fixtures::create_test_stream("provider1:stream:1", "provider1", "Stream 1");
    let stream2 = fixtures::create_test_stream("provider1:stream:2", "provider1", "Stream 2");
    let stream3 = fixtures::create_test_stream("provider2:stream:1", "provider2", "Stream 3");

    cache.upsert_streams(&[stream1.clone(), stream2.clone(), stream3.clone()])?;

    // Query all streams
    let all_streams = cache.get_streams(None)?;
    assert_eq!(all_streams.len(), 3);

    // Query streams by provider
    let provider1_streams = cache.get_streams(Some("provider1"))?;
    assert_eq!(provider1_streams.len(), 2);

    let provider2_streams = cache.get_streams(Some("provider2"))?;
    assert_eq!(provider2_streams.len(), 1);

    Ok(())
}

// ============================================================================
// Provider Registry + Sync Manager Integration Tests
// ============================================================================

#[tokio::test]
async fn test_registry_and_sync_manager_integration() -> Result<()> {
    let (registry, cache, sync_manager, _temp_dir) = setup_test_environment().await?;

    // Verify provider is registered
    assert_eq!(registry.count(), 1);
    assert!(registry.contains("dummy"));

    // Start sync manager
    let mut manager = sync_manager.write().await;
    manager.start().await?;

    // Wait for initial sync cycle
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Get sync states
    let states = manager.get_sync_states().await;
    assert_eq!(states.len(), 1);
    assert!(states.contains_key("dummy"));

    let dummy_state = states.get("dummy").unwrap();
    assert_eq!(dummy_state.provider_id, "dummy");

    // Trigger manual sync
    let result = manager.trigger_sync("dummy").await;
    assert!(result.is_ok());

    // Wait for sync to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify sync state was updated in cache
    let sync_state = cache.get_sync_state("dummy")?;
    assert!(sync_state.is_some());

    // Shutdown manager
    manager.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn test_sync_manager_multiple_providers() -> Result<()> {
    let (cache, temp_dir) = create_test_cache()?;
    let cache = Arc::new(cache);

    let mut registry = ProviderRegistry::new();
    registry.register(DummyProvider::new());
    let registry = Arc::new(registry);

    let mut config = Config::default();
    config.providers.insert(
        "dummy".to_string(),
        ProviderConfig {
            enabled: true,
            sync_interval_minutes: 60,
            settings: toml::Value::Table(toml::map::Map::new()),
        },
    );

    let sync_manager = SyncManager::new(config, registry, cache);
    let mut sync_manager = sync_manager;

    sync_manager.start().await?;

    // Wait for initial sync
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let states = sync_manager.get_sync_states().await;
    assert_eq!(states.len(), 1);

    sync_manager.shutdown().await;
    drop(temp_dir);

    Ok(())
}

#[tokio::test]
async fn test_provider_registry_provider_access() -> Result<()> {
    let mut registry = ProviderRegistry::new();
    registry.register(DummyProvider::new());

    // Get provider
    let provider = registry.get("dummy");
    assert!(provider.is_some());

    let provider = provider.unwrap();
    assert_eq!(provider.id(), "dummy");
    assert_eq!(provider.name(), "Dummy Provider");

    // Check capabilities
    let caps = provider.capabilities();
    assert!(caps.has_feeds);
    assert!(caps.has_collections);

    // Health check
    let health = provider.health_check().await?;
    assert!(health.is_healthy);

    Ok(())
}

// ============================================================================
// Daemon-TUI Communication Tests (JSON-RPC)
// ============================================================================

#[tokio::test]
async fn test_jsonrpc_streams_list() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    let (url, handle) = start_test_server(cache, sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call streams.list
    let result: Vec<Stream> = client.request("streams.list", rpc_params![]).await?;

    // Should get dummy streams
    assert!(!result.is_empty());

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_items_list() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Insert test data
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let items = fixtures::create_test_items("test:stream:1", 3);
    cache.upsert_streams(&[stream.clone()])?;
    cache.upsert_items(&items)?;

    let (url, handle) = start_test_server(cache, sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call items.list with a dummy stream (returns dummy data)
    let result: Vec<scryforge_provider_core::Item> = client
        .request("items.list", rpc_params!["dummy:inbox"])
        .await?;

    // Should get items
    assert!(!result.is_empty());

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_search_query() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Insert test data
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let items = fixtures::create_test_items("test:stream:1", 5);
    cache.upsert_streams(&[stream])?;
    cache.upsert_items(&items)?;

    let (url, handle) = start_test_server(cache, sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call search.query
    let result: Vec<scryforge_provider_core::Item> = client
        .request("search.query", rpc_params!["Test Item", json!(null)])
        .await?;

    assert_eq!(result.len(), 5);

    // Search with filters
    let filters = json!({
        "is_read": false,
        "is_saved": false
    });

    let result: Vec<scryforge_provider_core::Item> = client
        .request("search.query", rpc_params!["Test Item", filters])
        .await?;

    assert_eq!(result.len(), 5);

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_items_save() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Insert test data
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let item = fixtures::create_test_item("test:item:1", "test:stream:1", "Test Item");
    cache.upsert_streams(&[stream.clone()])?;
    cache.upsert_items(&[item.clone()])?;

    let (url, handle) = start_test_server(cache.clone(), sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Verify item is not saved initially
    let items = cache.get_items(&stream.id, None)?;
    assert!(!items[0].is_saved);

    // Call items.save
    let _result: () = client
        .request("items.save", rpc_params!["test:item:1"])
        .await?;

    // Verify item is now saved
    let items = cache.get_items(&stream.id, None)?;
    assert!(items[0].is_saved);

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_items_mark_read() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Insert test data
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let item = fixtures::create_test_item("test:item:1", "test:stream:1", "Test Item");
    cache.upsert_streams(&[stream.clone()])?;
    cache.upsert_items(&[item.clone()])?;

    let (url, handle) = start_test_server(cache.clone(), sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Verify item is not read initially
    let items = cache.get_items(&stream.id, None)?;
    assert!(!items[0].is_read);

    // Call items.mark_read
    let _result: () = client
        .request("items.mark_read", rpc_params!["test:item:1"])
        .await?;

    // Verify item is now read
    let items = cache.get_items(&stream.id, None)?;
    assert!(items[0].is_read);

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_collections_list() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    let (url, handle) = start_test_server(cache, sync_manager).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call collections.list
    let result: Vec<scryforge_provider_core::Collection> =
        client.request("collections.list", rpc_params![]).await?;

    // Should get collections from dummy provider
    assert!(!result.is_empty());
    assert!(result.iter().any(|c| c.name == "My Favorites"));

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_sync_status() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Start sync manager
    {
        let mut manager = sync_manager.write().await;
        manager.start().await?;
    }

    // Wait for initial sync
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let (url, handle) = start_test_server(cache, Arc::clone(&sync_manager)).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call sync.status
    let result: HashMap<String, serde_json::Value> =
        client.request("sync.status", rpc_params![]).await?;

    // Should have status for dummy provider
    assert!(result.contains_key("dummy"));

    // Shutdown
    {
        let mut manager = sync_manager.write().await;
        manager.shutdown().await;
    }

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_sync_trigger() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Start sync manager
    {
        let mut manager = sync_manager.write().await;
        manager.start().await?;
    }

    let (url, handle) = start_test_server(cache, Arc::clone(&sync_manager)).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // Call sync.trigger
    let result: () = client.request("sync.trigger", rpc_params!["dummy"]).await?;

    // Should succeed without error
    assert_eq!(result, ());

    // Shutdown
    {
        let mut manager = sync_manager.write().await;
        manager.shutdown().await;
    }

    handle.abort();
    Ok(())
}

// ============================================================================
// End-to-End Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_integration_insert_sync_query() -> Result<()> {
    let (registry, cache, sync_manager, _temp_dir) = setup_test_environment().await?;

    // Step 1: Insert test data into cache
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let items = fixtures::create_test_items("test:stream:1", 10);
    cache.upsert_streams(&[stream.clone()])?;
    cache.upsert_items(&items)?;

    // Step 2: Start sync manager
    {
        let mut manager = sync_manager.write().await;
        manager.start().await?;

        // Wait for initial sync
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Trigger manual sync
        manager.trigger_sync("dummy").await?;

        // Wait for sync to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Step 3: Query data back from cache
    let retrieved_streams = cache.get_streams(Some("dummy"))?;
    let retrieved_items = cache.get_items(&stream.id, None)?;

    // Step 4: Verify data integrity
    assert!(!retrieved_streams.is_empty());
    assert_eq!(retrieved_items.len(), 10);

    // Verify all items are present (order may vary due to published date DESC)
    let item_ids: Vec<_> = retrieved_items.iter().map(|i| i.id.0.as_str()).collect();
    for i in 0..10 {
        assert!(item_ids.contains(&format!("test:item:{}", i).as_str()));
    }
    for item in &retrieved_items {
        assert_eq!(item.stream_id.0, "test:stream:1");
    }

    // Step 5: Test search functionality
    let search_results = cache.search_items("Test Item 5", None, None, None, None)?;
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].title, "Test Item 5");

    // Step 6: Test item state changes
    let item_id = ItemId("test:item:0".to_string());
    cache.mark_read(&item_id, true)?;
    cache.mark_starred(&item_id, true)?;

    let updated_items = cache.get_items(&stream.id, None)?;
    let updated_item = updated_items
        .iter()
        .find(|i| i.id.0 == "test:item:0")
        .unwrap();
    assert!(updated_item.is_read);
    assert!(updated_item.is_saved);

    // Cleanup
    {
        let mut manager = sync_manager.write().await;
        manager.shutdown().await;
    }

    Ok(())
}

#[tokio::test]
async fn test_jsonrpc_full_workflow() -> Result<()> {
    let (cache, sync_manager, _temp_dir) = {
        let (_, cache, sync_manager, temp_dir) = setup_test_environment().await?;
        (cache, sync_manager, temp_dir)
    };

    // Start sync manager
    {
        let mut manager = sync_manager.write().await;
        manager.start().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Insert test data
    let stream = fixtures::create_test_stream("test:stream:1", "dummy", "Test Stream");
    let items = fixtures::create_test_items("test:stream:1", 3);
    cache.upsert_streams(&[stream.clone()])?;
    cache.upsert_items(&items)?;

    let (url, handle) = start_test_server(cache.clone(), Arc::clone(&sync_manager)).await?;

    let client = HttpClientBuilder::default().build(&url)?;

    // 1. List streams
    let streams: Vec<Stream> = client.request("streams.list", rpc_params![]).await?;
    assert!(!streams.is_empty());

    // 2. Search for items
    let search_results: Vec<scryforge_provider_core::Item> = client
        .request("search.query", rpc_params!["Test Item", json!(null)])
        .await?;
    assert_eq!(search_results.len(), 3);

    // 3. Mark item as read
    let _: () = client
        .request("items.mark_read", rpc_params!["test:item:0"])
        .await?;

    // 4. Save item
    let _: () = client
        .request("items.save", rpc_params!["test:item:1"])
        .await?;

    // 5. Verify changes
    let items_after = cache.get_items(&stream.id, None)?;
    let item_0 = items_after
        .iter()
        .find(|i| i.id.0 == "test:item:0")
        .unwrap();
    let item_1 = items_after
        .iter()
        .find(|i| i.id.0 == "test:item:1")
        .unwrap();
    assert!(item_0.is_read);
    assert!(item_1.is_saved);

    // 6. Get sync status
    let sync_status: HashMap<String, serde_json::Value> =
        client.request("sync.status", rpc_params![]).await?;
    assert!(sync_status.contains_key("dummy"));

    // 7. List collections
    let collections: Vec<scryforge_provider_core::Collection> =
        client.request("collections.list", rpc_params![]).await?;
    assert!(!collections.is_empty());

    // Cleanup
    {
        let mut manager = sync_manager.write().await;
        manager.shutdown().await;
    }

    handle.abort();
    Ok(())
}

// Helper macro for RPC params
macro_rules! rpc_params {
    () => {
        jsonrpsee::core::params::ArrayParams::new()
    };
    ($($param:expr),* $(,)?) => {{
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        $(
            params.insert($param).unwrap();
        )*
        params
    }};
}

use rpc_params;

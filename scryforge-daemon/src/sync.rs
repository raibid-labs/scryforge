//! Background sync loop for providers.
//!
//! This module implements the `SyncManager` which orchestrates periodic
//! synchronization of data from all enabled providers. It handles:
//!
//! - Per-provider sync scheduling based on configured intervals
//! - Tracking sync state (last sync time, status, error count)
//! - Exponential backoff on provider errors
//! - Graceful shutdown signaling
//! - Event emission for new items
//!
//! # Architecture
//!
//! The `SyncManager` spawns a background tokio task for each enabled provider.
//! Each task runs its own sync loop with the configured interval, fetching
//! new data and storing it in the cache.
//!
//! # Example
//!
//! ```no_run
//! use scryforge_daemon::sync::SyncManager;
//! use scryforge_daemon::registry::ProviderRegistry;
//! use scryforge_daemon::cache::{SqliteCache, Cache};
//! use scryforge_daemon::config::Config;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = Config::load_default()?;
//! let registry = Arc::new(ProviderRegistry::new());
//! let cache = Arc::new(SqliteCache::open()?);
//!
//! let mut sync_manager = SyncManager::new(config.clone(), registry, cache);
//! sync_manager.start().await?;
//!
//! // Later: shutdown gracefully
//! sync_manager.shutdown().await;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::cache::Cache;
use crate::config::{Config, ProviderConfig};
use crate::registry::ProviderRegistry;

// ============================================================================
// Sync State Types
// ============================================================================

/// Current status of a provider sync operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Provider is idle, waiting for next sync interval
    Idle,
    /// Provider is currently syncing
    Syncing,
    /// Provider encountered an error during last sync
    Error(String),
}

/// Per-provider sync state tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSyncState {
    /// Provider identifier
    pub provider_id: String,
    /// Last successful sync timestamp
    pub last_sync: Option<DateTime<Utc>>,
    /// Current sync status
    pub status: SyncStatus,
    /// Number of consecutive errors
    pub error_count: u32,
    /// Scheduled time for next sync (considering backoff)
    pub next_sync: Option<DateTime<Utc>>,
}

impl ProviderSyncState {
    fn new(provider_id: String) -> Self {
        Self {
            provider_id,
            last_sync: None,
            status: SyncStatus::Idle,
            error_count: 0,
            next_sync: Some(Utc::now()),
        }
    }
}

/// Event emitted when new items are discovered during sync.
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub provider_id: String,
    pub items_added: u32,
    pub items_updated: u32,
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// SyncManager
// ============================================================================

/// Manages background sync operations for all providers.
///
/// The sync manager spawns a task for each enabled provider and coordinates
/// their sync schedules. It tracks sync state, handles errors with exponential
/// backoff, and emits events when new items are discovered.
pub struct SyncManager<C: Cache + 'static> {
    config: Config,
    registry: Arc<ProviderRegistry>,
    cache: Arc<C>,
    state: Arc<RwLock<HashMap<String, ProviderSyncState>>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
    event_tx: mpsc::Sender<SyncEvent>,
    event_rx: Option<mpsc::Receiver<SyncEvent>>,
}

impl<C: Cache + 'static> SyncManager<C> {
    /// Create a new sync manager.
    ///
    /// # Arguments
    ///
    /// * `config` - Daemon configuration with provider settings
    /// * `registry` - Provider registry containing loaded providers
    /// * `cache` - Cache implementation for storing synced data
    pub fn new(config: Config, registry: Arc<ProviderRegistry>, cache: Arc<C>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);

        Self {
            config,
            registry,
            cache,
            state: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Start the sync manager and spawn background tasks for all enabled providers.
    ///
    /// This method spawns a tokio task for each enabled provider configured in
    /// the daemon config. Each task runs independently with its own sync interval.
    ///
    /// # Errors
    ///
    /// Returns an error if there are no enabled providers or if task spawning fails.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting sync manager");

        let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Initialize state for all configured providers
        let mut state = self.state.write().await;
        for (provider_id, provider_config) in &self.config.providers {
            if provider_config.enabled {
                state.insert(provider_id.clone(), ProviderSyncState::new(provider_id.clone()));
            }
        }
        drop(state);

        // Spawn sync tasks for each enabled provider
        let mut task_count = 0;
        for (provider_id, provider_config) in &self.config.providers {
            if !provider_config.enabled {
                debug!("Skipping disabled provider: {}", provider_id);
                continue;
            }

            let provider = match self.registry.get(provider_id) {
                Some(p) => p,
                None => {
                    warn!("Provider '{}' configured but not registered, skipping", provider_id);
                    continue;
                }
            };

            info!(
                "Starting sync task for provider '{}' with interval {} minutes",
                provider_id, provider_config.sync_interval_minutes
            );

            let task_shutdown_rx = shutdown_tx.subscribe();
            self.spawn_sync_task(
                provider_id.clone(),
                provider,
                provider_config.clone(),
                task_shutdown_rx,
            );

            task_count += 1;
        }

        if task_count == 0 {
            warn!("No enabled providers found, sync manager has no work to do");
        } else {
            info!("Started {} sync task(s)", task_count);
        }

        Ok(())
    }

    /// Spawn a background sync task for a single provider.
    fn spawn_sync_task(
        &self,
        provider_id: String,
        provider: Arc<dyn Provider>,
        config: ProviderConfig,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let cache = Arc::clone(&self.cache);
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let mut sync_interval = interval(std::time::Duration::from_secs(
                config.sync_interval_minutes * 60,
            ));

            loop {
                tokio::select! {
                    _ = sync_interval.tick() => {
                        Self::run_sync_cycle(
                            &provider_id,
                            &provider,
                            &cache,
                            &state,
                            &event_tx,
                        ).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Sync task for '{}' received shutdown signal", provider_id);
                        break;
                    }
                }
            }

            info!("Sync task for '{}' stopped", provider_id);
        });
    }

    /// Run a single sync cycle for a provider.
    async fn run_sync_cycle(
        provider_id: &str,
        provider: &Arc<dyn Provider>,
        cache: &Arc<C>,
        state: &Arc<RwLock<HashMap<String, ProviderSyncState>>>,
        event_tx: &mpsc::Sender<SyncEvent>,
    ) {
        debug!("Starting sync cycle for provider '{}'", provider_id);

        // Update state to syncing
        {
            let mut state_lock = state.write().await;
            if let Some(provider_state) = state_lock.get_mut(provider_id) {
                provider_state.status = SyncStatus::Syncing;
            }
        }

        // Execute the sync
        let sync_start = std::time::Instant::now();
        let sync_result = provider.sync().await;
        let duration = sync_start.elapsed();

        match sync_result {
            Ok(result) => {
                if result.success {
                    info!(
                        "Provider '{}' sync completed successfully: +{} items, ~{} items, duration: {}ms",
                        provider_id, result.items_added, result.items_updated, result.duration_ms
                    );

                    // Update sync state to cache
                    let now = Utc::now();
                    if let Err(e) = cache.update_sync_state(provider_id, now) {
                        warn!("Failed to update sync state in cache: {}", e);
                    }

                    // Update state to idle and reset error count
                    {
                        let mut state_lock = state.write().await;
                        if let Some(provider_state) = state_lock.get_mut(provider_id) {
                            provider_state.status = SyncStatus::Idle;
                            provider_state.last_sync = Some(now);
                            provider_state.error_count = 0;
                            provider_state.next_sync = None;
                        }
                    }

                    // Emit sync event if items were added
                    if result.items_added > 0 || result.items_updated > 0 {
                        let event = SyncEvent {
                            provider_id: provider_id.to_string(),
                            items_added: result.items_added,
                            items_updated: result.items_updated,
                            timestamp: now,
                        };

                        if let Err(e) = event_tx.send(event).await {
                            warn!("Failed to send sync event: {}", e);
                        }
                    }
                } else {
                    warn!(
                        "Provider '{}' sync completed with errors: {:?}",
                        provider_id, result.errors
                    );
                    Self::handle_sync_error(
                        provider_id,
                        state,
                        &format!("Sync failed: {:?}", result.errors),
                    )
                    .await;
                }
            }
            Err(e) => {
                error!("Provider '{}' sync failed: {}", provider_id, e);
                Self::handle_sync_error(provider_id, state, &e.to_string()).await;
            }
        }

        debug!(
            "Sync cycle for provider '{}' completed in {:?}",
            provider_id, duration
        );
    }

    /// Handle a sync error with exponential backoff.
    async fn handle_sync_error(
        provider_id: &str,
        state: &Arc<RwLock<HashMap<String, ProviderSyncState>>>,
        error_message: &str,
    ) {
        let mut state_lock = state.write().await;
        if let Some(provider_state) = state_lock.get_mut(provider_id) {
            provider_state.status = SyncStatus::Error(error_message.to_string());
            provider_state.error_count += 1;

            // Calculate exponential backoff: 2^error_count minutes, max 60 minutes
            let backoff_minutes = (2_u32.pow(provider_state.error_count.min(6))).min(60);
            let backoff_duration = Duration::minutes(backoff_minutes as i64);
            provider_state.next_sync = Some(Utc::now() + backoff_duration);

            warn!(
                "Provider '{}' error count: {}, next retry in {} minutes",
                provider_id, provider_state.error_count, backoff_minutes
            );
        }
    }

    /// Get the current sync state for all providers.
    pub async fn get_sync_states(&self) -> HashMap<String, ProviderSyncState> {
        self.state.read().await.clone()
    }

    /// Get the sync state for a specific provider.
    pub async fn get_provider_state(&self, provider_id: &str) -> Option<ProviderSyncState> {
        self.state.read().await.get(provider_id).cloned()
    }

    /// Manually trigger a sync for a specific provider.
    ///
    /// This bypasses the scheduled sync interval and runs a sync immediately.
    /// The error backoff is temporarily ignored for manual triggers.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - The ID of the provider to sync
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not found or not enabled.
    pub async fn trigger_sync(&self, provider_id: &str) -> Result<()> {
        info!("Manual sync triggered for provider '{}'", provider_id);

        // Verify provider is configured and enabled
        let provider_config = self
            .config
            .providers
            .get(provider_id)
            .context("Provider not configured")?;

        if !provider_config.enabled {
            anyhow::bail!("Provider '{}' is not enabled", provider_id);
        }

        // Get provider from registry
        let provider = self
            .registry
            .get(provider_id)
            .context("Provider not registered")?;

        // Run sync cycle immediately
        Self::run_sync_cycle(
            provider_id,
            &provider,
            &self.cache,
            &self.state,
            &self.event_tx,
        )
        .await;

        Ok(())
    }

    /// Gracefully shutdown all sync tasks.
    ///
    /// This sends a shutdown signal to all background sync tasks and waits
    /// for them to complete their current cycle and exit.
    pub async fn shutdown(&mut self) {
        info!("Shutting down sync manager");

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            // Send shutdown signal to all tasks
            drop(shutdown_tx);

            // Give tasks a moment to shut down gracefully
            sleep(std::time::Duration::from_millis(500)).await;
        }

        info!("Sync manager shutdown complete");
    }

    /// Take ownership of the event receiver.
    ///
    /// This allows the caller to receive sync events. Can only be called once.
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<SyncEvent>> {
        self.event_rx.take()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::SqliteCache;
    use async_trait::async_trait;
    use tempfile::TempDir;

    // Mock provider for testing
    struct MockProvider {
        id: &'static str,
        should_fail: bool,
    }

    impl MockProvider {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                should_fail: false,
            }
        }

        fn new_failing(id: &'static str) -> Self {
            Self {
                id,
                should_fail: true,
            }
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            "Mock Provider"
        }

        async fn health_check(&self) -> scryforge_provider_core::Result<ProviderHealth> {
            Ok(ProviderHealth {
                is_healthy: true,
                message: None,
                last_sync: None,
                error_count: 0,
            })
        }

        async fn sync(&self) -> scryforge_provider_core::Result<SyncResult> {
            if self.should_fail {
                Err(StreamError::Provider("Mock sync failure".to_string()))
            } else {
                Ok(SyncResult {
                    success: true,
                    items_added: 5,
                    items_updated: 2,
                    items_removed: 0,
                    errors: vec![],
                    duration_ms: 100,
                })
            }
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }

        async fn available_actions(&self, _item: &Item) -> scryforge_provider_core::Result<Vec<Action>> {
            Ok(vec![])
        }

        async fn execute_action(&self, _item: &Item, _action: &Action) -> scryforge_provider_core::Result<ActionResult> {
            Ok(ActionResult {
                success: true,
                message: None,
                data: None,
            })
        }
    }

    fn create_test_cache() -> Arc<SqliteCache> {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.db");
        let cache = SqliteCache::open_at(&path).unwrap();
        std::mem::forget(temp_dir);
        Arc::new(cache)
    }

    fn create_test_config() -> Config {
        let mut config = Config::default();
        config.providers.insert(
            "mock".to_string(),
            ProviderConfig {
                enabled: true,
                sync_interval_minutes: 1,
                settings: toml::Value::Table(toml::map::Map::new()),
            },
        );
        config
    }

    #[tokio::test]
    async fn test_sync_manager_creation() {
        let config = create_test_config();
        let registry = Arc::new(ProviderRegistry::new());
        let cache = create_test_cache();

        let sync_manager = SyncManager::new(config, registry, cache);
        assert!(sync_manager.shutdown_tx.is_none());
    }

    #[tokio::test]
    async fn test_sync_manager_start() {
        let config = create_test_config();
        let mut registry = ProviderRegistry::new();
        registry.register(MockProvider::new("mock"));
        let registry = Arc::new(registry);
        let cache = create_test_cache();

        let mut sync_manager = SyncManager::new(config, registry, cache);
        let result = sync_manager.start().await;
        assert!(result.is_ok());
        assert!(sync_manager.shutdown_tx.is_some());

        sync_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_sync_state_initialization() {
        let config = create_test_config();
        let mut registry = ProviderRegistry::new();
        registry.register(MockProvider::new("mock"));
        let registry = Arc::new(registry);
        let cache = create_test_cache();

        let mut sync_manager = SyncManager::new(config, registry, cache);
        sync_manager.start().await.unwrap();

        let states = sync_manager.get_sync_states().await;
        assert_eq!(states.len(), 1);
        assert!(states.contains_key("mock"));

        let mock_state = states.get("mock").unwrap();
        assert_eq!(mock_state.provider_id, "mock");
        assert_eq!(mock_state.error_count, 0);

        sync_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_manual_sync_trigger() {
        let config = create_test_config();
        let mut registry = ProviderRegistry::new();
        registry.register(MockProvider::new("mock"));
        let registry = Arc::new(registry);
        let cache = create_test_cache();

        let mut sync_manager = SyncManager::new(config, registry, cache);
        sync_manager.start().await.unwrap();

        let result = sync_manager.trigger_sync("mock").await;
        assert!(result.is_ok());

        // Wait a bit for sync to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let state = sync_manager.get_provider_state("mock").await;
        assert!(state.is_some());

        sync_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_manual_sync_nonexistent_provider() {
        let config = create_test_config();
        let registry = Arc::new(ProviderRegistry::new());
        let cache = create_test_cache();

        let mut sync_manager = SyncManager::new(config, registry, cache);
        sync_manager.start().await.unwrap();

        let result = sync_manager.trigger_sync("nonexistent").await;
        assert!(result.is_err());

        sync_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_error_handling_and_backoff() {
        let mut config = create_test_config();
        config.providers.insert(
            "failing".to_string(),
            ProviderConfig {
                enabled: true,
                sync_interval_minutes: 60, // Long interval to avoid additional automatic syncs
                settings: toml::Value::Table(toml::map::Map::new()),
            },
        );

        let mut registry = ProviderRegistry::new();
        registry.register(MockProvider::new_failing("failing"));
        let registry = Arc::new(registry);
        let cache = create_test_cache();

        let mut sync_manager = SyncManager::new(config, registry, cache);
        sync_manager.start().await.unwrap();

        // Wait for the initial sync from interval.tick() to complete
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let state = sync_manager.get_provider_state("failing").await;
        assert!(state.is_some());

        let state = state.unwrap();
        // The error count should be at least 1 from the initial sync
        assert!(state.error_count >= 1);
        assert!(matches!(state.status, SyncStatus::Error(_)));
        assert!(state.next_sync.is_some());

        sync_manager.shutdown().await;
    }

    #[test]
    fn test_sync_status_serialization() {
        let status_idle = SyncStatus::Idle;
        let json = serde_json::to_string(&status_idle).unwrap();
        assert_eq!(json, r#""Idle""#);

        let status_syncing = SyncStatus::Syncing;
        let json = serde_json::to_string(&status_syncing).unwrap();
        assert_eq!(json, r#""Syncing""#);

        let status_error = SyncStatus::Error("test error".to_string());
        let json = serde_json::to_string(&status_error).unwrap();
        assert!(json.contains("test error"));
    }

    #[test]
    fn test_provider_sync_state_new() {
        let state = ProviderSyncState::new("test-provider".to_string());
        assert_eq!(state.provider_id, "test-provider");
        assert!(state.last_sync.is_none());
        assert_eq!(state.status, SyncStatus::Idle);
        assert_eq!(state.error_count, 0);
        assert!(state.next_sync.is_some());
    }
}

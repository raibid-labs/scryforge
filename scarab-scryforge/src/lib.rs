//! Scarab plugin for Scryforge integration.
//!
//! This plugin connects to the scryforge-daemon via JSON-RPC and provides
//! status bar integration and menu actions for Scarab terminal.

pub mod client;
pub mod status;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scarab_plugin_api::{
    menu::{MenuAction, MenuItem},
    Plugin, PluginContext, PluginError, PluginMetadata, Result,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use client::ScryforgeClient;

/// The Scryforge plugin for Scarab terminal.
///
/// This plugin provides integration with scryforge-daemon, showing unread
/// counts in the status bar and providing menu actions for syncing and
/// marking items as read.
pub struct ScryforgePlugin {
    /// Plugin metadata
    metadata: PluginMetadata,

    /// JSON-RPC client for scryforge-daemon
    client: Arc<RwLock<Option<ScryforgeClient>>>,

    /// Current unread count
    unread_count: Arc<RwLock<usize>>,

    /// Last sync timestamp
    last_sync: Arc<RwLock<Option<DateTime<Utc>>>>,

    /// Health status of daemon connection
    is_healthy: Arc<RwLock<bool>>,

    /// Daemon URL
    daemon_url: String,
}

impl Default for ScryforgePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ScryforgePlugin {
    /// Create a new Scryforge plugin instance.
    pub fn new() -> Self {
        let metadata = PluginMetadata::new(
            "scryforge",
            env!("CARGO_PKG_VERSION"),
            "Scryforge integration - unified feed reader status and controls",
            "raibid-labs",
        )
        .with_emoji("📬")
        .with_color("#a6e3a1") // catppuccin mocha green
        .with_catchphrase("Stay informed, stay focused");

        Self {
            metadata,
            client: Arc::new(RwLock::new(None)),
            unread_count: Arc::new(RwLock::new(0)),
            last_sync: Arc::new(RwLock::new(None)),
            is_healthy: Arc::new(RwLock::new(false)),
            daemon_url: "http://127.0.0.1:3030".to_string(),
        }
    }

    /// Create a new plugin with a custom daemon URL.
    pub fn with_daemon_url(url: impl Into<String>) -> Self {
        let mut plugin = Self::new();
        plugin.daemon_url = url.into();
        plugin
    }

    /// Update the unread count from the daemon.
    async fn update_unread_count(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        if let Some(client) = client_guard.as_ref() {
            match client.get_unread_count().await {
                Ok(count) => {
                    let mut unread = self.unread_count.write().await;
                    *unread = count;
                    debug!("Updated unread count: {}", count);
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to get unread count: {}", e);
                    *self.is_healthy.write().await = false;
                    Err(PluginError::Other(anyhow::anyhow!(e)))
                }
            }
        } else {
            Err(PluginError::LoadError("Client not connected".to_string()))
        }
    }

    /// Update sync status from the daemon.
    async fn update_sync_status(&self) -> Result<()> {
        let client_guard = self.client.read().await;
        if let Some(client) = client_guard.as_ref() {
            match client.get_sync_status().await {
                Ok(status) => {
                    // Find the most recent sync time across all providers
                    let most_recent = status
                        .values()
                        .filter_map(|s| s.last_sync)
                        .max();

                    if let Some(sync_time) = most_recent {
                        *self.last_sync.write().await = Some(sync_time);
                        debug!("Updated last sync time: {}", sync_time);
                    }
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to get sync status: {}", e);
                    Err(PluginError::Other(anyhow::anyhow!(e)))
                }
            }
        } else {
            Err(PluginError::LoadError("Client not connected".to_string()))
        }
    }

    /// Perform a health check on the daemon connection.
    async fn health_check(&self) -> bool {
        let client_guard = self.client.read().await;
        if let Some(client) = client_guard.as_ref() {
            client.health_check().await
        } else {
            false
        }
    }

    /// Start background task to periodically update status.
    async fn start_background_updates(&self) {
        let unread_count = self.unread_count.clone();
        let last_sync = self.last_sync.clone();
        let is_healthy = self.is_healthy.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Check health
                let healthy = {
                    let client_guard = client.read().await;
                    if let Some(c) = client_guard.as_ref() {
                        c.health_check().await
                    } else {
                        false
                    }
                };

                *is_healthy.write().await = healthy;

                if !healthy {
                    warn!("Daemon health check failed");
                    continue;
                }

                // Update unread count
                {
                    let client_guard = client.read().await;
                    if let Some(c) = client_guard.as_ref() {
                        match c.get_unread_count().await {
                            Ok(count) => {
                                *unread_count.write().await = count;
                                debug!("Background update: {} unread items", count);
                            }
                            Err(e) => {
                                warn!("Background update failed: {}", e);
                            }
                        }

                        // Update sync status
                        match c.get_sync_status().await {
                            Ok(status) => {
                                let most_recent = status
                                    .values()
                                    .filter_map(|s| s.last_sync)
                                    .max();

                                if let Some(sync_time) = most_recent {
                                    *last_sync.write().await = Some(sync_time);
                                }
                            }
                            Err(e) => {
                                debug!("Failed to update sync status: {}", e);
                            }
                        }
                    }
                }
            }
        });
    }
}

#[async_trait]
impl Plugin for ScryforgePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn get_menu(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::new("Sync All", MenuAction::Remote("sync_all".to_string()))
                .with_icon("🔄")
                .with_shortcut("Ctrl+S"),
            MenuItem::new("Mark All Read", MenuAction::Remote("mark_all_read".to_string()))
                .with_icon("✓")
                .with_shortcut("Ctrl+R"),
            MenuItem::new("Open TUI", MenuAction::Command("scryforge-tui".to_string()))
                .with_icon("📊")
                .with_shortcut("Ctrl+T"),
            MenuItem::new(
                "Refresh Status",
                MenuAction::Remote("refresh_status".to_string()),
            )
            .with_icon("♻️"),
        ]
    }

    async fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<()> {
        info!("Loading Scryforge plugin...");

        // Connect to daemon
        match ScryforgeClient::new(&self.daemon_url).await {
            Ok(client) => {
                info!("Connected to scryforge-daemon at {}", self.daemon_url);
                *self.client.write().await = Some(client);
                *self.is_healthy.write().await = true;

                // Initial status update
                if let Err(e) = self.update_unread_count().await {
                    warn!("Initial unread count update failed: {}", e);
                }

                if let Err(e) = self.update_sync_status().await {
                    warn!("Initial sync status update failed: {}", e);
                }

                // Start background updates
                self.start_background_updates().await;

                info!("Scryforge plugin loaded successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to scryforge-daemon: {}", e);
                warn!("Plugin will continue with limited functionality");

                // Don't fail plugin load, just mark as unhealthy
                *self.is_healthy.write().await = false;
                Ok(())
            }
        }
    }

    async fn on_unload(&mut self) -> Result<()> {
        info!("Unloading Scryforge plugin...");
        *self.client.write().await = None;
        Ok(())
    }

    async fn on_remote_command(&mut self, id: &str, _ctx: &PluginContext) -> Result<()> {
        debug!("Handling remote command: {}", id);

        match id {
            "sync_all" => {
                info!("Triggering sync for all providers...");
                let client_guard = self.client.read().await;
                if let Some(client) = client_guard.as_ref() {
                    match client.sync_all().await {
                        Ok(()) => {
                            info!("Sync triggered successfully");
                            // Update status after a short delay to allow sync to start
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            drop(client_guard);
                            self.update_unread_count().await?;
                            self.update_sync_status().await?;
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to trigger sync: {}", e);
                            Err(PluginError::Other(anyhow::anyhow!(e)))
                        }
                    }
                } else {
                    Err(PluginError::LoadError(
                        "Not connected to daemon".to_string(),
                    ))
                }
            }

            "mark_all_read" => {
                info!("Marking all items as read...");
                let client_guard = self.client.read().await;
                if let Some(client) = client_guard.as_ref() {
                    match client.mark_all_read().await {
                        Ok(()) => {
                            info!("All items marked as read");
                            drop(client_guard);
                            self.update_unread_count().await?;
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to mark all items as read: {}", e);
                            Err(PluginError::Other(anyhow::anyhow!(e)))
                        }
                    }
                } else {
                    Err(PluginError::LoadError(
                        "Not connected to daemon".to_string(),
                    ))
                }
            }

            "refresh_status" => {
                info!("Refreshing status...");
                self.update_unread_count().await?;
                self.update_sync_status().await?;

                // Update health status
                *self.is_healthy.write().await = self.health_check().await;

                info!("Status refreshed successfully");
                Ok(())
            }

            _ => {
                warn!("Unknown remote command: {}", id);
                Err(PluginError::Other(anyhow::anyhow!(
                    "Unknown command: {}",
                    id
                )))
            }
        }
    }
}

// Export the plugin creation function for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ScryforgePlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let plugin = ScryforgePlugin::new();
        let metadata = plugin.metadata();

        assert_eq!(metadata.name, "scryforge");
        assert_eq!(metadata.emoji, Some("📬".to_string()));
        assert_eq!(metadata.color, Some("#a6e3a1".to_string()));
    }

    #[test]
    fn test_plugin_menu() {
        let plugin = ScryforgePlugin::new();
        let menu = plugin.get_menu();

        assert_eq!(menu.len(), 4);
        assert_eq!(menu[0].label, "Sync All");
        assert_eq!(menu[1].label, "Mark All Read");
        assert_eq!(menu[2].label, "Open TUI");
        assert_eq!(menu[3].label, "Refresh Status");
    }

    #[test]
    fn test_custom_daemon_url() {
        let plugin = ScryforgePlugin::with_daemon_url("http://localhost:4040");
        assert_eq!(plugin.daemon_url, "http://localhost:4040");
    }

    #[tokio::test]
    async fn test_plugin_creation() {
        let plugin = ScryforgePlugin::new();
        assert_eq!(*plugin.unread_count.read().await, 0);
        assert_eq!(*plugin.last_sync.read().await, None);
        assert!(!*plugin.is_healthy.read().await);
    }
}

//! Plugin instance and provider implementation.
//!
//! This module provides the bridge between Fusabi plugins and Scryforge's
//! provider system.

use crate::host::{DefaultHostFunctions, HostFunctions};
use async_trait::async_trait;
use fusabi_runtime::{Bytecode, BytecodeLoader, PluginManifest, PluginPath, RuntimeResult};
use scryforge_provider_core::{
    Action, ActionResult, Feed, FeedId, FeedOptions, HasFeeds, Item, Provider, ProviderCapabilities,
    ProviderHealth, Result as ProviderResult, StreamError, SyncResult,
};
use std::any::Any;
use std::sync::Arc;
use tracing::{debug, info};

/// A loaded plugin instance.
pub struct PluginInstance {
    /// Plugin manifest.
    pub manifest: PluginManifest,

    /// Plugin path.
    pub path: std::path::PathBuf,

    /// Loaded bytecode (if available).
    pub bytecode: Option<Bytecode>,

    /// Host functions for this plugin.
    pub host: Arc<dyn HostFunctions>,

    /// Whether the plugin is enabled.
    pub enabled: bool,
}

impl PluginInstance {
    /// Load a plugin from a discovered path.
    pub fn load(plugin_path: &PluginPath) -> RuntimeResult<Self> {
        info!("Loading plugin: {} v{}", plugin_path.name(), plugin_path.version());

        // Create host functions with plugin's capabilities
        let host = Arc::new(DefaultHostFunctions::new(
            plugin_path.id().to_string(),
            plugin_path.manifest.capability_set(),
        ));

        // Try to load bytecode if entry point exists
        let bytecode = if plugin_path.has_entry_point() {
            let entry_path = plugin_path.entry_point_path();
            debug!("Loading bytecode from {:?}", entry_path);
            match BytecodeLoader::load(&entry_path) {
                Ok(bc) => {
                    BytecodeLoader::validate(&bc)?;
                    Some(bc)
                }
                Err(e) => {
                    debug!("No bytecode loaded: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            manifest: plugin_path.manifest.clone(),
            path: plugin_path.path.clone(),
            bytecode,
            host,
            enabled: plugin_path.enabled,
        })
    }

    /// Get the plugin ID.
    pub fn id(&self) -> &str {
        &self.manifest.plugin.id
    }

    /// Get the plugin name.
    pub fn name(&self) -> &str {
        &self.manifest.plugin.name
    }

    /// Get the plugin version.
    pub fn version(&self) -> &str {
        &self.manifest.plugin.version
    }

    /// Check if this is a provider plugin.
    pub fn is_provider(&self) -> bool {
        self.manifest.is_provider()
    }

    /// Convert to a provider if this is a provider plugin.
    pub fn as_provider(self: Arc<Self>) -> Option<PluginProvider> {
        if self.is_provider() {
            Some(PluginProvider { instance: self })
        } else {
            None
        }
    }
}

/// A plugin that implements the Provider trait.
pub struct PluginProvider {
    instance: Arc<PluginInstance>,
}

impl PluginProvider {
    /// Create a new plugin provider.
    pub fn new(instance: Arc<PluginInstance>) -> Self {
        Self { instance }
    }

    /// Get the underlying plugin instance.
    pub fn instance(&self) -> &PluginInstance {
        &self.instance
    }
}

#[async_trait]
impl Provider for PluginProvider {
    fn id(&self) -> &'static str {
        // We need to leak the string since Provider expects 'static
        // This is acceptable because plugins are loaded once at startup
        let id = self
            .instance
            .manifest
            .provider
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| self.instance.id().to_string());

        Box::leak(id.into_boxed_str())
    }

    fn name(&self) -> &'static str {
        let name = self
            .instance
            .manifest
            .provider
            .as_ref()
            .and_then(|p| p.display_name.clone())
            .unwrap_or_else(|| self.instance.name().to_string());

        Box::leak(name.into_boxed_str())
    }

    async fn health_check(&self) -> ProviderResult<ProviderHealth> {
        // TODO: Call plugin's health_check function via bytecode interpreter
        // For now, return healthy if plugin loaded successfully
        Ok(ProviderHealth {
            is_healthy: self.instance.enabled && self.instance.bytecode.is_some(),
            message: if self.instance.bytecode.is_some() {
                None
            } else {
                Some("Plugin bytecode not loaded".to_string())
            },
            last_sync: None,
            error_count: 0,
        })
    }

    async fn sync(&self) -> ProviderResult<SyncResult> {
        // TODO: Call plugin's sync function via bytecode interpreter
        // For now, return empty sync result
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
        let provider_config = self.instance.manifest.provider.as_ref();

        ProviderCapabilities {
            has_feeds: provider_config.map(|p| p.has_feeds).unwrap_or(false),
            has_collections: provider_config.map(|p| p.has_collections).unwrap_or(false),
            has_saved_items: provider_config.map(|p| p.has_saved_items).unwrap_or(false),
            has_communities: provider_config.map(|p| p.has_communities).unwrap_or(false),
        }
    }

    async fn available_actions(&self, _item: &Item) -> ProviderResult<Vec<Action>> {
        // TODO: Call plugin's available_actions function
        Ok(vec![])
    }

    async fn execute_action(&self, _item: &Item, _action: &Action) -> ProviderResult<ActionResult> {
        // TODO: Call plugin's execute_action function
        Err(StreamError::Provider("Action execution not implemented for plugins".to_string()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl HasFeeds for PluginProvider {
    async fn list_feeds(&self) -> ProviderResult<Vec<Feed>> {
        // TODO: Call plugin's list_feeds function via bytecode interpreter
        Ok(vec![])
    }

    async fn get_feed_items(&self, _feed_id: &FeedId, _options: FeedOptions) -> ProviderResult<Vec<Item>> {
        // TODO: Call plugin's get_feed_items function via bytecode interpreter
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusabi_runtime::PluginManifest;
    use tempfile::TempDir;
    use std::io::Write;

    fn create_test_manifest() -> String {
        r#"
[plugin]
id = "test-plugin"
name = "Test Plugin"
version = "0.1.0"
plugin_type = "provider"

capabilities = ["network"]

[provider]
id = "test"
display_name = "Test Provider"
has_feeds = true
"#.to_string()
    }

    #[test]
    fn test_parse_manifest() {
        let manifest = PluginManifest::from_str(&create_test_manifest()).unwrap();
        assert_eq!(manifest.plugin.id, "test-plugin");
        assert!(manifest.is_provider());
    }

    #[tokio::test]
    async fn test_plugin_provider() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest_path = plugin_dir.join("manifest.toml");
        let mut file = std::fs::File::create(&manifest_path).unwrap();
        file.write_all(create_test_manifest().as_bytes()).unwrap();

        let plugin_path = fusabi_runtime::discover_plugin(&plugin_dir).unwrap();
        let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());

        let provider = PluginProvider::new(instance);
        assert_eq!(provider.id(), "test");
        assert_eq!(provider.name(), "Test Provider");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(!caps.has_collections);
    }
}

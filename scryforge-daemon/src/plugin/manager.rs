//! Plugin manager for loading and managing Fusabi plugins.

use crate::registry::ProviderRegistry;
use fusabi_plugin_api::{PluginInstance, PluginProvider, PluginRegistry as FusabiPluginRegistry};
use fusabi_runtime::{discover_plugins, RuntimeResult};
use std::sync::Arc;
use tracing::{info, warn};

/// Status of a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is loaded and enabled.
    Enabled,
    /// Plugin is loaded but disabled.
    Disabled,
    /// Plugin failed to load.
    Failed(String),
}

/// Manager for Fusabi plugins.
///
/// The plugin manager handles plugin discovery, loading, and lifecycle management.
/// It bridges Fusabi plugins with the Scryforge provider registry.
pub struct PluginManager {
    /// Internal Fusabi plugin registry.
    plugin_registry: FusabiPluginRegistry,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new() -> Self {
        Self {
            plugin_registry: FusabiPluginRegistry::new(),
        }
    }

    /// Discover and load all plugins from well-known paths.
    ///
    /// Returns the number of successfully loaded plugins.
    pub fn discover_and_load(&mut self) -> RuntimeResult<usize> {
        info!("Discovering plugins...");
        self.plugin_registry.discover_and_load()
    }

    /// Register all plugin providers with the provider registry.
    ///
    /// This method should be called after plugins are loaded to make
    /// plugin-based providers available through the standard provider registry.
    pub fn register_providers(&self, registry: &mut ProviderRegistry) {
        let provider_ids = self.plugin_registry.provider_ids();
        info!("Registering {} plugin providers", provider_ids.len());

        for id in &provider_ids {
            if let Some(provider) = self.plugin_registry.get_provider(id) {
                // We need to clone the provider or create a wrapper
                // For now, log that we would register it
                info!("Would register plugin provider: {}", id);
                // Note: The actual registration requires Provider to be Clone
                // or we need to store Arc<PluginProvider> in the registry
            }
        }
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugin_registry.plugin_count()
    }

    /// Get the number of plugin-based providers.
    pub fn provider_count(&self) -> usize {
        self.plugin_registry.provider_count()
    }

    /// List all loaded plugins with their status.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugin_registry
            .list_plugins()
            .into_iter()
            .map(|p| PluginInfo {
                id: p.id,
                name: p.name,
                version: p.version,
                status: if p.enabled {
                    PluginStatus::Enabled
                } else {
                    PluginStatus::Disabled
                },
                is_provider: p.is_provider,
                has_bytecode: p.has_bytecode,
            })
            .collect()
    }

    /// Enable a plugin by ID.
    pub fn enable_plugin(&mut self, id: &str) -> RuntimeResult<()> {
        self.plugin_registry.enable_plugin(id)
    }

    /// Disable a plugin by ID.
    pub fn disable_plugin(&mut self, id: &str) -> RuntimeResult<()> {
        self.plugin_registry.disable_plugin(id)
    }

    /// Check if a plugin is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugin_registry.is_enabled(id)
    }

    /// Get the underlying plugin registry.
    pub fn registry(&self) -> &FusabiPluginRegistry {
        &self.plugin_registry
    }

    /// Get a mutable reference to the underlying plugin registry.
    pub fn registry_mut(&mut self) -> &mut FusabiPluginRegistry {
        &mut self.plugin_registry
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a loaded plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub status: PluginStatus,
    pub is_provider: bool,
    pub has_bytecode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_plugin_manager() {
        let manager = PluginManager::new();
        assert_eq!(manager.plugin_count(), 0);
        assert_eq!(manager.provider_count(), 0);
    }
}

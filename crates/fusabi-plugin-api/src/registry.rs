//! Plugin registry for managing loaded plugins.
//!
//! The registry handles plugin discovery, loading, enabling/disabling,
//! and provides access to loaded plugins.

use crate::plugin::{PluginInstance, PluginProvider};
use fusabi_runtime::{discover_plugins, RuntimeError, RuntimeResult};
use scryforge_provider_core::Provider;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// Registry for managing Fusabi plugins.
pub struct PluginRegistry {
    /// Loaded plugin instances by ID.
    plugins: HashMap<String, Arc<PluginInstance>>,

    /// Plugin providers (for plugins that implement Provider).
    providers: HashMap<String, PluginProvider>,

    /// Disabled plugin IDs.
    disabled: std::collections::HashSet<String>,
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            providers: HashMap::new(),
            disabled: std::collections::HashSet::new(),
        }
    }

    /// Discover and load all plugins from well-known paths.
    pub fn discover_and_load(&mut self) -> RuntimeResult<usize> {
        let discovered = discover_plugins()?;
        let mut loaded = 0;

        for plugin_path in discovered {
            match self.load_plugin(&plugin_path.path) {
                Ok(_) => loaded += 1,
                Err(e) => warn!("Failed to load plugin {:?}: {}", plugin_path.path, e),
            }
        }

        info!("Loaded {} plugins", loaded);
        Ok(loaded)
    }

    /// Load a plugin from a specific path.
    pub fn load_plugin(&mut self, path: &Path) -> RuntimeResult<String> {
        let plugin_path = fusabi_runtime::discover_plugin(path)?;
        let instance = Arc::new(PluginInstance::load(&plugin_path)?);
        let id = instance.id().to_string();

        // Check if already loaded
        if self.plugins.contains_key(&id) {
            return Err(RuntimeError::InvalidManifest(format!(
                "Plugin '{}' is already loaded",
                id
            )));
        }

        // Check if disabled
        let enabled = !self.disabled.contains(&id);

        info!(
            "Registered plugin: {} v{} ({})",
            instance.name(),
            instance.version(),
            if enabled { "enabled" } else { "disabled" }
        );

        // Register as provider if applicable
        if instance.is_provider() && enabled {
            let provider = PluginProvider::new(Arc::clone(&instance));
            let provider_id = provider.id().to_string();
            self.providers.insert(provider_id, provider);
        }

        self.plugins.insert(id.clone(), instance);
        Ok(id)
    }

    /// Unload a plugin by ID.
    pub fn unload_plugin(&mut self, id: &str) -> RuntimeResult<()> {
        if let Some(instance) = self.plugins.remove(id) {
            // Remove from providers if it was registered
            if instance.is_provider() {
                if let Some(provider_config) = &instance.manifest.provider {
                    self.providers.remove(&provider_config.id);
                }
            }
            info!("Unloaded plugin: {}", id);
            Ok(())
        } else {
            Err(RuntimeError::PluginNotFound(id.to_string()))
        }
    }

    /// Enable a plugin.
    pub fn enable_plugin(&mut self, id: &str) -> RuntimeResult<()> {
        self.disabled.remove(id);

        if let Some(instance) = self.plugins.get(id) {
            // Register as provider if applicable
            if instance.is_provider() {
                let provider = PluginProvider::new(Arc::clone(instance));
                let provider_id = provider.id().to_string();
                self.providers.insert(provider_id, provider);
            }
            info!("Enabled plugin: {}", id);
            Ok(())
        } else {
            Err(RuntimeError::PluginNotFound(id.to_string()))
        }
    }

    /// Disable a plugin.
    pub fn disable_plugin(&mut self, id: &str) -> RuntimeResult<()> {
        if let Some(instance) = self.plugins.get(id) {
            self.disabled.insert(id.to_string());

            // Remove from providers
            if instance.is_provider() {
                if let Some(provider_config) = &instance.manifest.provider {
                    self.providers.remove(&provider_config.id);
                }
            }
            info!("Disabled plugin: {}", id);
            Ok(())
        } else {
            Err(RuntimeError::PluginNotFound(id.to_string()))
        }
    }

    /// Check if a plugin is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugins.contains_key(id) && !self.disabled.contains(id)
    }

    /// Get a plugin instance by ID.
    pub fn get_plugin(&self, id: &str) -> Option<&Arc<PluginInstance>> {
        self.plugins.get(id)
    }

    /// Get a provider by ID.
    pub fn get_provider(&self, id: &str) -> Option<&PluginProvider> {
        self.providers.get(id)
    }

    /// Get all loaded plugin IDs.
    pub fn plugin_ids(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get all provider IDs from plugins.
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Get all providers as trait objects.
    pub fn providers(&self) -> impl Iterator<Item = &dyn Provider> {
        self.providers.values().map(|p| p as &dyn Provider)
    }

    /// List plugin information.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .map(|p| PluginInfo {
                id: p.id().to_string(),
                name: p.name().to_string(),
                version: p.version().to_string(),
                enabled: !self.disabled.contains(p.id()),
                is_provider: p.is_provider(),
                has_bytecode: p.bytecode.is_some(),
            })
            .collect()
    }
}

impl Default for PluginRegistry {
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
    pub enabled: bool,
    pub is_provider: bool,
    pub has_bytecode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &Path, id: &str, provider_id: &str) {
        let plugin_dir = dir.join(id);
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = format!(
            r#"
[plugin]
id = "{id}"
name = "Test Plugin {id}"
version = "0.1.0"
plugin_type = "provider"

capabilities = ["network"]

[provider]
id = "{provider_id}"
has_feeds = true
"#
        );

        let manifest_path = plugin_dir.join("manifest.toml");
        let mut file = std::fs::File::create(manifest_path).unwrap();
        file.write_all(manifest.as_bytes()).unwrap();
    }

    #[test]
    fn test_registry_load_plugin() {
        let temp_dir = TempDir::new().unwrap();
        create_test_plugin(temp_dir.path(), "test-plugin", "test-provider");

        let mut registry = PluginRegistry::new();
        let plugin_dir = temp_dir.path().join("test-plugin");
        let id = registry.load_plugin(&plugin_dir).unwrap();

        assert_eq!(id, "test-plugin");
        assert_eq!(registry.plugin_count(), 1);
        assert_eq!(registry.provider_count(), 1);
        assert!(registry.is_enabled("test-plugin"));
    }

    #[test]
    fn test_registry_enable_disable() {
        let temp_dir = TempDir::new().unwrap();
        create_test_plugin(temp_dir.path(), "test-plugin", "test-provider");

        let mut registry = PluginRegistry::new();
        let plugin_dir = temp_dir.path().join("test-plugin");
        registry.load_plugin(&plugin_dir).unwrap();

        // Initially enabled
        assert!(registry.is_enabled("test-plugin"));
        assert_eq!(registry.provider_count(), 1);

        // Disable
        registry.disable_plugin("test-plugin").unwrap();
        assert!(!registry.is_enabled("test-plugin"));
        assert_eq!(registry.provider_count(), 0);

        // Re-enable
        registry.enable_plugin("test-plugin").unwrap();
        assert!(registry.is_enabled("test-plugin"));
        assert_eq!(registry.provider_count(), 1);
    }

    #[test]
    fn test_registry_unload() {
        let temp_dir = TempDir::new().unwrap();
        create_test_plugin(temp_dir.path(), "test-plugin", "test-provider");

        let mut registry = PluginRegistry::new();
        let plugin_dir = temp_dir.path().join("test-plugin");
        registry.load_plugin(&plugin_dir).unwrap();

        assert_eq!(registry.plugin_count(), 1);

        registry.unload_plugin("test-plugin").unwrap();
        assert_eq!(registry.plugin_count(), 0);
        assert_eq!(registry.provider_count(), 0);
    }
}

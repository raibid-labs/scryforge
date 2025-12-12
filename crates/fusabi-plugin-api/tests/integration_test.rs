//! Integration tests for fusabi-plugin-api.
//!
//! These tests cover:
//! - Full plugin lifecycle: discover → load → enable → use → disable → unload
//! - Plugin registry operations
//! - Plugin provider registration and usage
//! - Host function capabilities and security

use fusabi_plugin_api::{PluginInstance, PluginProvider, PluginRegistry};
use fusabi_runtime::bytecode::{Bytecode, BytecodeMetadata, Constant, Function, Instruction};
use fusabi_runtime::{discover_plugin, Capability};
use scryforge_provider_core::{HasFeeds, Provider};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

// ==============================================================================
// Test Fixture Helpers
// ==============================================================================

/// Create a test plugin directory with a manifest.toml file.
fn create_test_plugin(dir: &Path, id: &str, config: PluginConfig) -> PathBuf {
    let plugin_dir = dir.join(id);
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let mut manifest = String::new();

    // Capabilities at the top level
    if !config.capabilities.is_empty() {
        manifest.push_str("capabilities = [");
        for (i, cap) in config.capabilities.iter().enumerate() {
            if i > 0 {
                manifest.push_str(", ");
            }
            manifest.push_str(&format!("\"{}\"", cap));
        }
        manifest.push_str("]\n\n");
    }

    // Plugin section
    manifest.push_str(&format!(
        r#"[plugin]
id = "{id}"
name = "{name}"
version = "{version}"
plugin_type = "{plugin_type}"
"#,
        name = config.name.unwrap_or(&format!("Test Plugin {}", id)),
        version = config.version.unwrap_or("0.1.0"),
        plugin_type = config.plugin_type.unwrap_or("provider"),
    ));

    if let Some(entry_point) = config.entry_point {
        manifest.push_str(&format!("entry_point = \"{}\"\n", entry_point));
    }

    if config.is_provider {
        manifest.push_str("\n[provider]\n");
        manifest.push_str(&format!("id = \"{}\"\n", config.provider_id.unwrap_or(id)));
        if let Some(display_name) = config.provider_display_name {
            manifest.push_str(&format!("display_name = \"{}\"\n", display_name));
        }
        if config.has_feeds {
            manifest.push_str("has_feeds = true\n");
        }
        if config.has_collections {
            manifest.push_str("has_collections = true\n");
        }
        if config.has_saved_items {
            manifest.push_str("has_saved_items = true\n");
        }
    }

    let manifest_path = plugin_dir.join("manifest.toml");
    let mut file = std::fs::File::create(&manifest_path).unwrap();
    file.write_all(manifest.as_bytes()).unwrap();

    plugin_dir
}

/// Configuration for creating a test plugin.
#[derive(Default)]
struct PluginConfig<'a> {
    name: Option<&'a str>,
    version: Option<&'a str>,
    plugin_type: Option<&'a str>,
    entry_point: Option<&'a str>,
    capabilities: Vec<&'a str>,
    is_provider: bool,
    provider_id: Option<&'a str>,
    provider_display_name: Option<&'a str>,
    has_feeds: bool,
    has_collections: bool,
    has_saved_items: bool,
}

/// Create a sample bytecode file.
fn create_bytecode_file(dir: &Path, filename: &str, plugin_id: &str) {
    let bytecode = Bytecode {
        version: 1,
        metadata: BytecodeMetadata {
            plugin_id: plugin_id.to_string(),
            plugin_version: "0.1.0".to_string(),
            compiled_at: None,
            compiler_version: None,
        },
        constants: vec![Constant::String("test".to_string())],
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            instructions: vec![Instruction::Return],
            local_count: 0,
        }],
        entry_point: "main".to_string(),
    };

    let bytecode_path = dir.join(filename);
    let json = serde_json::to_vec_pretty(&bytecode).unwrap();
    std::fs::write(&bytecode_path, json).unwrap();
}

// ==============================================================================
// Plugin Instance Tests
// ==============================================================================

#[test]
fn test_plugin_instance_load() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            capabilities: vec!["network"],
            is_provider: true,
            has_feeds: true,
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = PluginInstance::load(&plugin_path).unwrap();

    assert_eq!(instance.id(), "test-plugin");
    assert_eq!(instance.name(), "Test Plugin test-plugin");
    assert_eq!(instance.version(), "0.1.0");
    assert!(instance.is_provider());
    assert!(instance.enabled);
    assert!(instance.bytecode.is_none()); // No bytecode file created yet
}

#[test]
fn test_plugin_instance_load_with_bytecode() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            capabilities: vec!["network"],
            is_provider: true,
            ..Default::default()
        },
    );

    create_bytecode_file(&plugin_dir, "plugin.fzb", "test-plugin");

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = PluginInstance::load(&plugin_path).unwrap();

    assert!(instance.bytecode.is_some());
    let bytecode = instance.bytecode.as_ref().unwrap();
    assert_eq!(bytecode.version, 1);
    assert_eq!(bytecode.metadata.plugin_id, "test-plugin");
}

#[test]
fn test_plugin_instance_as_provider() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "provider-plugin",
        PluginConfig {
            is_provider: true,
            provider_id: Some("test-provider"),
            provider_display_name: Some("Test Provider"),
            has_feeds: true,
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());

    let provider = instance.as_provider();
    assert!(provider.is_some());

    let provider = provider.unwrap();
    assert_eq!(provider.instance().id(), "provider-plugin");
}

#[test]
fn test_non_provider_plugin_as_provider() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "action-plugin",
        PluginConfig {
            plugin_type: Some("action"),
            is_provider: false,
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());

    assert!(!instance.is_provider());
    assert!(instance.as_provider().is_none());
}

// ==============================================================================
// Plugin Provider Tests
// ==============================================================================

#[tokio::test]
async fn test_plugin_provider_trait_implementation() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "provider-test",
        PluginConfig {
            is_provider: true,
            provider_id: Some("test-provider"),
            provider_display_name: Some("Test Provider"),
            capabilities: vec!["network"],
            has_feeds: true,
            has_collections: true,
            ..Default::default()
        },
    );

    create_bytecode_file(&plugin_dir, "plugin.fzb", "provider-test");

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());
    let provider = PluginProvider::new(instance);

    // Test Provider trait methods
    assert_eq!(provider.id(), "test-provider");
    assert_eq!(provider.name(), "Test Provider");

    let caps = provider.capabilities();
    assert!(caps.has_feeds);
    assert!(caps.has_collections);
    assert!(!caps.has_saved_items);

    // Test health check
    let health = provider.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert!(health.message.is_none());

    // Test sync
    let sync_result = provider.sync().await.unwrap();
    assert!(sync_result.success);
}

#[tokio::test]
async fn test_plugin_provider_without_bytecode() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "provider-test",
        PluginConfig {
            is_provider: true,
            provider_id: Some("test-provider"),
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());
    let provider = PluginProvider::new(instance);

    // Health check should indicate bytecode not loaded
    let health = provider.health_check().await.unwrap();
    assert!(!health.is_healthy);
    assert!(health.message.is_some());
    assert!(health.message.unwrap().contains("bytecode not loaded"));
}

#[tokio::test]
async fn test_plugin_provider_has_feeds() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "feed-provider",
        PluginConfig {
            is_provider: true,
            provider_id: Some("feed-test"),
            has_feeds: true,
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());
    let provider = PluginProvider::new(instance);

    // Test HasFeeds implementation
    let feeds = provider.list_feeds().await.unwrap();
    assert_eq!(feeds.len(), 0); // Default implementation returns empty

    let feed_id = scryforge_provider_core::FeedId("test-feed".to_string());
    let items = provider
        .get_feed_items(&feed_id, Default::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 0); // Default implementation returns empty
}

// ==============================================================================
// Plugin Registry Tests
// ==============================================================================

#[test]
fn test_registry_new() {
    let registry = PluginRegistry::new();
    assert_eq!(registry.plugin_count(), 0);
    assert_eq!(registry.provider_count(), 0);
}

#[test]
fn test_registry_load_single_plugin() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            is_provider: true,
            provider_id: Some("test-provider"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    let id = registry.load_plugin(&plugin_dir).unwrap();

    assert_eq!(id, "test-plugin");
    assert_eq!(registry.plugin_count(), 1);
    assert_eq!(registry.provider_count(), 1);
    assert!(registry.is_enabled("test-plugin"));
}

#[test]
fn test_registry_load_multiple_plugins() {
    let temp_dir = TempDir::new().unwrap();

    let plugin1 = create_test_plugin(
        temp_dir.path(),
        "plugin-1",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-1"),
            ..Default::default()
        },
    );

    let plugin2 = create_test_plugin(
        temp_dir.path(),
        "plugin-2",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-2"),
            ..Default::default()
        },
    );

    let plugin3 = create_test_plugin(
        temp_dir.path(),
        "plugin-3",
        PluginConfig {
            plugin_type: Some("action"),
            is_provider: false,
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin1).unwrap();
    registry.load_plugin(&plugin2).unwrap();
    registry.load_plugin(&plugin3).unwrap();

    assert_eq!(registry.plugin_count(), 3);
    assert_eq!(registry.provider_count(), 2); // Only 2 are providers

    let plugin_ids = registry.plugin_ids();
    assert!(plugin_ids.contains(&"plugin-1".to_string()));
    assert!(plugin_ids.contains(&"plugin-2".to_string()));
    assert!(plugin_ids.contains(&"plugin-3".to_string()));

    let provider_ids = registry.provider_ids();
    assert!(provider_ids.contains(&"provider-1".to_string()));
    assert!(provider_ids.contains(&"provider-2".to_string()));
}

#[test]
fn test_registry_duplicate_plugin() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            is_provider: true,
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin_dir).unwrap();

    // Try to load the same plugin again
    let result = registry.load_plugin(&plugin_dir);
    assert!(result.is_err());
}

#[test]
fn test_registry_get_plugin() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(temp_dir.path(), "test-plugin", PluginConfig::default());

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin_dir).unwrap();

    let plugin = registry.get_plugin("test-plugin");
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().id(), "test-plugin");

    let missing = registry.get_plugin("nonexistent");
    assert!(missing.is_none());
}

#[test]
fn test_registry_get_provider() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            is_provider: true,
            provider_id: Some("test-provider"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin_dir).unwrap();

    let provider = registry.get_provider("test-provider");
    assert!(provider.is_some());
    assert_eq!(provider.unwrap().id(), "test-provider");

    let missing = registry.get_provider("nonexistent");
    assert!(missing.is_none());
}

// ==============================================================================
// Plugin Lifecycle Tests (Enable/Disable/Unload)
// ==============================================================================

#[test]
fn test_full_plugin_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "lifecycle-plugin",
        PluginConfig {
            is_provider: true,
            provider_id: Some("lifecycle-provider"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();

    // 1. Load
    let id = registry.load_plugin(&plugin_dir).unwrap();
    assert_eq!(id, "lifecycle-plugin");
    assert_eq!(registry.plugin_count(), 1);
    assert_eq!(registry.provider_count(), 1);
    assert!(registry.is_enabled("lifecycle-plugin"));

    // 2. Disable
    registry.disable_plugin("lifecycle-plugin").unwrap();
    assert!(!registry.is_enabled("lifecycle-plugin"));
    assert_eq!(registry.plugin_count(), 1); // Still loaded
    assert_eq!(registry.provider_count(), 0); // Not registered as provider

    // 3. Re-enable
    registry.enable_plugin("lifecycle-plugin").unwrap();
    assert!(registry.is_enabled("lifecycle-plugin"));
    assert_eq!(registry.provider_count(), 1); // Registered again

    // 4. Unload
    registry.unload_plugin("lifecycle-plugin").unwrap();
    assert_eq!(registry.plugin_count(), 0);
    assert_eq!(registry.provider_count(), 0);
    assert!(!registry.is_enabled("lifecycle-plugin"));
}

#[test]
fn test_disable_nonexistent_plugin() {
    let mut registry = PluginRegistry::new();
    let result = registry.disable_plugin("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_enable_nonexistent_plugin() {
    let mut registry = PluginRegistry::new();
    let result = registry.enable_plugin("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_unload_nonexistent_plugin() {
    let mut registry = PluginRegistry::new();
    let result = registry.unload_plugin("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_disable_non_provider_plugin() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "action-plugin",
        PluginConfig {
            plugin_type: Some("action"),
            is_provider: false,
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin_dir).unwrap();

    assert_eq!(registry.provider_count(), 0);

    registry.disable_plugin("action-plugin").unwrap();
    assert!(!registry.is_enabled("action-plugin"));
    assert_eq!(registry.provider_count(), 0); // Still 0
}

#[test]
fn test_list_plugins() {
    let temp_dir = TempDir::new().unwrap();

    let plugin1 = create_test_plugin(
        temp_dir.path(),
        "plugin-1",
        PluginConfig {
            is_provider: true,
            ..Default::default()
        },
    );

    let plugin2 = create_test_plugin(
        temp_dir.path(),
        "plugin-2",
        PluginConfig {
            plugin_type: Some("action"),
            is_provider: false,
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin1).unwrap();
    registry.load_plugin(&plugin2).unwrap();

    let info_list = registry.list_plugins();
    assert_eq!(info_list.len(), 2);

    let plugin1_info = info_list.iter().find(|p| p.id == "plugin-1").unwrap();
    assert_eq!(plugin1_info.name, "Test Plugin plugin-1");
    assert_eq!(plugin1_info.version, "0.1.0");
    assert!(plugin1_info.enabled);
    assert!(plugin1_info.is_provider);

    let plugin2_info = info_list.iter().find(|p| p.id == "plugin-2").unwrap();
    assert!(!plugin2_info.is_provider);
}

// ==============================================================================
// Registry Provider Integration Tests
// ==============================================================================

#[test]
fn test_registry_providers_iterator() {
    let temp_dir = TempDir::new().unwrap();

    let plugin1 = create_test_plugin(
        temp_dir.path(),
        "plugin-1",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-1"),
            provider_display_name: Some("Provider One"),
            ..Default::default()
        },
    );

    let plugin2 = create_test_plugin(
        temp_dir.path(),
        "plugin-2",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-2"),
            provider_display_name: Some("Provider Two"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin1).unwrap();
    registry.load_plugin(&plugin2).unwrap();

    let providers: Vec<_> = registry.providers().collect();
    assert_eq!(providers.len(), 2);

    let provider_ids: Vec<_> = providers.iter().map(|p| p.id()).collect();
    assert!(provider_ids.contains(&"provider-1"));
    assert!(provider_ids.contains(&"provider-2"));
}

#[tokio::test]
async fn test_registry_provider_usage() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "usable-plugin",
        PluginConfig {
            is_provider: true,
            provider_id: Some("usable-provider"),
            provider_display_name: Some("Usable Provider"),
            has_feeds: true,
            ..Default::default()
        },
    );

    create_bytecode_file(&plugin_dir, "plugin.fzb", "usable-plugin");

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin_dir).unwrap();

    let provider = registry.get_provider("usable-provider").unwrap();

    // Use the provider
    assert_eq!(provider.id(), "usable-provider");
    assert_eq!(provider.name(), "Usable Provider");

    let health = provider.health_check().await.unwrap();
    assert!(health.is_healthy);

    let caps = provider.capabilities();
    assert!(caps.has_feeds);
}

// ==============================================================================
// Complex Integration Scenarios
// ==============================================================================

#[test]
fn test_complex_multi_plugin_lifecycle() {
    let temp_dir = TempDir::new().unwrap();

    // Create different types of plugins
    let provider1 = create_test_plugin(
        temp_dir.path(),
        "provider-a",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-a"),
            has_feeds: true,
            ..Default::default()
        },
    );

    let provider2 = create_test_plugin(
        temp_dir.path(),
        "provider-b",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-b"),
            has_collections: true,
            ..Default::default()
        },
    );

    let action = create_test_plugin(
        temp_dir.path(),
        "action-x",
        PluginConfig {
            plugin_type: Some("action"),
            is_provider: false,
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();

    // Load all plugins
    registry.load_plugin(&provider1).unwrap();
    registry.load_plugin(&provider2).unwrap();
    registry.load_plugin(&action).unwrap();

    assert_eq!(registry.plugin_count(), 3);
    assert_eq!(registry.provider_count(), 2);

    // Disable one provider
    registry.disable_plugin("provider-a").unwrap();
    assert_eq!(registry.plugin_count(), 3);
    assert_eq!(registry.provider_count(), 1); // Only provider-b active

    // Unload the action
    registry.unload_plugin("action-x").unwrap();
    assert_eq!(registry.plugin_count(), 2);
    assert_eq!(registry.provider_count(), 1);

    // Re-enable provider-a
    registry.enable_plugin("provider-a").unwrap();
    assert_eq!(registry.provider_count(), 2);

    // Verify remaining plugins
    let info_list = registry.list_plugins();
    assert_eq!(info_list.len(), 2);
    assert!(info_list.iter().all(|p| p.is_provider));
}

#[test]
fn test_plugin_with_all_capabilities() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "full-caps",
        PluginConfig {
            capabilities: vec![
                "network",
                "file_read",
                "file_write",
                "credentials",
                "cache_read",
                "cache_write",
                "notifications",
                "clipboard",
            ],
            is_provider: true,
            provider_id: Some("full-caps-provider"),
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    let instance = PluginInstance::load(&plugin_path).unwrap();

    assert_eq!(instance.manifest.capabilities.len(), 8);

    let caps = instance.manifest.capability_set();
    assert!(caps.has(&Capability::Network));
    assert!(caps.has(&Capability::FileRead));
    assert!(caps.has(&Capability::FileWrite));
    assert!(caps.has(&Capability::Credentials));
    assert!(caps.has(&Capability::CacheRead));
    assert!(caps.has(&Capability::CacheWrite));
    assert!(caps.has(&Capability::Notifications));
    assert!(caps.has(&Capability::Clipboard));
}

#[test]
fn test_multiple_versions_same_provider_id() {
    let temp_dir = TempDir::new().unwrap();

    // Create two different plugins with the same provider ID
    let plugin1 = create_test_plugin(
        temp_dir.path(),
        "plugin-v1",
        PluginConfig {
            version: Some("1.0.0"),
            is_provider: true,
            provider_id: Some("shared-provider"),
            ..Default::default()
        },
    );

    let plugin2 = create_test_plugin(
        temp_dir.path(),
        "plugin-v2",
        PluginConfig {
            version: Some("2.0.0"),
            is_provider: true,
            provider_id: Some("shared-provider"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin1).unwrap();

    // Loading second plugin will overwrite the provider registration
    registry.load_plugin(&plugin2).unwrap();

    assert_eq!(registry.plugin_count(), 2);
    assert_eq!(registry.provider_count(), 1); // Only one provider registered

    let provider = registry.get_provider("shared-provider").unwrap();
    // The last loaded plugin wins
    assert_eq!(provider.instance().id(), "plugin-v2");
}

#[tokio::test]
async fn test_end_to_end_plugin_as_provider() {
    let temp_dir = TempDir::new().unwrap();

    // Create a fully featured plugin
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "e2e-plugin",
        PluginConfig {
            name: Some("E2E Test Plugin"),
            version: Some("1.0.0"),
            capabilities: vec!["network", "cache_read", "cache_write"],
            is_provider: true,
            provider_id: Some("e2e-provider"),
            provider_display_name: Some("E2E Provider"),
            has_feeds: true,
            has_collections: true,
            has_saved_items: true,
            ..Default::default()
        },
    );

    create_bytecode_file(&plugin_dir, "plugin.fzb", "e2e-plugin");

    // Discover
    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    assert!(plugin_path.has_entry_point());

    // Load
    let instance = Arc::new(PluginInstance::load(&plugin_path).unwrap());
    assert!(instance.bytecode.is_some());
    assert!(instance.is_provider());

    // Use as provider
    let provider = instance.as_provider().unwrap();
    assert_eq!(provider.id(), "e2e-provider");
    assert_eq!(provider.name(), "E2E Provider");

    // Test provider capabilities
    let caps = provider.capabilities();
    assert!(caps.has_feeds);
    assert!(caps.has_collections);
    assert!(caps.has_saved_items);

    // Test provider health
    let health = provider.health_check().await.unwrap();
    assert!(health.is_healthy);

    // Test sync
    let sync_result = provider.sync().await.unwrap();
    assert!(sync_result.success);

    // Test feeds
    let feeds = provider.list_feeds().await.unwrap();
    assert_eq!(feeds.len(), 0); // Default implementation
}

#[test]
fn test_registry_with_bytecode_plugins() {
    let temp_dir = TempDir::new().unwrap();

    // Plugin with bytecode
    let plugin1 = create_test_plugin(
        temp_dir.path(),
        "with-bytecode",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-1"),
            ..Default::default()
        },
    );
    create_bytecode_file(&plugin1, "plugin.fzb", "with-bytecode");

    // Plugin without bytecode
    let plugin2 = create_test_plugin(
        temp_dir.path(),
        "without-bytecode",
        PluginConfig {
            is_provider: true,
            provider_id: Some("provider-2"),
            ..Default::default()
        },
    );

    let mut registry = PluginRegistry::new();
    registry.load_plugin(&plugin1).unwrap();
    registry.load_plugin(&plugin2).unwrap();

    let info_list = registry.list_plugins();
    let with_bc = info_list.iter().find(|p| p.id == "with-bytecode").unwrap();
    let without_bc = info_list
        .iter()
        .find(|p| p.id == "without-bytecode")
        .unwrap();

    assert!(with_bc.has_bytecode);
    assert!(!without_bc.has_bytecode);
}

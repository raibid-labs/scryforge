//! Integration tests for fusabi-runtime plugin system.
//!
//! These tests cover:
//! - Plugin discovery from directories
//! - Manifest parsing and validation
//! - Bytecode loading and validation
//! - Capability set handling

use fusabi_runtime::bytecode::{Bytecode, BytecodeMetadata, Constant, Function, Instruction};
use fusabi_runtime::{
    discover_plugin, discover_plugins, BytecodeLoader, Capability, CapabilitySet, PluginManifest,
    RuntimeError,
};
use std::io::Write;
use std::path::{Path, PathBuf};
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

    if let Some(description) = config.description {
        manifest.push_str(&format!("description = \"{}\"\n", description));
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
        if config.has_communities {
            manifest.push_str("has_communities = true\n");
        }
    }

    if config.add_rate_limit {
        manifest.push_str("\n[rate_limit]\n");
        manifest.push_str("requests_per_second = 10.0\n");
        manifest.push_str("max_concurrent = 5\n");
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
    description: Option<&'a str>,
    entry_point: Option<&'a str>,
    capabilities: Vec<&'a str>,
    is_provider: bool,
    provider_id: Option<&'a str>,
    provider_display_name: Option<&'a str>,
    has_feeds: bool,
    has_collections: bool,
    has_saved_items: bool,
    has_communities: bool,
    add_rate_limit: bool,
}

/// Create a sample bytecode file in JSON format.
fn create_bytecode_file(dir: &Path, filename: &str, plugin_id: &str) -> PathBuf {
    let bytecode = Bytecode {
        version: 1,
        metadata: BytecodeMetadata {
            plugin_id: plugin_id.to_string(),
            plugin_version: "0.1.0".to_string(),
            compiled_at: Some("2025-01-01T00:00:00Z".to_string()),
            compiler_version: Some("1.0.0".to_string()),
        },
        constants: vec![
            Constant::String("Hello from plugin".to_string()),
            Constant::Int(42),
            Constant::Bool(true),
        ],
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            instructions: vec![
                Instruction::LoadConst { index: 0 },
                Instruction::Call {
                    name: "log".to_string(),
                    arg_count: 1,
                },
                Instruction::LoadConst { index: 1 },
                Instruction::Return,
            ],
            local_count: 0,
        }],
        entry_point: "main".to_string(),
    };

    let bytecode_path = dir.join(filename);
    let json = serde_json::to_vec_pretty(&bytecode).unwrap();
    std::fs::write(&bytecode_path, json).unwrap();
    bytecode_path
}

// ==============================================================================
// Plugin Discovery Tests
// ==============================================================================

#[test]
fn test_discover_single_plugin() {
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

    assert_eq!(plugin_path.id(), "test-plugin");
    assert_eq!(plugin_path.name(), "Test Plugin test-plugin");
    assert_eq!(plugin_path.version(), "0.1.0");
    assert!(plugin_path.enabled);
    assert_eq!(plugin_path.manifest.capabilities, vec!["network"]);
}

#[test]
fn test_discover_multiple_plugins() {
    let temp_dir = TempDir::new().unwrap();

    create_test_plugin(
        temp_dir.path(),
        "plugin-a",
        PluginConfig {
            is_provider: true,
            ..Default::default()
        },
    );

    create_test_plugin(
        temp_dir.path(),
        "plugin-b",
        PluginConfig {
            is_provider: true,
            ..Default::default()
        },
    );

    create_test_plugin(
        temp_dir.path(),
        "plugin-c",
        PluginConfig {
            plugin_type: Some("action"),
            ..Default::default()
        },
    );

    // Set plugin directory temporarily for discovery
    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let plugins_dir = temp_dir.path().join("scryforge/plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    for plugin in &["plugin-a", "plugin-b", "plugin-c"] {
        let src = temp_dir.path().join(plugin);
        let dst = plugins_dir.join(plugin);
        copy_dir_all(&src, &dst).unwrap();
    }

    let discovered = discover_plugins().unwrap();
    std::env::remove_var("XDG_DATA_HOME");

    assert_eq!(discovered.len(), 3);

    let ids: Vec<&str> = discovered.iter().map(|p| p.id()).collect();
    assert!(ids.contains(&"plugin-a"));
    assert!(ids.contains(&"plugin-b"));
    assert!(ids.contains(&"plugin-c"));
}

#[test]
fn test_discover_plugin_with_missing_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("invalid-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let result = discover_plugin(&plugin_dir);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RuntimeError::Io(_)));
}

#[test]
fn test_discover_plugin_with_invalid_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("invalid-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let manifest_path = plugin_dir.join("manifest.toml");
    let mut file = std::fs::File::create(&manifest_path).unwrap();
    file.write_all(b"invalid toml content [[[").unwrap();

    let result = discover_plugin(&plugin_dir);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RuntimeError::Toml(_)));
}

// ==============================================================================
// Manifest Parsing Tests
// ==============================================================================

#[test]
fn test_parse_minimal_manifest() {
    let toml = r#"
[plugin]
id = "minimal"
name = "Minimal Plugin"
version = "1.0.0"
"#;

    let manifest = PluginManifest::from_str(toml).unwrap();
    assert_eq!(manifest.plugin.id, "minimal");
    assert_eq!(manifest.plugin.name, "Minimal Plugin");
    assert_eq!(manifest.plugin.version, "1.0.0");
    assert_eq!(manifest.capabilities.len(), 0);
    assert!(manifest.provider.is_none());
}

#[test]
fn test_parse_full_manifest() {
    let toml = r#"
capabilities = ["network", "credentials", "cache_read", "cache_write"]

[plugin]
id = "full-plugin"
name = "Full Featured Plugin"
version = "2.1.0"
description = "A fully configured test plugin"
authors = ["Test Author <test@example.com>"]
license = "MIT"
homepage = "https://example.com"
repository = "https://github.com/example/plugin"
plugin_type = "provider"
entry_point = "custom.fzb"

[provider]
id = "test-provider"
display_name = "Test Provider"
icon = "test-icon.png"
has_feeds = true
has_collections = true
has_saved_items = false
oauth_provider = "test-oauth"

[rate_limit]
requests_per_second = 5.0
max_concurrent = 3
retry_delay_ms = 1000
"#;

    let manifest = PluginManifest::from_str(toml).unwrap();

    assert_eq!(manifest.plugin.id, "full-plugin");
    assert_eq!(manifest.plugin.name, "Full Featured Plugin");
    assert_eq!(manifest.plugin.version, "2.1.0");
    assert_eq!(
        manifest.plugin.description,
        Some("A fully configured test plugin".to_string())
    );
    assert_eq!(manifest.plugin.authors.len(), 1);
    assert_eq!(manifest.plugin.license, Some("MIT".to_string()));
    assert_eq!(manifest.plugin.entry_point, Some("custom.fzb".to_string()));
    assert_eq!(manifest.entry_point(), "custom.fzb");

    assert_eq!(manifest.capabilities.len(), 4);
    assert!(manifest.capabilities.contains(&"network".to_string()));
    assert!(manifest.capabilities.contains(&"credentials".to_string()));

    let provider = manifest.provider.as_ref().unwrap();
    assert_eq!(provider.id, "test-provider");
    assert_eq!(provider.display_name, Some("Test Provider".to_string()));
    assert!(provider.has_feeds);
    assert!(provider.has_collections);
    assert!(!provider.has_saved_items);

    let rate_limit = manifest.rate_limit.as_ref().unwrap();
    assert_eq!(rate_limit.requests_per_second, Some(5.0));
    assert_eq!(rate_limit.max_concurrent, Some(3));
    assert_eq!(rate_limit.retry_delay_ms, Some(1000));
}

#[test]
fn test_manifest_validation_empty_id() {
    let toml = r#"
[plugin]
id = ""
name = "Test"
version = "1.0.0"
"#;

    let result = PluginManifest::from_str(toml);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RuntimeError::InvalidManifest(_)));
}

#[test]
fn test_manifest_validation_empty_name() {
    let toml = r#"
[plugin]
id = "test"
name = ""
version = "1.0.0"
"#;

    let result = PluginManifest::from_str(toml);
    assert!(result.is_err());
}

#[test]
fn test_manifest_validation_empty_version() {
    let toml = r#"
[plugin]
id = "test"
name = "Test"
version = ""
"#;

    let result = PluginManifest::from_str(toml);
    assert!(result.is_err());
}

#[test]
fn test_manifest_is_provider() {
    let provider_toml = r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"
plugin_type = "provider"

[provider]
id = "test-provider"
"#;

    let manifest = PluginManifest::from_str(provider_toml).unwrap();
    assert!(manifest.is_provider());

    let action_toml = r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"
plugin_type = "action"
"#;

    let manifest = PluginManifest::from_str(action_toml).unwrap();
    assert!(!manifest.is_provider());
}

// ==============================================================================
// Bytecode Loading Tests
// ==============================================================================

#[test]
fn test_load_bytecode_json() {
    let temp_dir = TempDir::new().unwrap();
    let bytecode_path = create_bytecode_file(temp_dir.path(), "plugin.fzb", "test-plugin");

    let bytecode = BytecodeLoader::load(&bytecode_path).unwrap();

    assert_eq!(bytecode.version, 1);
    assert_eq!(bytecode.metadata.plugin_id, "test-plugin");
    assert_eq!(bytecode.metadata.plugin_version, "0.1.0");
    assert_eq!(bytecode.constants.len(), 3);
    assert_eq!(bytecode.functions.len(), 1);
    assert_eq!(bytecode.entry_point, "main");
}

#[test]
fn test_validate_bytecode() {
    let bytecode = Bytecode {
        version: 1,
        metadata: BytecodeMetadata {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            compiled_at: None,
            compiler_version: None,
        },
        constants: vec![],
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            instructions: vec![Instruction::Return],
            local_count: 0,
        }],
        entry_point: "main".to_string(),
    };

    assert!(BytecodeLoader::validate(&bytecode).is_ok());
}

#[test]
fn test_validate_bytecode_wrong_version() {
    let bytecode = Bytecode {
        version: 99,
        metadata: BytecodeMetadata {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            compiled_at: None,
            compiler_version: None,
        },
        constants: vec![],
        functions: vec![],
        entry_point: "main".to_string(),
    };

    let result = BytecodeLoader::validate(&bytecode);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RuntimeError::BytecodeError(_)));
}

#[test]
fn test_validate_bytecode_missing_entry_point() {
    let bytecode = Bytecode {
        version: 1,
        metadata: BytecodeMetadata {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            compiled_at: None,
            compiler_version: None,
        },
        constants: vec![],
        functions: vec![Function {
            name: "other".to_string(),
            params: vec![],
            instructions: vec![],
            local_count: 0,
        }],
        entry_point: "main".to_string(),
    };

    let result = BytecodeLoader::validate(&bytecode);
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_bytecode_file() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_path = temp_dir.path().join("invalid.fzb");
    std::fs::write(&invalid_path, b"not json or bytecode").unwrap();

    let result = BytecodeLoader::load(&invalid_path);
    assert!(result.is_err());
}

// ==============================================================================
// Capability Tests
// ==============================================================================

#[test]
fn test_capability_from_str() {
    assert_eq!(Capability::from_str("network"), Capability::Network);
    assert_eq!(Capability::from_str("file_read"), Capability::FileRead);
    assert_eq!(Capability::from_str("credentials"), Capability::Credentials);
    assert_eq!(Capability::from_str("cache_read"), Capability::CacheRead);
    assert_eq!(
        Capability::from_str("custom_cap"),
        Capability::Custom("custom_cap".to_string())
    );
}

#[test]
fn test_capability_set_operations() {
    let mut caps = CapabilitySet::new();
    assert!(caps.is_empty());
    assert_eq!(caps.len(), 0);

    caps.add(Capability::Network);
    caps.add(Capability::Credentials);

    assert!(!caps.is_empty());
    assert_eq!(caps.len(), 2);
    assert!(caps.has(&Capability::Network));
    assert!(caps.has(&Capability::Credentials));
    assert!(!caps.has(&Capability::FileRead));
}

#[test]
fn test_capability_set_from_strings() {
    let caps = CapabilitySet::from_strings(["network", "credentials", "cache_read"]);

    assert_eq!(caps.len(), 3);
    assert!(caps.has(&Capability::Network));
    assert!(caps.has(&Capability::Credentials));
    assert!(caps.has(&Capability::CacheRead));
}

#[test]
fn test_capability_set_contains_all() {
    let superset = CapabilitySet::from_strings(["network", "credentials", "cache_read"]);
    let subset = CapabilitySet::from_strings(["network", "credentials"]);
    let disjoint = CapabilitySet::from_strings(["file_write"]);

    assert!(superset.contains_all(&subset));
    assert!(superset.contains_all(&superset));
    assert!(!subset.contains_all(&superset));
    assert!(!superset.contains_all(&disjoint));
}

#[test]
fn test_manifest_capability_set() {
    let toml = r#"
capabilities = ["network", "credentials", "cache_read"]

[plugin]
id = "test"
name = "Test"
version = "1.0.0"
"#;

    let manifest = PluginManifest::from_str(toml).unwrap();
    let caps = manifest.capability_set();

    assert_eq!(caps.len(), 3);
    assert!(caps.has(&Capability::Network));
    assert!(caps.has(&Capability::Credentials));
    assert!(caps.has(&Capability::CacheRead));
}

// ==============================================================================
// Plugin Lifecycle Tests
// ==============================================================================

#[test]
fn test_plugin_entry_point_detection() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig {
            entry_point: Some("custom.fzb"),
            ..Default::default()
        },
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    assert_eq!(plugin_path.manifest.entry_point(), "custom.fzb");
    assert!(!plugin_path.has_entry_point());

    // Create the entry point file
    create_bytecode_file(&plugin_dir, "custom.fzb", "test-plugin");
    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    assert!(plugin_path.has_entry_point());
}

#[test]
fn test_plugin_default_entry_point() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "test-plugin",
        PluginConfig::default(),
    );

    let plugin_path = discover_plugin(&plugin_dir).unwrap();
    assert_eq!(plugin_path.manifest.entry_point(), "plugin.fzb");
}

// ==============================================================================
// Complex Integration Tests
// ==============================================================================

#[test]
fn test_full_plugin_lifecycle_with_bytecode() {
    let temp_dir = TempDir::new().unwrap();

    // Create plugin with full configuration
    let plugin_dir = create_test_plugin(
        temp_dir.path(),
        "lifecycle-test",
        PluginConfig {
            name: Some("Lifecycle Test Plugin"),
            version: Some("1.2.3"),
            description: Some("Test plugin for lifecycle"),
            capabilities: vec!["network", "cache_read", "cache_write"],
            is_provider: true,
            provider_id: Some("lifecycle-provider"),
            provider_display_name: Some("Lifecycle Provider"),
            has_feeds: true,
            has_collections: true,
            ..Default::default()
        },
    );

    // Add bytecode
    create_bytecode_file(&plugin_dir, "plugin.fzb", "lifecycle-test");

    // Discover the plugin
    let plugin_path = discover_plugin(&plugin_dir).unwrap();

    // Verify all properties
    assert_eq!(plugin_path.id(), "lifecycle-test");
    assert_eq!(plugin_path.name(), "Lifecycle Test Plugin");
    assert_eq!(plugin_path.version(), "1.2.3");
    assert!(plugin_path.enabled);
    assert!(plugin_path.has_entry_point());

    // Verify manifest
    assert!(plugin_path.manifest.is_provider());
    assert_eq!(plugin_path.manifest.capabilities.len(), 3);

    let provider = plugin_path.manifest.provider.as_ref().unwrap();
    assert_eq!(provider.id, "lifecycle-provider");
    assert!(provider.has_feeds);
    assert!(provider.has_collections);

    // Load and validate bytecode
    let bytecode = BytecodeLoader::load(&plugin_path.entry_point_path()).unwrap();
    assert!(BytecodeLoader::validate(&bytecode).is_ok());
    assert_eq!(bytecode.metadata.plugin_id, "lifecycle-test");
}

#[test]
fn test_multiple_plugins_with_different_types() {
    let temp_dir = TempDir::new().unwrap();

    // Create various plugin types
    create_test_plugin(
        temp_dir.path(),
        "provider-plugin",
        PluginConfig {
            plugin_type: Some("provider"),
            is_provider: true,
            has_feeds: true,
            ..Default::default()
        },
    );

    create_test_plugin(
        temp_dir.path(),
        "action-plugin",
        PluginConfig {
            plugin_type: Some("action"),
            capabilities: vec!["clipboard"],
            ..Default::default()
        },
    );

    create_test_plugin(
        temp_dir.path(),
        "theme-plugin",
        PluginConfig {
            plugin_type: Some("theme"),
            ..Default::default()
        },
    );

    create_test_plugin(
        temp_dir.path(),
        "extension-plugin",
        PluginConfig {
            plugin_type: Some("extension"),
            capabilities: vec!["notifications"],
            ..Default::default()
        },
    );

    // Verify each plugin
    let provider = discover_plugin(&temp_dir.path().join("provider-plugin")).unwrap();
    assert!(provider.manifest.is_provider());

    let action = discover_plugin(&temp_dir.path().join("action-plugin")).unwrap();
    assert!(!action.manifest.is_provider());

    let theme = discover_plugin(&temp_dir.path().join("theme-plugin")).unwrap();
    assert!(!theme.manifest.is_provider());

    let extension = discover_plugin(&temp_dir.path().join("extension-plugin")).unwrap();
    assert!(!extension.manifest.is_provider());
}

// ==============================================================================
// Helper Functions
// ==============================================================================

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

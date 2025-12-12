//! Plugin manifest parsing.
//!
//! Each plugin has a `manifest.toml` file that describes its metadata,
//! capabilities, and configuration.

use crate::capability::CapabilitySet;
use crate::error::{RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Plugin manifest structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata.
    pub plugin: PluginMetadata,

    /// Required capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Provider-specific configuration.
    #[serde(default)]
    pub provider: Option<ProviderConfig>,

    /// Rate limiting configuration.
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// Custom configuration key-value pairs.
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
}

/// Plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique identifier for the plugin.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Version string (semver).
    pub version: String,

    /// Plugin description.
    #[serde(default)]
    pub description: Option<String>,

    /// Plugin author(s).
    #[serde(default)]
    pub authors: Vec<String>,

    /// License identifier.
    #[serde(default)]
    pub license: Option<String>,

    /// Homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,

    /// Repository URL.
    #[serde(default)]
    pub repository: Option<String>,

    /// Plugin type (provider, action, theme, etc.).
    #[serde(default = "default_plugin_type")]
    pub plugin_type: PluginType,

    /// Entry point file (defaults to plugin.fzb or plugin.fsx).
    #[serde(default)]
    pub entry_point: Option<String>,
}

fn default_plugin_type() -> PluginType {
    PluginType::Provider
}

/// Type of plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// A data provider plugin.
    Provider,
    /// A custom action plugin.
    Action,
    /// A theme/appearance plugin.
    Theme,
    /// A generic extension plugin.
    Extension,
}

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider ID to register as.
    pub id: String,

    /// Display name for the provider.
    #[serde(default)]
    pub display_name: Option<String>,

    /// Icon name or path.
    #[serde(default)]
    pub icon: Option<String>,

    /// Whether this provider supports feeds.
    #[serde(default)]
    pub has_feeds: bool,

    /// Whether this provider supports collections.
    #[serde(default)]
    pub has_collections: bool,

    /// Whether this provider supports saved items.
    #[serde(default)]
    pub has_saved_items: bool,

    /// Whether this provider supports communities.
    #[serde(default)]
    pub has_communities: bool,

    /// OAuth provider name (for Sigilforge integration).
    #[serde(default)]
    pub oauth_provider: Option<String>,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per second.
    #[serde(default)]
    pub requests_per_second: Option<f64>,

    /// Maximum concurrent requests.
    #[serde(default)]
    pub max_concurrent: Option<u32>,

    /// Retry delay in milliseconds after rate limit.
    #[serde(default)]
    pub retry_delay_ms: Option<u64>,
}

impl PluginManifest {
    /// Load a manifest from a TOML file.
    pub fn from_file(path: &Path) -> RuntimeResult<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Parse a manifest from a TOML string.
    pub fn from_str(content: &str) -> RuntimeResult<Self> {
        let manifest: PluginManifest = toml::from_str(content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest.
    fn validate(&self) -> RuntimeResult<()> {
        if self.plugin.id.is_empty() {
            return Err(RuntimeError::InvalidManifest(
                "Plugin ID cannot be empty".to_string(),
            ));
        }

        if self.plugin.name.is_empty() {
            return Err(RuntimeError::InvalidManifest(
                "Plugin name cannot be empty".to_string(),
            ));
        }

        if self.plugin.version.is_empty() {
            return Err(RuntimeError::InvalidManifest(
                "Plugin version cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the capability set for this plugin.
    pub fn capability_set(&self) -> CapabilitySet {
        CapabilitySet::from_strings(&self.capabilities)
    }

    /// Check if this is a provider plugin.
    pub fn is_provider(&self) -> bool {
        self.plugin.plugin_type == PluginType::Provider
    }

    /// Get the entry point file name.
    pub fn entry_point(&self) -> &str {
        self.plugin.entry_point.as_deref().unwrap_or("plugin.fzb")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml = r#"
capabilities = ["network", "credentials"]

[plugin]
id = "test-provider"
name = "Test Provider"
version = "0.1.0"
description = "A test provider"
plugin_type = "provider"

[provider]
id = "test"
has_feeds = true
oauth_provider = "test-oauth"

[rate_limit]
requests_per_second = 10.0
max_concurrent = 5
"#;

        let manifest = PluginManifest::from_str(toml).unwrap();
        assert_eq!(manifest.plugin.id, "test-provider");
        assert_eq!(manifest.plugin.name, "Test Provider");
        assert!(manifest.is_provider());
        assert_eq!(manifest.capabilities.len(), 2);

        let provider = manifest.provider.unwrap();
        assert_eq!(provider.id, "test");
        assert!(provider.has_feeds);

        let rate_limit = manifest.rate_limit.unwrap();
        assert_eq!(rate_limit.requests_per_second, Some(10.0));
    }

    #[test]
    fn test_invalid_manifest() {
        let toml = r#"
[plugin]
id = ""
name = "Test"
version = "0.1.0"
"#;

        let result = PluginManifest::from_str(toml);
        assert!(result.is_err());
    }
}

//! Configuration file loading and management
//!
//! This module handles loading and parsing the daemon configuration from
//! `$XDG_CONFIG_HOME/scryforge/config.toml`. If the configuration file doesn't
//! exist, a default configuration is created with documented comments.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Main daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Daemon-specific configuration
    pub daemon: DaemonConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Provider-specific configurations
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

/// Daemon server configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonConfig {
    /// Bind address for the JSON-RPC server
    /// Default: "127.0.0.1:3030"
    pub bind_address: String,
    /// Log level (trace, debug, info, warn, error)
    /// Default: "info"
    pub log_level: String,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    /// Path to cache database (SQLite)
    /// If None, uses XDG_DATA_HOME/scryforge/cache.db
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Maximum number of items to keep per stream
    /// Default: 1000
    pub max_items_per_stream: usize,
}

/// Per-provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Sync interval in minutes
    pub sync_interval_minutes: u64,
    /// Provider-specific settings as arbitrary TOML value
    #[serde(default = "default_settings")]
    pub settings: toml::Value,
}

fn default_settings() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig::default(),
            cache: CacheConfig::default(),
            providers: HashMap::new(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:3030".to_string(),
            log_level: "info".to_string(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            path: None,
            max_items_per_stream: 1000,
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sync_interval_minutes: 15,
            settings: toml::Value::Table(toml::map::Map::new()),
        }
    }
}

impl Config {
    /// Load configuration from the specified path
    ///
    /// # Arguments
    /// * `path` - Path to the configuration file
    ///
    /// # Returns
    /// The parsed configuration or an error if loading/parsing fails
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from the default XDG config location
    ///
    /// If the configuration file doesn't exist, creates a default configuration
    /// file with documented comments.
    ///
    /// # Returns
    /// The parsed configuration or an error if loading/parsing fails
    pub fn load_default() -> Result<Self> {
        let config_path = Self::default_config_path()?;

        if !config_path.exists() {
            Self::create_default_file(&config_path)?;
        }

        Self::load(&config_path)
    }

    /// Get the default configuration file path
    ///
    /// Returns `$XDG_CONFIG_HOME/scryforge/config.toml`
    pub fn default_config_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "raibid-labs", "scryforge")
            .context("Failed to determine project directories")?;

        Ok(dirs.config_dir().join("config.toml"))
    }

    /// Create a default configuration file with documented comments
    fn create_default_file(path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let default_config = Self::default_config_content();
        fs::write(path, default_config)
            .with_context(|| format!("Failed to write default config file: {}", path.display()))?;

        tracing::info!("Created default configuration file at: {}", path.display());
        Ok(())
    }

    /// Generate the default configuration file content with comments
    fn default_config_content() -> String {
        r#"# Scryforge Daemon Configuration
# This file configures the scryforge-daemon behavior.

[daemon]
# Bind address for the JSON-RPC API server
# Default: "127.0.0.1:3030"
bind_address = "127.0.0.1:3030"

# Log level: trace, debug, info, warn, error
# Default: "info"
log_level = "info"

[cache]
# Path to the SQLite cache database
# If not specified, defaults to $XDG_DATA_HOME/scryforge/cache.db
# path = "/path/to/cache.db"

# Maximum number of items to keep per stream
# Default: 1000
max_items_per_stream = 1000

# Provider-specific configurations
# Each provider can be configured with:
# - enabled: Whether the provider is enabled (default: true)
# - sync_interval_minutes: How often to sync data (default: 15)
# - settings: Provider-specific settings (varies by provider)

# Example: Dummy provider configuration
[providers.dummy]
enabled = true
sync_interval_minutes = 15

# Provider-specific settings are defined here
[providers.dummy.settings]
# Add provider-specific settings as needed
# For the dummy provider, no special settings are required

# Example: Future RSS provider configuration
# [providers.rss]
# enabled = true
# sync_interval_minutes = 30
#
# [providers.rss.settings]
# feeds = [
#     "https://example.com/feed.xml",
#     "https://another.example.com/rss",
# ]

# Example: Future email provider configuration
# [providers.email]
# enabled = true
# sync_interval_minutes = 5
#
# [providers.email.settings]
# imap_server = "imap.example.com"
# imap_port = 993
# use_tls = true
"#
        .to_string()
    }

    /// Validate the configuration
    ///
    /// Ensures all configuration values are valid and within acceptable ranges.
    pub fn validate(&self) -> Result<()> {
        // Validate bind address format
        self.daemon
            .bind_address
            .parse::<std::net::SocketAddr>()
            .with_context(|| format!("Invalid bind_address: {}", self.daemon.bind_address))?;

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.daemon.log_level.as_str()) {
            anyhow::bail!(
                "Invalid log_level: {}. Must be one of: {}",
                self.daemon.log_level,
                valid_log_levels.join(", ")
            );
        }

        // Validate cache settings
        if self.cache.max_items_per_stream == 0 {
            anyhow::bail!("cache.max_items_per_stream must be greater than 0");
        }

        // Validate provider configurations
        for (provider_id, provider_config) in &self.providers {
            if provider_config.sync_interval_minutes == 0 {
                anyhow::bail!(
                    "Provider '{}': sync_interval_minutes must be greater than 0",
                    provider_id
                );
            }
        }

        Ok(())
    }

    /// Get the cache database path
    ///
    /// Returns the configured cache path or the default XDG data directory path
    pub fn cache_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.cache.path {
            return Ok(path.clone());
        }

        let dirs = directories::ProjectDirs::from("", "raibid-labs", "scryforge")
            .context("Failed to determine project directories")?;

        Ok(dirs.data_dir().join("cache.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.daemon.bind_address, "127.0.0.1:3030");
        assert_eq!(config.daemon.log_level, "info");
        assert_eq!(config.cache.max_items_per_stream, 1000);
        assert!(config.cache.path.is_none());
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_default_daemon_config() {
        let config = DaemonConfig::default();
        assert_eq!(config.bind_address, "127.0.0.1:3030");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_default_cache_config() {
        let config = CacheConfig::default();
        assert_eq!(config.max_items_per_stream, 1000);
        assert!(config.path.is_none());
    }

    #[test]
    fn test_default_provider_config() {
        let config = ProviderConfig::default();
        assert!(config.enabled);
        assert_eq!(config.sync_interval_minutes, 15);
    }

    #[test]
    fn test_load_valid_config() {
        let config_content = r#"
[daemon]
bind_address = "0.0.0.0:8080"
log_level = "debug"

[cache]
max_items_per_stream = 500

[providers.dummy]
enabled = true
sync_interval_minutes = 10

[providers.dummy.settings]
foo = "bar"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(temp_file.path()).unwrap();
        assert_eq!(config.daemon.bind_address, "0.0.0.0:8080");
        assert_eq!(config.daemon.log_level, "debug");
        assert_eq!(config.cache.max_items_per_stream, 500);
        assert!(config.providers.contains_key("dummy"));

        let dummy_config = &config.providers["dummy"];
        assert!(dummy_config.enabled);
        assert_eq!(dummy_config.sync_interval_minutes, 10);
    }

    #[test]
    fn test_load_minimal_config() {
        let config_content = r#"
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "info"

[cache]
max_items_per_stream = 1000
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(temp_file.path()).unwrap();
        assert_eq!(config.daemon.bind_address, "127.0.0.1:3030");
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_bind_address() {
        let mut config = Config::default();
        config.daemon.bind_address = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = Config::default();
        config.daemon.log_level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_max_items() {
        let mut config = Config::default();
        config.cache.max_items_per_stream = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_sync_interval() {
        let mut config = Config::default();
        config.providers.insert(
            "test".to_string(),
            ProviderConfig {
                enabled: true,
                sync_interval_minutes: 0,
                settings: toml::Value::Table(toml::map::Map::new()),
            },
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cache_path_default() {
        let config = Config::default();
        let cache_path = config.cache_path().unwrap();
        assert!(cache_path.to_string_lossy().contains("scryforge"));
        assert!(cache_path.to_string_lossy().ends_with("cache.db"));
    }

    #[test]
    fn test_cache_path_custom() {
        let mut config = Config::default();
        let custom_path = PathBuf::from("/custom/path/cache.db");
        config.cache.path = Some(custom_path.clone());
        assert_eq!(config.cache_path().unwrap(), custom_path);
    }

    #[test]
    fn test_provider_config_serialization() {
        let mut settings = toml::map::Map::new();
        settings.insert("key".to_string(), toml::Value::String("value".to_string()));

        let provider_config = ProviderConfig {
            enabled: false,
            sync_interval_minutes: 30,
            settings: toml::Value::Table(settings),
        };

        let toml_str = toml::to_string(&provider_config).unwrap();
        let deserialized: ProviderConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(provider_config, deserialized);
    }

    #[test]
    fn test_full_config_roundtrip() {
        let mut config = Config::default();
        config.daemon.log_level = "debug".to_string();

        let mut provider_config = ProviderConfig::default();
        provider_config.sync_interval_minutes = 20;
        config.providers.insert("test".to_string(), provider_config);

        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config, deserialized);
    }
}

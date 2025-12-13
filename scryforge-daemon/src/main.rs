//! # scryforge-daemon
//!
//! The Scryforge hub daemon.
//!
//! This daemon is responsible for:
//! - Loading and managing provider plugins
//! - Periodic sync and caching of stream data
//! - Token retrieval from Sigilforge for OAuth providers
//! - Exposing the daemon API over Unix socket for TUI and other clients
//! - Managing local state (SQLite cache)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    scryforge-daemon                          │
//! │                                                              │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
//! │  │   Provider   │  │   Provider   │  │   Provider   │      │
//! │  │   Registry   │  │   Sync Loop  │  │   Cache      │      │
//! │  └──────────────┘  └──────────────┘  └──────────────┘      │
//! │                                                              │
//! │  ┌────────────────────────────────────────────────────┐    │
//! │  │              JSON-RPC API Server                    │    │
//! │  │           (Unix socket / TCP)                       │    │
//! │  └────────────────────────────────────────────────────┘    │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!           ┌──────────────────┼──────────────────┐
//!           ▼                  ▼                  ▼
//!      ┌─────────┐      ┌─────────────┐    ┌──────────┐
//!      │   TUI   │      │   Scarab    │    │  Other   │
//!      │ Client  │      │   (future)  │    │ Clients  │
//!      └─────────┘      └─────────────┘    └──────────┘
//! ```
//!
//! ## Configuration
//!
//! The daemon reads configuration from `$XDG_CONFIG_HOME/scryforge/config.toml`.
//! See `docs/ARCHITECTURE.md` for configuration options.
//!
//! ## Running
//!
//! ```bash
//! # Start the daemon
//! cargo run --bin scryforge-daemon
//!
//! # With debug logging
//! RUST_LOG=debug cargo run --bin scryforge-daemon
//! ```

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use modules from the library crate
use scryforge_daemon::api;
use scryforge_daemon::cache::SqliteCache;
use scryforge_daemon::config::Config;
use scryforge_daemon::plugin::PluginManager;
use scryforge_daemon::registry::ProviderRegistry;
use scryforge_daemon::sync::SyncManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .init();

    info!("Starting scryforge-daemon v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration from config.toml
    let config = match Config::load_default() {
        Ok(cfg) => {
            info!("Loaded configuration from default path");
            cfg
        }
        Err(e) => {
            info!("Failed to load config, using defaults: {}", e);
            Config::default()
        }
    };

    // Initialize plugin manager
    let mut plugin_manager = PluginManager::new();

    // Discover and load plugins
    match plugin_manager.discover_and_load() {
        Ok(count) => info!("Loaded {} plugin(s)", count),
        Err(e) => info!("Plugin discovery: {}", e),
    }

    // List loaded plugins
    let plugins = plugin_manager.list_plugins();
    for plugin in &plugins {
        info!(
            "Plugin: {} v{} ({:?}) - provider: {}, bytecode: {}",
            plugin.name, plugin.version, plugin.status, plugin.is_provider, plugin.has_bytecode
        );
    }

    // Initialize provider registry
    let mut registry = ProviderRegistry::new();

    // Load dummy provider for testing
    info!("Loading dummy provider...");
    registry.register(provider_dummy::DummyProvider::new());

    // Register plugin-based providers
    plugin_manager.register_providers(&mut registry);

    // Display registered providers
    let provider_ids = registry.list();
    info!(
        "Registered {} provider(s): {:?}",
        provider_ids.len(),
        provider_ids
    );

    // Verify dummy provider is accessible
    if let Some(provider) = registry.get("dummy") {
        info!("Dummy provider loaded: {}", provider.name());

        // Perform health check
        match provider.health_check().await {
            Ok(health) => {
                info!(
                    "Provider health check: healthy={}, message={:?}",
                    health.is_healthy, health.message
                );
            }
            Err(e) => {
                info!("Provider health check failed: {}", e);
            }
        }
    }

    // Initialize cache (SQLite)
    let cache_path = config.cache_path()?;
    info!("Initializing cache at: {}", cache_path.display());
    let cache = match SqliteCache::open_at(&cache_path) {
        Ok(c) => {
            info!("Cache initialized successfully");
            Arc::new(c)
        }
        Err(e) => {
            info!("Failed to initialize cache: {}", e);
            return Err(e);
        }
    };

    // Wrap registry in Arc for sharing with sync manager and API
    let registry = Arc::new(registry);

    // Start sync manager with background sync tasks
    let mut sync_manager = SyncManager::new(config.clone(), Arc::clone(&registry), Arc::clone(&cache));
    match sync_manager.start().await {
        Ok(_) => info!("Sync manager started successfully"),
        Err(e) => info!("Sync manager startup: {}", e),
    }

    // Start the JSON-RPC API server
    let (server_handle, addr) = api::start_server().await?;

    info!("Daemon startup complete");
    info!("Listening on: {}", addr);
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");

    // Shutdown sync manager gracefully
    sync_manager.shutdown().await;

    // Stop the server gracefully
    server_handle.stop()?;

    info!("Daemon stopped");
    Ok(())
}

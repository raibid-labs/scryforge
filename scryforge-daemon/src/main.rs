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
//! # With custom config file
//! cargo run --bin scryforge-daemon -- --config /path/to/config.toml
//!
//! # With debug logging
//! RUST_LOG=debug cargo run --bin scryforge-daemon
//! ```

pub mod cache;
pub mod config;
pub mod registry;
pub mod sync;

use anyhow::Result;
use cache::{Cache, SqliteCache};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

mod api;

/// Scryforge daemon - manages providers, caching, and local API
#[derive(Parser, Debug)]
#[command(name = "scryforge-daemon")]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    ///
    /// If not specified, uses $XDG_CONFIG_HOME/scryforge/config.toml
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Load configuration
    let config = if let Some(config_path) = args.config {
        config::Config::load(&config_path)?
    } else {
        config::Config::load_default()?
    };

    // Initialize logging with configured log level
    let log_level = match config.daemon.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(true)
        .init();

    info!("Starting scryforge-daemon v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration loaded successfully");
    info!("  Bind address: {}", config.daemon.bind_address);
    info!("  Log level: {}", config.daemon.log_level);
    info!("  Cache max items per stream: {}", config.cache.max_items_per_stream);
    info!("  Cache path: {}", config.cache_path()?.display());
    info!("  Configured providers: {}", config.providers.len());

    // Initialize provider registry
    let mut registry = registry::ProviderRegistry::new();

    // Load dummy provider for testing
    info!("Loading dummy provider...");
    registry.register(provider_dummy::DummyProvider::new());

    // Display registered providers
    let provider_ids = registry.list();
    info!("Registered {} provider(s): {:?}", provider_ids.len(), provider_ids);

    // Verify dummy provider is accessible
    if let Some(provider) = registry.get("dummy") {
        info!("Dummy provider loaded: {}", provider.name());

        // Perform health check
        match provider.health_check().await {
            Ok(health) => {
                info!("Provider health check: healthy={}, message={:?}",
                      health.is_healthy, health.message);
            }
            Err(e) => {
                info!("Provider health check failed: {}", e);
            }
        }
    }

    // Initialize cache (SQLite)
    info!("Initializing SQLite cache...");
    let cache = match SqliteCache::open() {
        Ok(cache) => {
            info!("Cache initialized successfully");

            // Log cache statistics
            match cache.get_streams(None) {
                Ok(streams) => {
                    info!("Cache contains {} streams", streams.len());
                }
                Err(e) => {
                    warn!("Failed to query cache streams: {}", e);
                }
            }

            Some(Arc::new(cache))
        }
        Err(e) => {
            warn!("Failed to initialize cache: {}", e);
            warn!("Continuing without cache support");
            None
        }
    };

    // TODO: Connect to Sigilforge for auth
    // let sigilforge = SigilforgeClient::connect(&config.sigilforge_socket)?;

    // Initialize and start sync manager
    let sync_manager = if let Some(cache_arc) = cache {
        info!("Starting sync manager...");
        let registry_arc = Arc::new(registry);
        let mut manager = sync::SyncManager::new(
            config.clone(),
            Arc::clone(&registry_arc),
            Arc::clone(&cache_arc),
        );

        // Start the sync tasks
        if let Err(e) = manager.start().await {
            warn!("Failed to start sync manager: {}", e);
            None
        } else {
            info!("Sync manager started successfully");
            Some(Arc::new(RwLock::new(manager)))
        }
    } else {
        warn!("Cache not available, skipping sync manager initialization");
        None
    };

    // Start the JSON-RPC API server
    let (server_handle, addr) = api::start_server(sync_manager.clone()).await?;

    info!("Daemon startup complete");
    info!("Listening on: {}", addr);
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");

    // Stop the sync manager gracefully
    if let Some(sm) = sync_manager {
        info!("Stopping sync manager...");
        let mut manager = sm.write().await;
        manager.shutdown().await;
    }

    // Stop the server gracefully
    server_handle.stop()?;

    info!("Daemon stopped");
    Ok(())
}

// ============================================================================
// TODO: Module stubs for future implementation
// ============================================================================

// mod sync {
//     //! Background sync loop for fetching new data
// }

// mod sigilforge {
//     //! Client for communicating with Sigilforge auth daemon
// }

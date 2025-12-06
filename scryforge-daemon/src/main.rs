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
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod api;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .init();

    info!("Starting scryforge-daemon v{}", env!("CARGO_PKG_VERSION"));

    // Start the JSON-RPC API server
    let (server_handle, addr) = api::start_server().await?;

    info!("Daemon startup complete");
    info!("Listening on: {}", addr);
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");

    // Stop the server gracefully
    server_handle.stop()?;

    info!("Daemon stopped");
    Ok(())
}

// ============================================================================
// TODO: Module stubs for future implementation
// ============================================================================

// mod config {
//     //! Configuration loading and management
// }

// mod registry {
//     //! Provider registry for managing loaded providers
// }

// mod cache {
//     //! SQLite-based caching for items and streams
// }

// mod sync {
//     //! Background sync loop for fetching new data
// }

// mod sigilforge {
//     //! Client for communicating with Sigilforge auth daemon
// }

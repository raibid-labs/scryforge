//! JSON-RPC server implementation for scryforge-daemon.
//!
//! This module provides the server that listens on TCP localhost and handles
//! incoming JSON-RPC requests from clients.

use anyhow::{Context, Result};
use jsonrpsee::server::{Server, ServerHandle};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::handlers::{ApiImpl, ScryforgeApiServer};
use crate::cache::Cache;
use crate::sync::SyncManager;

/// Start the JSON-RPC API server on TCP localhost.
///
/// This function creates a TCP listener on 127.0.0.1:3030 and starts the JSON-RPC server.
/// It returns a handle that can be used to gracefully shut down the server.
///
/// # Arguments
///
/// * `sync_manager` - Optional sync manager to enable sync RPC methods
///
/// # Returns
///
/// A tuple of (ServerHandle, SocketAddr) - the handle keeps the server running,
/// and the address shows where it's listening
pub async fn start_server<C: Cache + 'static>(
    sync_manager: Option<Arc<RwLock<SyncManager<C>>>>,
) -> Result<(ServerHandle, std::net::SocketAddr)> {
    info!("Starting JSON-RPC server on 127.0.0.1:3030");

    // Create API implementation
    let api = if let Some(sm) = sync_manager {
        ApiImpl::with_sync_manager(sm)
    } else {
        ApiImpl::new()
    };

    // Build the server on localhost
    let server = Server::builder()
        .build("127.0.0.1:3030")
        .await
        .context("Failed to build JSON-RPC server")?;

    // Get the address
    let addr = server.local_addr()
        .context("Failed to get server address")?;
    info!("JSON-RPC server listening on {}", addr);

    // Start the server with the API methods
    let handle = server.start(api.into_rpc());

    info!("JSON-RPC server started successfully");

    Ok((handle, addr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_starts() {
        let result = start_server::<crate::cache::SqliteCache>(None).await;
        assert!(result.is_ok(), "Server should start successfully");

        // Clean up
        let (handle, _addr) = result.unwrap();
        handle.stop().unwrap();
    }
}

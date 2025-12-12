//! Client for communicating with the scryforge-daemon.
//!
//! This module provides an async client for fetching streams and items
//! from the daemon via JSON-RPC over HTTP.

use anyhow::{Context, Result};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use scryforge_provider_core::{Collection, Item, Stream};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Messages sent from the UI thread to the async client thread.
#[derive(Debug, Clone)]
pub enum Command {
    /// Fetch all available streams
    FetchStreams,
    /// Fetch items for a specific stream
    FetchItems(String),
    /// Mark an item as read
    MarkItemRead(String),
    /// Mark an item as unread
    MarkItemUnread(String),
    /// Archive an item
    ArchiveItem(String),
    /// Save an item
    SaveItem(String),
    /// Unsave an item
    UnsaveItem(String),
    /// Fetch all collections
    FetchCollections,
    /// Add item to collection
    AddToCollection { collection_id: String, item_id: String },
    /// Remove item from collection
    RemoveFromCollection { collection_id: String, item_id: String },
    /// Create a new collection
    CreateCollection(String),
    /// Shutdown the client
    Shutdown,
}

/// Messages sent from the async client thread to the UI thread.
#[derive(Debug, Clone)]
pub enum Message {
    /// Streams were loaded successfully
    StreamsLoaded(Vec<Stream>),
    /// Items were loaded successfully
    ItemsLoaded(Vec<Item>),
    /// Collections were loaded successfully
    CollectionsLoaded(Vec<Collection>),
    /// Collection created successfully
    CollectionCreated(Collection),
    /// Item added to collection
    ItemAddedToCollection,
    /// Item removed from collection
    ItemRemovedFromCollection,
    /// An error occurred
    Error(String),
    /// Client is ready
    Ready,
    /// Client disconnected
    Disconnected,
}

/// Client for communicating with the scryforge-daemon.
pub struct DaemonClient {
    client: HttpClient,
}

impl DaemonClient {
    /// Connect to the daemon via HTTP.
    pub async fn connect(url: &str) -> Result<Self> {
        info!("Connecting to daemon at {}", url);

        // Build HTTP client
        let client = HttpClientBuilder::default()
            .build(url)
            .context("Failed to build HTTP client")?;

        info!("Connected to daemon successfully");

        Ok(Self { client })
    }

    /// List all available streams.
    pub async fn list_streams(&self) -> Result<Vec<Stream>> {
        debug!("Fetching streams from daemon");

        let streams: Vec<Stream> = self
            .client
            .request("streams.list", rpc_params![])
            .await
            .context("Failed to fetch streams")?;

        debug!("Fetched {} streams", streams.len());
        Ok(streams)
    }

    /// List items for a specific stream.
    pub async fn list_items(&self, stream_id: &str) -> Result<Vec<Item>> {
        debug!("Fetching items for stream: {}", stream_id);

        let items: Vec<Item> = self
            .client
            .request("items.list", rpc_params![stream_id])
            .await
            .context("Failed to fetch items")?;

        debug!("Fetched {} items for stream {}", items.len(), stream_id);
        Ok(items)
    }

    /// Save an item.
    pub async fn save_item(&self, item_id: &str) -> Result<()> {
        debug!("Saving item: {}", item_id);

        self.client
            .request::<(), _>("items.save", rpc_params![item_id])
            .await
            .context("Failed to save item")?;

        debug!("Saved item {}", item_id);
        Ok(())
    }

    /// Unsave an item.
    pub async fn unsave_item(&self, item_id: &str) -> Result<()> {
        debug!("Unsaving item: {}", item_id);

        self.client
            .request::<(), _>("items.unsave", rpc_params![item_id])
            .await
            .context("Failed to unsave item")?;

        debug!("Unsaved item {}", item_id);
        Ok(())
    }

    /// Mark an item as read.
    pub async fn mark_item_read(&self, item_id: &str) -> Result<()> {
        debug!("Marking item as read: {}", item_id);

        self.client
            .request::<(), _>("items.mark_read", rpc_params![item_id])
            .await
            .context("Failed to mark item as read")?;

        debug!("Marked item {} as read", item_id);
        Ok(())
    }

    /// Mark an item as unread.
    pub async fn mark_item_unread(&self, item_id: &str) -> Result<()> {
        debug!("Marking item as unread: {}", item_id);

        self.client
            .request::<(), _>("items.mark_unread", rpc_params![item_id])
            .await
            .context("Failed to mark item as unread")?;

        debug!("Marked item {} as unread", item_id);
        Ok(())
    }

    /// Archive an item.
    pub async fn archive_item(&self, item_id: &str) -> Result<()> {
        debug!("Archiving item: {}", item_id);

        self.client
            .request::<(), _>("items.archive", rpc_params![item_id])
            .await
            .context("Failed to archive item")?;

        debug!("Archived item {}", item_id);
        Ok(())
    }

    /// List all collections.
    pub async fn list_collections(&self) -> Result<Vec<Collection>> {
        debug!("Fetching collections from daemon");

        let collections: Vec<Collection> = self
            .client
            .request("collections.list", rpc_params![])
            .await
            .context("Failed to fetch collections")?;

        debug!("Fetched {} collections", collections.len());
        Ok(collections)
    }

    /// Add an item to a collection.
    pub async fn add_to_collection(&self, collection_id: &str, item_id: &str) -> Result<()> {
        debug!("Adding item {} to collection {}", item_id, collection_id);

        self.client
            .request::<(), _>("collections.add_item", rpc_params![collection_id, item_id])
            .await
            .context("Failed to add item to collection")?;

        debug!("Added item {} to collection {}", item_id, collection_id);
        Ok(())
    }

    /// Remove an item from a collection.
    pub async fn remove_from_collection(&self, collection_id: &str, item_id: &str) -> Result<()> {
        debug!("Removing item {} from collection {}", item_id, collection_id);

        self.client
            .request::<(), _>("collections.remove_item", rpc_params![collection_id, item_id])
            .await
            .context("Failed to remove item from collection")?;

        debug!("Removed item {} from collection {}", item_id, collection_id);
        Ok(())
    }

    /// Create a new collection.
    pub async fn create_collection(&self, name: &str) -> Result<Collection> {
        debug!("Creating collection: {}", name);

        let collection: Collection = self
            .client
            .request("collections.create", rpc_params![name])
            .await
            .context("Failed to create collection")?;

        debug!("Created collection: {} ({})", name, collection.id.0);
        Ok(collection)
    }
}

/// Spawn the daemon client task.
///
/// This function spawns a background task that handles communication with the daemon.
/// It receives commands from the UI thread via `cmd_rx` and sends responses back
/// via `msg_tx`.
///
/// # Arguments
///
/// * `url` - URL of the daemon's HTTP endpoint (e.g., "http://127.0.0.1:3030")
/// * `cmd_rx` - Receiver for commands from the UI thread
/// * `msg_tx` - Sender for messages to the UI thread
///
/// # Returns
///
/// A join handle for the spawned task
pub fn spawn_client_task(
    url: String,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    msg_tx: mpsc::UnboundedSender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Try to connect to the daemon
        let client = match DaemonClient::connect(&url).await {
            Ok(client) => {
                let _ = msg_tx.send(Message::Ready);
                client
            }
            Err(e) => {
                error!("Failed to connect to daemon: {}", e);
                let _ = msg_tx.send(Message::Error(format!(
                    "Failed to connect to daemon: {}",
                    e
                )));
                let _ = msg_tx.send(Message::Disconnected);
                return;
            }
        };

        // Process commands
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Command::FetchStreams => match client.list_streams().await {
                    Ok(streams) => {
                        let _ = msg_tx.send(Message::StreamsLoaded(streams));
                    }
                    Err(e) => {
                        error!("Failed to fetch streams: {}", e);
                        let _ =
                            msg_tx.send(Message::Error(format!("Failed to fetch streams: {}", e)));
                    }
                },
                Command::FetchItems(stream_id) => match client.list_items(&stream_id).await {
                    Ok(items) => {
                        let _ = msg_tx.send(Message::ItemsLoaded(items));
                    }
                    Err(e) => {
                        error!("Failed to fetch items: {}", e);
                        let _ =
                            msg_tx.send(Message::Error(format!("Failed to fetch items: {}", e)));
                    }
                },
                Command::SaveItem(item_id) => {
                    match client.save_item(&item_id).await {
                        Ok(()) => {
                            // Success - no message needed, just log
                            debug!("Successfully saved item {}", item_id);
                        }
                        Err(e) => {
                            error!("Failed to save item: {}", e);
                            let _ =
                                msg_tx.send(Message::Error(format!("Failed to save item: {}", e)));
                        }
                    }
                }
                Command::UnsaveItem(item_id) => {
                    match client.unsave_item(&item_id).await {
                        Ok(()) => {
                            // Success - no message needed, just log
                            debug!("Successfully unsaved item {}", item_id);
                        }
                        Err(e) => {
                            error!("Failed to unsave item: {}", e);
                            let _ = msg_tx
                                .send(Message::Error(format!("Failed to unsave item: {}", e)));
                        }
                    }
                }
                Command::MarkItemRead(item_id) => match client.mark_item_read(&item_id).await {
                    Ok(()) => {
                        debug!("Successfully marked item {} as read", item_id);
                    }
                    Err(e) => {
                        error!("Failed to mark item as read: {}", e);
                        let _ = msg_tx.send(Message::Error(format!(
                            "Failed to mark item as read: {}",
                            e
                        )));
                    }
                },
                Command::MarkItemUnread(item_id) => match client.mark_item_unread(&item_id).await {
                    Ok(()) => {
                        debug!("Successfully marked item {} as unread", item_id);
                    }
                    Err(e) => {
                        error!("Failed to mark item as unread: {}", e);
                        let _ = msg_tx.send(Message::Error(format!(
                            "Failed to mark item as unread: {}",
                            e
                        )));
                    }
                },
                Command::ArchiveItem(item_id) => match client.archive_item(&item_id).await {
                    Ok(()) => {
                        debug!("Successfully archived item {}", item_id);
                    }
                    Err(e) => {
                        error!("Failed to archive item: {}", e);
                        let _ =
                            msg_tx.send(Message::Error(format!("Failed to archive item: {}", e)));
                    }
                },
                Command::FetchCollections => match client.list_collections().await {
                    Ok(collections) => {
                        let _ = msg_tx.send(Message::CollectionsLoaded(collections));
                    }
                    Err(e) => {
                        error!("Failed to fetch collections: {}", e);
                        let _ = msg_tx.send(Message::Error(format!("Failed to fetch collections: {}", e)));
                    }
                },
                Command::AddToCollection { collection_id, item_id } => {
                    match client.add_to_collection(&collection_id, &item_id).await {
                        Ok(()) => {
                            debug!("Successfully added item {} to collection {}", item_id, collection_id);
                            let _ = msg_tx.send(Message::ItemAddedToCollection);
                        }
                        Err(e) => {
                            error!("Failed to add item to collection: {}", e);
                            let _ = msg_tx.send(Message::Error(format!("Failed to add item to collection: {}", e)));
                        }
                    }
                },
                Command::RemoveFromCollection { collection_id, item_id } => {
                    match client.remove_from_collection(&collection_id, &item_id).await {
                        Ok(()) => {
                            debug!("Successfully removed item {} from collection {}", item_id, collection_id);
                            let _ = msg_tx.send(Message::ItemRemovedFromCollection);
                        }
                        Err(e) => {
                            error!("Failed to remove item from collection: {}", e);
                            let _ = msg_tx.send(Message::Error(format!("Failed to remove item from collection: {}", e)));
                        }
                    }
                },
                Command::CreateCollection(name) => {
                    match client.create_collection(&name).await {
                        Ok(collection) => {
                            debug!("Successfully created collection: {}", name);
                            let _ = msg_tx.send(Message::CollectionCreated(collection));
                        }
                        Err(e) => {
                            error!("Failed to create collection: {}", e);
                            let _ = msg_tx.send(Message::Error(format!("Failed to create collection: {}", e)));
                        }
                    }
                },
                Command::Shutdown => {
                    info!("Shutting down daemon client");
                    break;
                }
            }
        }

        let _ = msg_tx.send(Message::Disconnected);
    })
}

/// Get the default daemon URL.
pub fn get_daemon_url() -> String {
    "http://127.0.0.1:3030".to_string()
}

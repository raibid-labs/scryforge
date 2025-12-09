//! # provider-bookmarks
//!
//! A provider for accessing local bookmarks stored in JSON format.
//!
//! This provider manages bookmarks stored in `$XDG_DATA_HOME/scryforge/bookmarks.json`
//! and implements both `HasCollections` (for bookmark folders) and `HasSavedItems`
//! (for all bookmarks) traits.
//!
//! ## Features
//!
//! - Local JSON storage for bookmarks
//! - Folder-based organization
//! - Tags and metadata support
//! - Full-text search capabilities
//! - Browser bookmark import (future)
//!
//! ## Storage Schema
//!
//! Bookmarks are stored in a simple JSON format:
//! ```json
//! {
//!   "bookmarks": [
//!     {
//!       "id": "uuid-v4",
//!       "url": "https://example.com",
//!       "title": "Example Site",
//!       "description": "Optional description",
//!       "tags": ["tag1", "tag2"],
//!       "folder": "Technical/Rust",
//!       "created_at": "2024-01-01T00:00:00Z",
//!       "updated_at": "2024-01-01T00:00:00Z"
//!     }
//!   ]
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusabi_streams_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum BookmarkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Bookmark not found: {0}")]
    NotFound(String),

    #[error("Invalid folder path: {0}")]
    InvalidFolder(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<BookmarkError> for StreamError {
    fn from(err: BookmarkError) -> Self {
        match err {
            BookmarkError::NotFound(msg) => StreamError::ItemNotFound(msg),
            _ => StreamError::Provider(err.to_string()),
        }
    }
}

// ============================================================================
// Storage Schema
// ============================================================================

/// A bookmark entry in the storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Bookmark {
    /// Create a new bookmark with generated ID and timestamps.
    pub fn new(url: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            title,
            description: None,
            tags: Vec::new(),
            folder: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Convert bookmark to Item for the streams API.
    fn to_item(&self, provider_id: &str) -> Item {
        let stream_id = if let Some(folder) = &self.folder {
            StreamId::new(provider_id, "collection", folder)
        } else {
            StreamId::new(provider_id, "saved", "all")
        };

        let mut metadata = HashMap::new();
        if let Some(folder) = &self.folder {
            metadata.insert("folder".to_string(), folder.clone());
        }
        metadata.insert("bookmark_id".to_string(), self.id.clone());

        Item {
            id: ItemId::new(provider_id, &self.id),
            stream_id,
            title: self.title.clone(),
            content: ItemContent::Bookmark {
                description: self.description.clone(),
            },
            author: None,
            published: Some(self.created_at),
            updated: Some(self.updated_at),
            url: Some(self.url.clone()),
            thumbnail_url: None,
            is_read: false,
            is_saved: true,
            tags: self.tags.clone(),
            metadata,
        }
    }
}

/// Root storage structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkStore {
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }
}

impl BookmarkStore {
    /// Load bookmark store from a file.
    pub async fn load(path: &Path) -> std::result::Result<Self, BookmarkError> {
        if !path.exists() {
            // Create parent directories if they don't exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            // Create empty store
            let store = Self::default();
            store.save(path).await?;
            return Ok(store);
        }

        let content = fs::read_to_string(path).await?;
        let store: Self = serde_json::from_str(&content)?;
        Ok(store)
    }

    /// Save bookmark store to a file.
    pub async fn save(&self, path: &Path) -> std::result::Result<(), BookmarkError> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Get all unique folder paths.
    pub fn get_folders(&self) -> Vec<String> {
        let mut folders: HashSet<String> = HashSet::new();
        for bookmark in &self.bookmarks {
            if let Some(folder) = &bookmark.folder {
                // Add the folder and all parent folders
                let parts: Vec<&str> = folder.split('/').collect();
                for i in 1..=parts.len() {
                    let path = parts[..i].join("/");
                    folders.insert(path);
                }
            }
        }
        let mut folder_vec: Vec<String> = folders.into_iter().collect();
        folder_vec.sort();
        folder_vec
    }

    /// Get bookmarks in a specific folder (not including subfolders).
    pub fn get_bookmarks_in_folder(&self, folder: &str) -> Vec<&Bookmark> {
        self.bookmarks
            .iter()
            .filter(|b| b.folder.as_deref() == Some(folder))
            .collect()
    }

    /// Get all bookmarks (including those in folders).
    pub fn get_all_bookmarks(&self) -> Vec<&Bookmark> {
        self.bookmarks.iter().collect()
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the bookmarks provider.
#[derive(Debug, Clone)]
pub struct BookmarksConfig {
    /// Path to the bookmarks JSON file.
    pub bookmarks_path: PathBuf,
}

impl BookmarksConfig {
    /// Create a new configuration with the given bookmarks file path.
    pub fn new(bookmarks_path: PathBuf) -> Self {
        Self { bookmarks_path }
    }

    /// Create a default configuration using XDG_DATA_HOME.
    pub fn default_path() -> PathBuf {
        let data_home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME environment variable not set");
            format!("{}/.local/share", home)
        });
        PathBuf::from(data_home)
            .join("scryforge")
            .join("bookmarks.json")
    }
}

impl Default for BookmarksConfig {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Bookmarks provider for local bookmark management.
pub struct BookmarksProvider {
    config: BookmarksConfig,
    store: tokio::sync::RwLock<BookmarkStore>,
}

impl BookmarksProvider {
    /// Create a new bookmarks provider with the given configuration.
    pub async fn new(config: BookmarksConfig) -> std::result::Result<Self, BookmarkError> {
        let store = BookmarkStore::load(&config.bookmarks_path).await?;
        tracing::info!(
            "Loaded {} bookmarks from {}",
            store.bookmarks.len(),
            config.bookmarks_path.display()
        );

        Ok(Self {
            config,
            store: tokio::sync::RwLock::new(store),
        })
    }

    /// Create a new bookmarks provider with default configuration.
    pub async fn with_default_config() -> std::result::Result<Self, BookmarkError> {
        Self::new(BookmarksConfig::default()).await
    }

    /// Reload bookmarks from disk.
    async fn reload(&self) -> std::result::Result<(), BookmarkError> {
        let store = BookmarkStore::load(&self.config.bookmarks_path).await?;
        *self.store.write().await = store;
        Ok(())
    }

    /// Save bookmarks to disk.
    async fn save(&self) -> std::result::Result<(), BookmarkError> {
        let store = self.store.read().await;
        store.save(&self.config.bookmarks_path).await
    }
}

#[async_trait]
impl Provider for BookmarksProvider {
    fn id(&self) -> &'static str {
        "bookmarks"
    }

    fn name(&self) -> &'static str {
        "Local Bookmarks"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Check if we can read the bookmarks file
        match self.reload().await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("Bookmarks file accessible".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Failed to read bookmarks: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        match self.reload().await {
            Ok(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(SyncResult {
                    success: true,
                    items_added: 0,
                    items_updated: 0,
                    items_removed: 0,
                    errors: vec![],
                    duration_ms,
                })
            }
            Err(e) => Ok(SyncResult {
                success: false,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![e.to_string()],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: false,
            has_collections: true,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "open".to_string(),
                name: "Open in Browser".to_string(),
                description: "Open bookmark URL in browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            },
        ];

        // Add preview if there's a description
        if let ItemContent::Bookmark {
            description: Some(_),
        } = &item.content
        {
            actions.insert(
                0,
                Action {
                    id: "preview".to_string(),
                    name: "Preview".to_string(),
                    description: "Show bookmark details".to_string(),
                    kind: ActionKind::Preview,
                    keyboard_shortcut: Some("p".to_string()),
                },
            );
        }

        Ok(actions)
    }

    async fn execute_action(&self, _item: &Item, action: &Action) -> Result<ActionResult> {
        // For now, just acknowledge the action
        // Actual implementation would require integration with system clipboard, browser, etc.
        Ok(ActionResult {
            success: true,
            message: Some(format!("Action '{}' acknowledged", action.name)),
            data: None,
        })
    }
}

#[async_trait]
impl HasCollections for BookmarksProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let store = self.store.read().await;
        let folders = store.get_folders();

        let collections = folders
            .into_iter()
            .map(|folder| {
                let bookmarks_in_folder = store.get_bookmarks_in_folder(&folder);
                let item_count = bookmarks_in_folder.len() as u32;

                // Determine icon based on folder depth/name
                let icon = if folder.contains('/') {
                    Some("📂".to_string())
                } else {
                    Some("📁".to_string())
                };

                Collection {
                    id: CollectionId(folder.clone()),
                    name: folder
                        .split('/')
                        .last()
                        .unwrap_or(&folder)
                        .to_string(),
                    description: Some(format!("Bookmark folder: {}", folder)),
                    icon,
                    item_count,
                    is_editable: true,
                    owner: None,
                }
            })
            .collect();

        Ok(collections)
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let store = self.store.read().await;
        let folder = &collection_id.0;

        let bookmarks = store.get_bookmarks_in_folder(folder);
        let items = bookmarks
            .into_iter()
            .map(|b| b.to_item(self.id()))
            .collect();

        Ok(items)
    }
}

#[async_trait]
impl HasSavedItems for BookmarksProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let store = self.store.read().await;
        let mut bookmarks = store.get_all_bookmarks();

        // Sort by created_at descending (newest first)
        bookmarks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply category filter (folder)
        if let Some(category) = &options.category {
            bookmarks.retain(|b| b.folder.as_deref() == Some(category));
        }

        // Apply pagination
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        let bookmarks = bookmarks.into_iter().skip(offset);
        let bookmarks: Vec<_> = if let Some(limit) = limit {
            bookmarks.take(limit).collect()
        } else {
            bookmarks.collect()
        };

        let items = bookmarks
            .into_iter()
            .map(|b| b.to_item(self.id()))
            .collect();

        Ok(items)
    }

    async fn is_saved(&self, item_id: &ItemId) -> Result<bool> {
        let store = self.store.read().await;
        let bookmark_id = item_id.as_str().strip_prefix("bookmarks:").unwrap_or("");
        Ok(store.bookmarks.iter().any(|b| b.id == bookmark_id))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_store() -> BookmarkStore {
        let mut store = BookmarkStore::default();

        let mut bookmark1 = Bookmark::new(
            "https://www.rust-lang.org".to_string(),
            "The Rust Programming Language".to_string(),
        );
        bookmark1.description = Some("Official Rust website".to_string());
        bookmark1.tags = vec!["rust".to_string(), "programming".to_string()];
        bookmark1.folder = Some("Technical/Rust".to_string());

        let mut bookmark2 = Bookmark::new(
            "https://docs.rs".to_string(),
            "Docs.rs".to_string(),
        );
        bookmark2.tags = vec!["rust".to_string(), "documentation".to_string()];
        bookmark2.folder = Some("Technical/Rust".to_string());

        let mut bookmark3 = Bookmark::new(
            "https://news.ycombinator.com".to_string(),
            "Hacker News".to_string(),
        );
        bookmark3.tags = vec!["news".to_string(), "tech".to_string()];
        bookmark3.folder = Some("News".to_string());

        let bookmark4 = Bookmark::new(
            "https://github.com".to_string(),
            "GitHub".to_string(),
        );
        // No folder - top-level bookmark

        store.bookmarks = vec![bookmark1, bookmark2, bookmark3, bookmark4];
        store
    }

    async fn create_test_provider() -> (BookmarksProvider, NamedTempFile) {
        let mut temp_file = NamedTempFile::new().unwrap();
        let store = create_test_store();
        let json = serde_json::to_string_pretty(&store).unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = BookmarksConfig::new(temp_file.path().to_path_buf());
        let provider = BookmarksProvider::new(config).await.unwrap();

        (provider, temp_file)
    }

    #[test]
    fn test_bookmark_creation() {
        let bookmark = Bookmark::new(
            "https://example.com".to_string(),
            "Example".to_string(),
        );

        assert!(!bookmark.id.is_empty());
        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(bookmark.title, "Example");
        assert!(bookmark.description.is_none());
        assert!(bookmark.tags.is_empty());
        assert!(bookmark.folder.is_none());
    }

    #[test]
    fn test_bookmark_to_item() {
        let mut bookmark = Bookmark::new(
            "https://example.com".to_string(),
            "Example".to_string(),
        );
        bookmark.description = Some("Test description".to_string());
        bookmark.tags = vec!["tag1".to_string()];
        bookmark.folder = Some("Test/Folder".to_string());

        let item = bookmark.to_item("bookmarks");

        assert_eq!(item.title, "Example");
        assert_eq!(item.url, Some("https://example.com".to_string()));
        assert!(item.is_saved);
        assert_eq!(item.tags, vec!["tag1".to_string()]);
        assert!(matches!(item.content, ItemContent::Bookmark { .. }));
    }

    #[tokio::test]
    async fn test_bookmark_store_load_save() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut store = BookmarkStore::default();
        store.bookmarks.push(Bookmark::new(
            "https://example.com".to_string(),
            "Example".to_string(),
        ));

        store.save(path).await.unwrap();

        let loaded = BookmarkStore::load(path).await.unwrap();
        assert_eq!(loaded.bookmarks.len(), 1);
        assert_eq!(loaded.bookmarks[0].url, "https://example.com");
    }

    #[test]
    fn test_bookmark_store_get_folders() {
        let store = create_test_store();
        let folders = store.get_folders();

        assert_eq!(folders.len(), 3);
        assert!(folders.contains(&"Technical".to_string()));
        assert!(folders.contains(&"Technical/Rust".to_string()));
        assert!(folders.contains(&"News".to_string()));
    }

    #[test]
    fn test_bookmark_store_get_bookmarks_in_folder() {
        let store = create_test_store();
        let bookmarks = store.get_bookmarks_in_folder("Technical/Rust");

        assert_eq!(bookmarks.len(), 2);
        assert!(bookmarks.iter().any(|b| b.url == "https://www.rust-lang.org"));
        assert!(bookmarks.iter().any(|b| b.url == "https://docs.rs"));
    }

    #[tokio::test]
    async fn test_provider_basics() {
        let (provider, _temp_file) = create_test_provider().await;

        assert_eq!(provider.id(), "bookmarks");
        assert_eq!(provider.name(), "Local Bookmarks");

        let caps = provider.capabilities();
        assert!(!caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_provider_health_check() {
        let (provider, _temp_file) = create_test_provider().await;
        let health = provider.health_check().await.unwrap();

        assert!(health.is_healthy);
        assert_eq!(health.error_count, 0);
    }

    #[tokio::test]
    async fn test_provider_sync() {
        let (provider, _temp_file) = create_test_provider().await;
        let result = provider.sync().await.unwrap();

        assert!(result.success);
        assert_eq!(result.errors.len(), 0);
    }

    #[tokio::test]
    async fn test_list_collections() {
        let (provider, _temp_file) = create_test_provider().await;
        let collections = provider.list_collections().await.unwrap();

        assert_eq!(collections.len(), 3);

        let rust_folder = collections
            .iter()
            .find(|c| c.id.0 == "Technical/Rust")
            .unwrap();
        assert_eq!(rust_folder.name, "Rust");
        assert_eq!(rust_folder.item_count, 2);
        assert!(rust_folder.is_editable);
    }

    #[tokio::test]
    async fn test_get_collection_items() {
        let (provider, _temp_file) = create_test_provider().await;
        let collection_id = CollectionId("Technical/Rust".to_string());
        let items = provider.get_collection_items(&collection_id).await.unwrap();

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "The Rust Programming Language"));
        assert!(items.iter().any(|i| i.title == "Docs.rs"));

        for item in &items {
            assert!(item.is_saved);
            assert!(matches!(item.content, ItemContent::Bookmark { .. }));
        }
    }

    #[tokio::test]
    async fn test_get_saved_items() {
        let (provider, _temp_file) = create_test_provider().await;
        let options = SavedItemsOptions::default();
        let items = provider.get_saved_items(options).await.unwrap();

        assert_eq!(items.len(), 4);
        for item in &items {
            assert!(item.is_saved);
        }
    }

    #[tokio::test]
    async fn test_get_saved_items_with_pagination() {
        let (provider, _temp_file) = create_test_provider().await;
        let options = SavedItemsOptions {
            limit: Some(2),
            offset: Some(1),
            category: None,
        };
        let items = provider.get_saved_items(options).await.unwrap();

        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_get_saved_items_with_category() {
        let (provider, _temp_file) = create_test_provider().await;
        let options = SavedItemsOptions {
            limit: None,
            offset: None,
            category: Some("Technical/Rust".to_string()),
        };
        let items = provider.get_saved_items(options).await.unwrap();

        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.tags.contains(&"rust".to_string())));
    }

    #[tokio::test]
    async fn test_is_saved() {
        let (provider, _temp_file) = create_test_provider().await;

        let bookmark_id = {
            let store = provider.store.read().await;
            store.bookmarks[0].id.clone()
        };

        let item_id = ItemId::new("bookmarks", &bookmark_id);
        let is_saved = provider.is_saved(&item_id).await.unwrap();
        assert!(is_saved);

        let fake_id = ItemId::new("bookmarks", "nonexistent");
        let is_saved = provider.is_saved(&fake_id).await.unwrap();
        assert!(!is_saved);
    }

    #[tokio::test]
    async fn test_available_actions() {
        let (provider, _temp_file) = create_test_provider().await;
        let bookmark = Bookmark::new("https://example.com".to_string(), "Test".to_string());
        let item = bookmark.to_item("bookmarks");

        let actions = provider.available_actions(&item).await.unwrap();
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_available_actions_with_description() {
        let (provider, _temp_file) = create_test_provider().await;
        let mut bookmark = Bookmark::new("https://example.com".to_string(), "Test".to_string());
        bookmark.description = Some("Test description".to_string());
        let item = bookmark.to_item("bookmarks");

        let actions = provider.available_actions(&item).await.unwrap();
        assert!(actions.iter().any(|a| a.kind == ActionKind::Preview));
    }

    #[tokio::test]
    async fn test_default_config_path() {
        let path = BookmarksConfig::default_path();
        assert!(path.to_string_lossy().contains("scryforge"));
        assert!(path.to_string_lossy().contains("bookmarks.json"));
    }

    #[tokio::test]
    async fn test_create_default_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("bookmarks.json");

        let store = BookmarkStore::load(&path).await.unwrap();
        assert_eq!(store.bookmarks.len(), 0);
        assert!(path.exists());
    }
}

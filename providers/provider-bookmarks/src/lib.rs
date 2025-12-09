//! # provider-bookmarks
//!
//! Local bookmarks provider for Scryforge.
//!
//! This provider manages bookmarks stored locally in JSON format and supports
//! importing bookmarks from Chrome and Firefox browser exports.
//!
//! ## Features
//!
//! - Store bookmarks locally in XDG_DATA_HOME/scryforge/bookmarks.json
//! - Organize bookmarks into folders (collections)
//! - Import bookmarks from Chrome/Firefox JSON exports
//! - Full-text search across bookmark titles and descriptions
//! - Automatic URL deduplication
//!
//! ## Storage Format
//!
//! Bookmarks are stored in a simple JSON format:
//! ```json
//! {
//!   "folders": [
//!     {
//!       "id": "work",
//!       "name": "Work",
//!       "description": "Work-related bookmarks"
//!     }
//!   ],
//!   "bookmarks": [
//!     {
//!       "id": "uuid-here",
//!       "folder_id": "work",
//!       "title": "Example",
//!       "url": "https://example.com",
//!       "description": "Optional description",
//!       "created_at": "2025-01-01T00:00:00Z",
//!       "tags": ["example"]
//!     }
//!   ]
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum BookmarkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Bookmark not found: {0}")]
    NotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("Invalid bookmark data: {0}")]
    InvalidData(String),
}

impl From<BookmarkError> for StreamError {
    fn from(err: BookmarkError) -> Self {
        match err {
            BookmarkError::NotFound(msg) => StreamError::ItemNotFound(msg),
            BookmarkError::FolderNotFound(msg) => StreamError::StreamNotFound(msg),
            BookmarkError::InvalidData(msg) => StreamError::Provider(msg),
            BookmarkError::Io(e) => StreamError::Internal(format!("IO error: {}", e)),
            BookmarkError::Json(e) => StreamError::Internal(format!("JSON error: {}", e)),
        }
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// A bookmark folder for organizing bookmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkFolder {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

/// A bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub favicon_url: Option<String>,
}

/// The root storage structure for bookmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookmarkStorage {
    folders: Vec<BookmarkFolder>,
    bookmarks: Vec<Bookmark>,
    #[serde(default)]
    version: u32,
}

impl Default for BookmarkStorage {
    fn default() -> Self {
        Self {
            folders: vec![BookmarkFolder {
                id: "default".to_string(),
                name: "Unsorted Bookmarks".to_string(),
                description: Some("Default folder for unsorted bookmarks".to_string()),
                icon: Some("📑".to_string()),
            }],
            bookmarks: vec![],
            version: 1,
        }
    }
}

// ============================================================================
// Chrome/Firefox Import Structures
// ============================================================================

/// Chrome bookmark export format.
#[derive(Debug, Deserialize)]
struct ChromeBookmarkRoot {
    roots: ChromeBookmarkRoots,
}

#[derive(Debug, Deserialize)]
struct ChromeBookmarkRoots {
    bookmark_bar: ChromeBookmarkNode,
    other: ChromeBookmarkNode,
}

#[derive(Debug, Deserialize)]
struct ChromeBookmarkNode {
    #[serde(default)]
    children: Vec<ChromeBookmarkNode>,
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    url: Option<String>,
}

/// Firefox bookmark export format (simplified).
#[derive(Debug, Deserialize)]
struct FirefoxBookmark {
    #[serde(default)]
    title: String,
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    children: Vec<FirefoxBookmark>,
    #[serde(rename = "type")]
    #[serde(default)]
    bookmark_type: String,
}

// ============================================================================
// BookmarksProvider Implementation
// ============================================================================

/// Provider for managing local bookmarks.
pub struct BookmarksProvider {
    storage_path: PathBuf,
    storage: Arc<RwLock<BookmarkStorage>>,
}

impl BookmarksProvider {
    /// Create a new bookmarks provider with default XDG data directory.
    pub fn new() -> Result<Self> {
        let storage_path = Self::default_storage_path()?;
        Self::with_path(storage_path)
    }

    /// Create a new bookmarks provider with a custom storage path.
    pub fn with_path(storage_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent).map_err(BookmarkError::Io)?;
        }

        // Load or create storage
        let storage = if storage_path.exists() {
            Self::load_storage(&storage_path)?
        } else {
            let default_storage = BookmarkStorage::default();
            Self::save_storage(&storage_path, &default_storage)?;
            default_storage
        };

        Ok(Self {
            storage_path,
            storage: Arc::new(RwLock::new(storage)),
        })
    }

    /// Get the default storage path in XDG_DATA_HOME.
    fn default_storage_path() -> Result<PathBuf> {
        let dirs =
            directories::ProjectDirs::from("com", "raibid-labs", "scryforge").ok_or_else(|| {
                StreamError::Internal("Could not determine data directory".to_string())
            })?;

        let data_dir = dirs.data_dir();
        std::fs::create_dir_all(data_dir).map_err(|e| {
            StreamError::Internal(format!("Failed to create data directory: {}", e))
        })?;

        Ok(data_dir.join("bookmarks.json"))
    }

    /// Load bookmark storage from disk.
    fn load_storage(path: &Path) -> Result<BookmarkStorage> {
        let contents = std::fs::read_to_string(path).map_err(BookmarkError::Io)?;
        let storage: BookmarkStorage =
            serde_json::from_str(&contents).map_err(BookmarkError::Json)?;
        Ok(storage)
    }

    /// Save bookmark storage to disk.
    fn save_storage(path: &Path, storage: &BookmarkStorage) -> Result<()> {
        let contents = serde_json::to_string_pretty(storage).map_err(BookmarkError::Json)?;
        std::fs::write(path, contents).map_err(BookmarkError::Io)?;
        Ok(())
    }

    /// Persist the current storage state to disk.
    fn persist(&self) -> Result<()> {
        let storage = self.storage.read().unwrap();
        Self::save_storage(&self.storage_path, &storage)
    }

    /// Add a new bookmark.
    pub fn add_bookmark(
        &self,
        title: String,
        url: String,
        folder_id: Option<String>,
        description: Option<String>,
        tags: Vec<String>,
    ) -> Result<Bookmark> {
        let mut storage = self.storage.write().unwrap();

        // Validate folder if specified
        if let Some(ref folder_id) = folder_id {
            if !storage.folders.iter().any(|f| &f.id == folder_id) {
                return Err(BookmarkError::FolderNotFound(folder_id.clone()).into());
            }
        }

        let bookmark = Bookmark {
            id: Uuid::new_v4().to_string(),
            folder_id,
            title,
            url,
            description,
            created_at: Utc::now(),
            updated_at: None,
            tags,
            favicon_url: None,
        };

        storage.bookmarks.push(bookmark.clone());
        drop(storage);
        self.persist()?;

        Ok(bookmark)
    }

    /// Add a new folder.
    pub fn add_folder(
        &self,
        name: String,
        description: Option<String>,
        icon: Option<String>,
    ) -> Result<BookmarkFolder> {
        let mut storage = self.storage.write().unwrap();

        let folder = BookmarkFolder {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            icon,
        };

        storage.folders.push(folder.clone());
        drop(storage);
        self.persist()?;

        Ok(folder)
    }

    /// Import bookmarks from a Chrome JSON export.
    pub fn import_from_chrome(&self, json_path: &Path) -> Result<(usize, Vec<String>)> {
        let contents = std::fs::read_to_string(json_path).map_err(BookmarkError::Io)?;
        let chrome_data: ChromeBookmarkRoot =
            serde_json::from_str(&contents).map_err(BookmarkError::Json)?;

        let mut imported_count = 0;
        let mut errors = Vec::new();

        // Import from bookmark bar
        if let Err(e) =
            self.import_chrome_node(&chrome_data.roots.bookmark_bar, None, &mut imported_count)
        {
            errors.push(format!("Bookmark bar import error: {}", e));
        }

        // Import from other bookmarks
        if let Err(e) = self.import_chrome_node(&chrome_data.roots.other, None, &mut imported_count)
        {
            errors.push(format!("Other bookmarks import error: {}", e));
        }

        Ok((imported_count, errors))
    }

    /// Recursively import a Chrome bookmark node.
    fn import_chrome_node(
        &self,
        node: &ChromeBookmarkNode,
        parent_folder: Option<String>,
        count: &mut usize,
    ) -> Result<()> {
        match node.node_type.as_str() {
            "folder" => {
                // Create folder
                let folder = self.add_folder(node.name.clone(), None, Some("📁".to_string()))?;

                // Import children
                for child in &node.children {
                    self.import_chrome_node(child, Some(folder.id.clone()), count)?;
                }
            }
            "url" => {
                if let Some(ref url) = node.url {
                    // Check for duplicates
                    let storage = self.storage.read().unwrap();
                    let exists = storage.bookmarks.iter().any(|b| &b.url == url);
                    drop(storage);

                    if !exists {
                        self.add_bookmark(
                            node.name.clone(),
                            url.clone(),
                            parent_folder,
                            None,
                            vec![],
                        )?;
                        *count += 1;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Import bookmarks from a Firefox JSON export.
    pub fn import_from_firefox(&self, json_path: &Path) -> Result<(usize, Vec<String>)> {
        let contents = std::fs::read_to_string(json_path).map_err(BookmarkError::Io)?;
        let firefox_data: FirefoxBookmark =
            serde_json::from_str(&contents).map_err(BookmarkError::Json)?;

        let mut imported_count = 0;
        let mut errors = Vec::new();

        if let Err(e) = self.import_firefox_node(&firefox_data, None, &mut imported_count) {
            errors.push(format!("Firefox import error: {}", e));
        }

        Ok((imported_count, errors))
    }

    /// Recursively import a Firefox bookmark node.
    fn import_firefox_node(
        &self,
        node: &FirefoxBookmark,
        parent_folder: Option<String>,
        count: &mut usize,
    ) -> Result<()> {
        match node.bookmark_type.as_str() {
            "text/x-moz-place-container" => {
                // Create folder
                let folder = self.add_folder(node.title.clone(), None, Some("📁".to_string()))?;

                // Import children
                for child in &node.children {
                    self.import_firefox_node(child, Some(folder.id.clone()), count)?;
                }
            }
            "text/x-moz-place" => {
                if let Some(ref url) = node.uri {
                    // Check for duplicates
                    let storage = self.storage.read().unwrap();
                    let exists = storage.bookmarks.iter().any(|b| &b.url == url);
                    drop(storage);

                    if !exists {
                        self.add_bookmark(
                            node.title.clone(),
                            url.clone(),
                            parent_folder,
                            None,
                            vec![],
                        )?;
                        *count += 1;
                    }
                }
            }
            _ => {
                // Try to import children anyway
                for child in &node.children {
                    self.import_firefox_node(child, parent_folder.clone(), count)?;
                }
            }
        }

        Ok(())
    }

    /// Convert a bookmark to an Item.
    fn bookmark_to_item(&self, bookmark: &Bookmark) -> Item {
        let folder_name = if let Some(ref folder_id) = bookmark.folder_id {
            let storage = self.storage.read().unwrap();
            storage
                .folders
                .iter()
                .find(|f| &f.id == folder_id)
                .map(|f| f.name.clone())
        } else {
            None
        };

        let stream_id = if let Some(ref folder_id) = bookmark.folder_id {
            StreamId::new("bookmarks", "collection", folder_id)
        } else {
            StreamId::new("bookmarks", "saved", "all")
        };

        let mut metadata = HashMap::new();
        if let Some(folder) = folder_name {
            metadata.insert("folder".to_string(), folder);
        }

        Item {
            id: ItemId::new("bookmarks", &bookmark.id),
            stream_id,
            title: bookmark.title.clone(),
            content: ItemContent::Bookmark {
                description: bookmark.description.clone(),
            },
            author: None,
            published: Some(bookmark.created_at),
            updated: bookmark.updated_at,
            url: Some(bookmark.url.clone()),
            thumbnail_url: bookmark.favicon_url.clone(),
            is_read: true,  // Bookmarks are always "read"
            is_saved: true, // All bookmarks are saved by definition
            tags: bookmark.tags.clone(),
            metadata,
        }
    }
}

impl Default for BookmarksProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create default BookmarksProvider")
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
        let storage = self.storage.read().unwrap();
        let bookmark_count = storage.bookmarks.len();
        let folder_count = storage.folders.len();

        Ok(ProviderHealth {
            is_healthy: true,
            message: Some(format!(
                "Healthy: {} bookmarks in {} folders",
                bookmark_count, folder_count
            )),
            last_sync: Some(Utc::now()),
            error_count: 0,
        })
    }

    async fn sync(&self) -> Result<SyncResult> {
        // Reload from disk to pick up any external changes
        let start = std::time::Instant::now();

        let new_storage = Self::load_storage(&self.storage_path)?;
        let mut storage = self.storage.write().unwrap();
        *storage = new_storage;

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

        // Add delete action for bookmarks
        if item.url.is_some() {
            actions.push(Action {
                id: "delete".to_string(),
                name: "Delete Bookmark".to_string(),
                description: "Remove this bookmark".to_string(),
                kind: ActionKind::Delete,
                keyboard_shortcut: Some("d".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser | ActionKind::Open => {
                if let Some(ref url) = item.url {
                    // In a real implementation, you'd open the URL
                    Ok(ActionResult {
                        success: true,
                        message: Some(format!("Would open: {}", url)),
                        data: None,
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available".to_string()),
                        data: None,
                    })
                }
            }
            ActionKind::CopyLink => {
                if let Some(ref url) = item.url {
                    Ok(ActionResult {
                        success: true,
                        message: Some(format!("Copied: {}", url)),
                        data: Some(serde_json::json!({ "url": url })),
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available".to_string()),
                        data: None,
                    })
                }
            }
            ActionKind::Delete => {
                // Extract bookmark ID from item ID
                let bookmark_id = item
                    .id
                    .0
                    .strip_prefix("bookmarks:")
                    .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

                let mut storage = self.storage.write().unwrap();
                let original_len = storage.bookmarks.len();
                storage.bookmarks.retain(|b| b.id != bookmark_id);
                let removed = original_len - storage.bookmarks.len();
                drop(storage);

                if removed > 0 {
                    self.persist()?;
                    Ok(ActionResult {
                        success: true,
                        message: Some("Bookmark deleted".to_string()),
                        data: None,
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("Bookmark not found".to_string()),
                        data: None,
                    })
                }
            }
            _ => Ok(ActionResult {
                success: false,
                message: Some(format!("Action '{}' not supported", action.name)),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl HasCollections for BookmarksProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let storage = self.storage.read().unwrap();

        let collections = storage
            .folders
            .iter()
            .map(|folder| {
                let item_count = storage
                    .bookmarks
                    .iter()
                    .filter(|b| b.folder_id.as_ref() == Some(&folder.id))
                    .count() as u32;

                Collection {
                    id: CollectionId(folder.id.clone()),
                    name: folder.name.clone(),
                    description: folder.description.clone(),
                    icon: folder.icon.clone(),
                    item_count,
                    is_editable: true,
                    owner: None,
                }
            })
            .collect();

        Ok(collections)
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let storage = self.storage.read().unwrap();

        // Verify collection exists
        if !storage.folders.iter().any(|f| f.id == collection_id.0) {
            return Err(BookmarkError::FolderNotFound(collection_id.0.clone()).into());
        }

        let items: Vec<Item> = storage
            .bookmarks
            .iter()
            .filter(|b| b.folder_id.as_ref() == Some(&collection_id.0))
            .map(|b| self.bookmark_to_item(b))
            .collect();

        Ok(items)
    }

    async fn add_to_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        // Extract bookmark ID from item ID
        let bookmark_id = item_id
            .0
            .strip_prefix("bookmarks:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

        let mut storage = self.storage.write().unwrap();

        // Verify collection exists
        if !storage.folders.iter().any(|f| f.id == collection_id.0) {
            return Err(BookmarkError::FolderNotFound(collection_id.0.clone()).into());
        }

        // Find and update the bookmark
        let bookmark = storage
            .bookmarks
            .iter_mut()
            .find(|b| b.id == bookmark_id)
            .ok_or_else(|| BookmarkError::NotFound(bookmark_id.to_string()))?;

        bookmark.folder_id = Some(collection_id.0.clone());
        bookmark.updated_at = Some(Utc::now());

        drop(storage);
        self.persist()?;

        Ok(())
    }

    async fn remove_from_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        // Extract bookmark ID from item ID
        let bookmark_id = item_id
            .0
            .strip_prefix("bookmarks:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

        let mut storage = self.storage.write().unwrap();

        // Find the bookmark and verify it's in the specified collection
        let bookmark = storage
            .bookmarks
            .iter_mut()
            .find(|b| b.id == bookmark_id)
            .ok_or_else(|| BookmarkError::NotFound(bookmark_id.to_string()))?;

        if bookmark.folder_id.as_ref() != Some(&collection_id.0) {
            return Err(StreamError::Provider(
                "Bookmark is not in the specified collection".to_string(),
            ));
        }

        // Remove from collection (move to default)
        bookmark.folder_id = None;
        bookmark.updated_at = Some(Utc::now());

        drop(storage);
        self.persist()?;

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> Result<Collection> {
        let folder = self.add_folder(name.to_string(), None, Some("📁".to_string()))?;

        Ok(Collection {
            id: CollectionId(folder.id.clone()),
            name: folder.name.clone(),
            description: folder.description.clone(),
            icon: folder.icon.clone(),
            item_count: 0,
            is_editable: true,
            owner: None,
        })
    }
}

#[async_trait]
impl HasSavedItems for BookmarksProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let storage = self.storage.read().unwrap();

        let mut items: Vec<Item> = storage
            .bookmarks
            .iter()
            .map(|b| self.bookmark_to_item(b))
            .collect();

        // Apply category filter (folder name)
        if let Some(ref category) = options.category {
            let storage = self.storage.read().unwrap();
            let folder_id = storage
                .folders
                .iter()
                .find(|f| &f.name == category)
                .map(|f| f.id.clone());
            drop(storage);

            if let Some(folder_id) = folder_id {
                items.retain(|item| {
                    item.metadata
                        .get("folder")
                        .map(|f| f == category)
                        .unwrap_or(false)
                        || item.stream_id.as_str().contains(&folder_id)
                });
            } else {
                items.clear(); // No matching folder
            }
        }

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let items = items.into_iter().skip(offset);

        let items = if let Some(limit) = options.limit {
            items.take(limit as usize).collect()
        } else {
            items.collect()
        };

        Ok(items)
    }

    async fn is_saved(&self, item_id: &ItemId) -> Result<bool> {
        let bookmark_id = item_id
            .0
            .strip_prefix("bookmarks:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

        let storage = self.storage.read().unwrap();
        Ok(storage.bookmarks.iter().any(|b| b.id == bookmark_id))
    }

    async fn save_item(&self, item_id: &ItemId) -> Result<()> {
        // For the bookmarks provider, this is a no-op since items are already bookmarks
        // However, we can check if the item exists
        let bookmark_id = item_id
            .0
            .strip_prefix("bookmarks:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

        let storage = self.storage.read().unwrap();
        if !storage.bookmarks.iter().any(|b| b.id == bookmark_id) {
            return Err(StreamError::ItemNotFound(bookmark_id.to_string()));
        }

        Ok(())
    }

    async fn unsave_item(&self, item_id: &ItemId) -> Result<()> {
        // For the bookmarks provider, unsaving means deleting the bookmark
        let bookmark_id = item_id
            .0
            .strip_prefix("bookmarks:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID".to_string()))?;

        let mut storage = self.storage.write().unwrap();
        let original_len = storage.bookmarks.len();
        storage.bookmarks.retain(|b| b.id != bookmark_id);
        let removed = original_len - storage.bookmarks.len();
        drop(storage);

        if removed > 0 {
            self.persist()?;
            Ok(())
        } else {
            Err(StreamError::ItemNotFound(bookmark_id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_provider() -> (BookmarksProvider, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage_path = temp_dir.path().join("bookmarks.json");
        let provider = BookmarksProvider::with_path(storage_path).unwrap();
        (provider, temp_dir)
    }

    #[tokio::test]
    async fn test_provider_basics() {
        let (provider, _temp_dir) = create_temp_provider();

        assert_eq!(provider.id(), "bookmarks");
        assert_eq!(provider.name(), "Local Bookmarks");

        let caps = provider.capabilities();
        assert!(!caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_health_check() {
        let (provider, _temp_dir) = create_temp_provider();
        let health = provider.health_check().await.unwrap();

        assert!(health.is_healthy);
        assert_eq!(health.error_count, 0);
    }

    #[tokio::test]
    async fn test_add_bookmark() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "Example".to_string(),
                "https://example.com".to_string(),
                None,
                Some("An example bookmark".to_string()),
                vec!["test".to_string()],
            )
            .unwrap();

        assert_eq!(bookmark.title, "Example");
        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(
            bookmark.description,
            Some("An example bookmark".to_string())
        );
        assert_eq!(bookmark.tags, vec!["test"]);
    }

    #[tokio::test]
    async fn test_add_folder_and_bookmark() {
        let (provider, _temp_dir) = create_temp_provider();

        let folder = provider
            .add_folder(
                "Work".to_string(),
                Some("Work bookmarks".to_string()),
                Some("💼".to_string()),
            )
            .unwrap();

        let bookmark = provider
            .add_bookmark(
                "Work Site".to_string(),
                "https://work.example.com".to_string(),
                Some(folder.id.clone()),
                None,
                vec![],
            )
            .unwrap();

        assert_eq!(bookmark.folder_id, Some(folder.id));
    }

    #[tokio::test]
    async fn test_list_collections() {
        let (provider, _temp_dir) = create_temp_provider();

        let folder = provider
            .add_folder(
                "Personal".to_string(),
                Some("Personal bookmarks".to_string()),
                Some("🏠".to_string()),
            )
            .unwrap();

        provider
            .add_bookmark(
                "Personal Site".to_string(),
                "https://personal.example.com".to_string(),
                Some(folder.id.clone()),
                None,
                vec![],
            )
            .unwrap();

        let collections = provider.list_collections().await.unwrap();

        // Should have default folder + our new folder
        assert!(collections.len() >= 2);

        let personal_collection = collections.iter().find(|c| c.name == "Personal").unwrap();

        assert_eq!(personal_collection.item_count, 1);
        assert_eq!(
            personal_collection.description,
            Some("Personal bookmarks".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_collection_items() {
        let (provider, _temp_dir) = create_temp_provider();

        let folder = provider.add_folder("Tech".to_string(), None, None).unwrap();

        provider
            .add_bookmark(
                "Rust".to_string(),
                "https://rust-lang.org".to_string(),
                Some(folder.id.clone()),
                None,
                vec![],
            )
            .unwrap();

        provider
            .add_bookmark(
                "GitHub".to_string(),
                "https://github.com".to_string(),
                Some(folder.id.clone()),
                None,
                vec![],
            )
            .unwrap();

        let items = provider
            .get_collection_items(&CollectionId(folder.id))
            .await
            .unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "Rust"));
        assert!(items.iter().any(|i| i.title == "GitHub"));
    }

    #[tokio::test]
    async fn test_get_saved_items() {
        let (provider, _temp_dir) = create_temp_provider();

        provider
            .add_bookmark(
                "Site 1".to_string(),
                "https://site1.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        provider
            .add_bookmark(
                "Site 2".to_string(),
                "https://site2.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let items = provider
            .get_saved_items(SavedItemsOptions::default())
            .await
            .unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_get_saved_items_with_limit() {
        let (provider, _temp_dir) = create_temp_provider();

        for i in 0..5 {
            provider
                .add_bookmark(
                    format!("Site {}", i),
                    format!("https://site{}.example.com", i),
                    None,
                    None,
                    vec![],
                )
                .unwrap();
        }

        let options = SavedItemsOptions {
            limit: Some(3),
            ..Default::default()
        };
        let items = provider.get_saved_items(options).await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn test_is_saved() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let item_id = ItemId::new("bookmarks", &bookmark.id);
        assert!(provider.is_saved(&item_id).await.unwrap());

        let non_existent = ItemId::new("bookmarks", "non-existent");
        assert!(!provider.is_saved(&non_existent).await.unwrap());
    }

    #[tokio::test]
    async fn test_import_chrome_bookmarks() {
        let (provider, _temp_dir) = create_temp_provider();

        // Create a temporary Chrome bookmarks file
        let chrome_json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks Bar",
                    "children": [
                        {
                            "type": "url",
                            "name": "Example",
                            "url": "https://example.com"
                        },
                        {
                            "type": "folder",
                            "name": "Tech",
                            "children": [
                                {
                                    "type": "url",
                                    "name": "Rust",
                                    "url": "https://rust-lang.org"
                                }
                            ]
                        }
                    ]
                },
                "other": {
                    "type": "folder",
                    "name": "Other Bookmarks",
                    "children": []
                }
            }
        }"#;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), chrome_json).unwrap();

        let (count, errors) = provider.import_from_chrome(temp_file.path()).unwrap();

        assert_eq!(count, 2); // 2 bookmarks imported
        assert!(errors.is_empty());

        let items = provider
            .get_saved_items(SavedItemsOptions::default())
            .await
            .unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "Example"));
        assert!(items.iter().any(|i| i.title == "Rust"));
    }

    #[tokio::test]
    async fn test_import_firefox_bookmarks() {
        let (provider, _temp_dir) = create_temp_provider();

        // Create a temporary Firefox bookmarks file
        let firefox_json = r#"{
            "type": "text/x-moz-place-container",
            "title": "root",
            "children": [
                {
                    "type": "text/x-moz-place",
                    "title": "Mozilla",
                    "uri": "https://mozilla.org"
                },
                {
                    "type": "text/x-moz-place-container",
                    "title": "Dev",
                    "children": [
                        {
                            "type": "text/x-moz-place",
                            "title": "MDN",
                            "uri": "https://developer.mozilla.org"
                        }
                    ]
                }
            ]
        }"#;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), firefox_json).unwrap();

        let (count, errors) = provider.import_from_firefox(temp_file.path()).unwrap();

        assert_eq!(count, 2);
        assert!(errors.is_empty());

        let items = provider
            .get_saved_items(SavedItemsOptions::default())
            .await
            .unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "Mozilla"));
        assert!(items.iter().any(|i| i.title == "MDN"));
    }

    #[tokio::test]
    async fn test_available_actions() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let item = provider.bookmark_to_item(&bookmark);
        let actions = provider.available_actions(&item).await.unwrap();

        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Delete));
    }

    #[tokio::test]
    async fn test_execute_delete_action() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "To Delete".to_string(),
                "https://delete.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let item = provider.bookmark_to_item(&bookmark);
        let delete_action = Action {
            id: "delete".to_string(),
            name: "Delete".to_string(),
            description: "Delete bookmark".to_string(),
            kind: ActionKind::Delete,
            keyboard_shortcut: Some("d".to_string()),
        };

        let result = provider
            .execute_action(&item, &delete_action)
            .await
            .unwrap();
        assert!(result.success);

        // Verify bookmark was deleted
        let items = provider
            .get_saved_items(SavedItemsOptions::default())
            .await
            .unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage_path = temp_dir.path().join("bookmarks.json");

        // Create provider and add a bookmark
        {
            let provider = BookmarksProvider::with_path(storage_path.clone()).unwrap();
            provider
                .add_bookmark(
                    "Persistent".to_string(),
                    "https://persistent.example.com".to_string(),
                    None,
                    None,
                    vec![],
                )
                .unwrap();
        }

        // Create new provider instance and verify bookmark exists
        {
            let provider = BookmarksProvider::with_path(storage_path).unwrap();
            let items = provider
                .get_saved_items(SavedItemsOptions::default())
                .await
                .unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].title, "Persistent");
        }
    }

    #[tokio::test]
    async fn test_sync() {
        let (provider, _temp_dir) = create_temp_provider();
        let result = provider.sync().await.unwrap();

        assert!(result.success);
        assert_eq!(result.errors.len(), 0);
    }

    #[tokio::test]
    async fn test_add_to_collection() {
        let (provider, _temp_dir) = create_temp_provider();

        let folder = provider.add_folder("Work".to_string(), None, None).unwrap();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let collection_id = CollectionId(folder.id.clone());
        let item_id = ItemId::new("bookmarks", &bookmark.id);

        provider
            .add_to_collection(&collection_id, &item_id)
            .await
            .unwrap();

        let items = provider.get_collection_items(&collection_id).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test");
    }

    #[tokio::test]
    async fn test_remove_from_collection() {
        let (provider, _temp_dir) = create_temp_provider();

        let folder = provider.add_folder("Work".to_string(), None, None).unwrap();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                Some(folder.id.clone()),
                None,
                vec![],
            )
            .unwrap();

        let collection_id = CollectionId(folder.id.clone());
        let item_id = ItemId::new("bookmarks", &bookmark.id);

        provider
            .remove_from_collection(&collection_id, &item_id)
            .await
            .unwrap();

        let items = provider.get_collection_items(&collection_id).await.unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn test_create_collection() {
        let (provider, _temp_dir) = create_temp_provider();

        let collection = provider.create_collection("New Folder").await.unwrap();

        assert_eq!(collection.name, "New Folder");
        assert_eq!(collection.item_count, 0);
        assert!(collection.is_editable);

        let collections = provider.list_collections().await.unwrap();
        assert!(collections.iter().any(|c| c.name == "New Folder"));
    }

    #[tokio::test]
    async fn test_save_item() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let item_id = ItemId::new("bookmarks", &bookmark.id);

        // Should succeed since item exists
        provider.save_item(&item_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_unsave_item() {
        let (provider, _temp_dir) = create_temp_provider();

        let bookmark = provider
            .add_bookmark(
                "Test".to_string(),
                "https://test.example.com".to_string(),
                None,
                None,
                vec![],
            )
            .unwrap();

        let item_id = ItemId::new("bookmarks", &bookmark.id);

        // Unsave should delete the bookmark
        provider.unsave_item(&item_id).await.unwrap();

        assert!(!provider.is_saved(&item_id).await.unwrap());

        let items = provider
            .get_saved_items(SavedItemsOptions::default())
            .await
            .unwrap();
        assert_eq!(items.len(), 0);
    }
}

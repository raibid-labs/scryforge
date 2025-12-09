//! SQLite-based caching layer for provider data
//!
//! This module provides a persistent cache for streams and items fetched from providers.
//! The cache is stored in a SQLite database at `$XDG_DATA_HOME/scryforge/cache.db`.
//!
//! # Database Schema
//!
//! - `streams`: Cached stream metadata
//! - `items`: Cached items from providers
//! - `sync_state`: Tracks last sync timestamps per provider
//! - `schema_version`: Migration tracking
//!
//! # Example
//!
//! ```no_run
//! use scryforge_daemon::cache::{SqliteCache, Cache};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let cache = SqliteCache::open()?;
//!
//!     // Use the cache...
//!     let streams = cache.get_streams(None)?;
//!
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use scryforge_provider_core::{Item, ItemId, Stream, StreamId};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

// ============================================================================
// Cache Trait
// ============================================================================

/// Trait defining cache operations for provider data.
pub trait Cache: Send + Sync {
    /// Get all streams, optionally filtered by provider ID.
    fn get_streams(&self, provider_id: Option<&str>) -> Result<Vec<Stream>>;

    /// Get items for a specific stream.
    fn get_items(&self, stream_id: &StreamId, limit: Option<u32>) -> Result<Vec<Item>>;

    /// Insert or update multiple streams in the cache.
    fn upsert_streams(&self, streams: &[Stream]) -> Result<()>;

    /// Insert or update multiple items in the cache.
    fn upsert_items(&self, items: &[Item]) -> Result<()>;

    /// Mark an item as read or unread.
    fn mark_read(&self, item_id: &ItemId, is_read: bool) -> Result<()>;

    /// Mark an item as starred (saved) or unstarred.
    fn mark_starred(&self, item_id: &ItemId, is_starred: bool) -> Result<()>;

    /// Get the last sync timestamp for a provider.
    fn get_sync_state(&self, provider_id: &str) -> Result<Option<DateTime<Utc>>>;

    /// Update the last sync timestamp for a provider.
    fn update_sync_state(&self, provider_id: &str, last_sync: DateTime<Utc>) -> Result<()>;

    /// Search for items matching a query and optional filters.
    ///
    /// # Arguments
    ///
    /// * `query` - The search text (searched in title, content)
    /// * `stream_id` - Optional stream ID to filter by
    /// * `content_type` - Optional content type to filter by
    /// * `is_read` - Optional read status filter
    /// * `is_saved` - Optional saved status filter
    fn search_items(
        &self,
        query: &str,
        stream_id: Option<&str>,
        content_type: Option<&str>,
        is_read: Option<bool>,
        is_saved: Option<bool>,
    ) -> Result<Vec<Item>>;
}

// ============================================================================
// SqliteCache Implementation
// ============================================================================

/// SQLite-based implementation of the cache.
///
/// The connection is wrapped in a `Mutex` to allow interior mutability
/// and to satisfy the `Sync` trait requirement.
pub struct SqliteCache {
    conn: Mutex<Connection>,
}

impl SqliteCache {
    /// Open the cache database, creating it if it doesn't exist.
    ///
    /// The database is stored at `$XDG_DATA_HOME/scryforge/cache.db`.
    /// The directory structure is created if it doesn't exist.
    pub fn open() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        Self::open_at(&db_path)
    }

    /// Open the cache database at a specific path.
    ///
    /// This is useful for testing with temporary databases.
    pub fn open_at(path: &PathBuf) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create cache directory: {:?}", parent))?;
        }

        info!("Opening cache database at: {:?}", path);

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {:?}", path))?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])
            .context("Failed to enable foreign keys")?;

        let cache = Self { conn: Mutex::new(conn) };
        cache.run_migrations()?;

        Ok(cache)
    }

    /// Get the default database path using XDG directories.
    fn default_db_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "raibid-labs", "scryforge")
            .context("Failed to determine project directories")?;

        let data_dir = project_dirs.data_dir();
        Ok(data_dir.join("cache.db"))
    }

    /// Run database migrations to set up the schema.
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create schema_version table if it doesn't exist
        conn
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY
                )",
                [],
            )
            .context("Failed to create schema_version table")?;

        // Get current schema version
        let current_version: i32 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        debug!("Current schema version: {}", current_version);

        // Apply migrations
        if current_version < 1 {
            drop(conn); // Release lock before calling migrate_to_v1
            self.migrate_to_v1()?;
        }

        // Future migrations would go here:
        // if current_version < 2 {
        //     self.migrate_to_v2()?;
        // }

        Ok(())
    }

    /// Migration to version 1: Initial schema.
    fn migrate_to_v1(&self) -> Result<()> {
        info!("Running migration to schema version 1");

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Create streams table
        tx.execute(
            "CREATE TABLE IF NOT EXISTS streams (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                stream_type TEXT NOT NULL,
                icon TEXT,
                unread_count INTEGER,
                total_count INTEGER,
                last_updated TEXT,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )
        .context("Failed to create streams table")?;

        // Create items table
        tx.execute(
            "CREATE TABLE IF NOT EXISTS items (
                id TEXT PRIMARY KEY,
                stream_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content_data TEXT NOT NULL,
                author_name TEXT,
                author_email TEXT,
                author_url TEXT,
                author_avatar_url TEXT,
                published TEXT,
                updated TEXT,
                url TEXT,
                thumbnail_url TEXT,
                is_read INTEGER NOT NULL DEFAULT 0,
                is_saved INTEGER NOT NULL DEFAULT 0,
                tags TEXT NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (stream_id) REFERENCES streams(id) ON DELETE CASCADE
            )",
            [],
        )
        .context("Failed to create items table")?;

        // Create sync_state table
        tx.execute(
            "CREATE TABLE IF NOT EXISTS sync_state (
                provider_id TEXT PRIMARY KEY,
                last_sync TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )
        .context("Failed to create sync_state table")?;

        // Create indexes for better query performance
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_streams_provider
             ON streams(provider_id)",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_stream
             ON items(stream_id)",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_published
             ON items(published DESC)",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_is_read
             ON items(is_read)",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_is_saved
             ON items(is_saved)",
            [],
        )?;

        // Mark migration as complete
        tx.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .context("Failed to update schema version")?;

        tx.commit()?;

        info!("Successfully migrated to schema version 1");
        Ok(())
    }

    /// Serialize metadata HashMap to JSON string.
    fn serialize_metadata(metadata: &HashMap<String, String>) -> Result<String> {
        serde_json::to_string(metadata).context("Failed to serialize metadata")
    }

    /// Deserialize metadata from JSON string.
    fn deserialize_metadata(json: &str) -> Result<HashMap<String, String>> {
        serde_json::from_str(json).context("Failed to deserialize metadata")
    }

    /// Serialize tags Vec to JSON string.
    fn serialize_tags(tags: &[String]) -> Result<String> {
        serde_json::to_string(tags).context("Failed to serialize tags")
    }

    /// Deserialize tags from JSON string.
    fn deserialize_tags(json: &str) -> Result<Vec<String>> {
        serde_json::from_str(json).context("Failed to deserialize tags")
    }
}

impl Cache for SqliteCache {
    fn get_streams(&self, provider_id: Option<&str>) -> Result<Vec<Stream>> {
        let conn = self.conn.lock().unwrap();

        let streams = if let Some(provider) = provider_id {
            let mut stmt = conn.prepare(
                "SELECT id, name, provider_id, stream_type, icon, unread_count,
                        total_count, last_updated, metadata
                 FROM streams
                 WHERE provider_id = ?
                 ORDER BY name",
            )?;

            let result = stmt.query_map([provider], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let provider_id: String = row.get(2)?;
                let stream_type_str: String = row.get(3)?;
                let icon: Option<String> = row.get(4)?;
                let unread_count: Option<u32> = row.get(5)?;
                let total_count: Option<u32> = row.get(6)?;
                let last_updated: Option<String> = row.get(7)?;
                let metadata_json: String = row.get(8)?;

                let stream_type = match stream_type_str.as_str() {
                    "Feed" => scryforge_provider_core::StreamType::Feed,
                    "Collection" => scryforge_provider_core::StreamType::Collection,
                    "SavedItems" => scryforge_provider_core::StreamType::SavedItems,
                    "Community" => scryforge_provider_core::StreamType::Community,
                    other => scryforge_provider_core::StreamType::Custom(other.to_string()),
                };

                let metadata = Self::deserialize_metadata(&metadata_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    ))?;

                let last_updated = last_updated
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                Ok(Stream {
                    id: StreamId(id),
                    name,
                    provider_id,
                    stream_type,
                    icon,
                    unread_count,
                    total_count,
                    last_updated,
                    metadata,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, name, provider_id, stream_type, icon, unread_count,
                        total_count, last_updated, metadata
                 FROM streams
                 ORDER BY provider_id, name",
            )?;

            let result = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let provider_id: String = row.get(2)?;
                let stream_type_str: String = row.get(3)?;
                let icon: Option<String> = row.get(4)?;
                let unread_count: Option<u32> = row.get(5)?;
                let total_count: Option<u32> = row.get(6)?;
                let last_updated: Option<String> = row.get(7)?;
                let metadata_json: String = row.get(8)?;

                let stream_type = match stream_type_str.as_str() {
                    "Feed" => scryforge_provider_core::StreamType::Feed,
                    "Collection" => scryforge_provider_core::StreamType::Collection,
                    "SavedItems" => scryforge_provider_core::StreamType::SavedItems,
                    "Community" => scryforge_provider_core::StreamType::Community,
                    other => scryforge_provider_core::StreamType::Custom(other.to_string()),
                };

                let metadata = Self::deserialize_metadata(&metadata_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    ))?;

                let last_updated = last_updated
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                Ok(Stream {
                    id: StreamId(id),
                    name,
                    provider_id,
                    stream_type,
                    icon,
                    unread_count,
                    total_count,
                    last_updated,
                    metadata,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        Ok(streams)
    }

    fn get_items(&self, stream_id: &StreamId, limit: Option<u32>) -> Result<Vec<Item>> {
        let conn = self.conn.lock().unwrap();

        let query = if limit.is_some() {
            "SELECT id, stream_id, title, content_type, content_data,
                    author_name, author_email, author_url, author_avatar_url,
                    published, updated, url, thumbnail_url, is_read, is_saved,
                    tags, metadata
             FROM items
             WHERE stream_id = ?
             ORDER BY published DESC, created_at DESC
             LIMIT ?"
        } else {
            "SELECT id, stream_id, title, content_type, content_data,
                    author_name, author_email, author_url, author_avatar_url,
                    published, updated, url, thumbnail_url, is_read, is_saved,
                    tags, metadata
             FROM items
             WHERE stream_id = ?
             ORDER BY published DESC, created_at DESC"
        };

        let mut stmt = conn.prepare(query)?;

        let items = if let Some(lim) = limit {
            stmt.query_map(params![stream_id.as_str(), lim], Self::row_to_item)?
        } else {
            stmt.query_map(params![stream_id.as_str()], Self::row_to_item)?
        };

        items.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to fetch items from cache")
    }

    fn upsert_streams(&self, streams: &[Stream]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for stream in streams {
            let stream_type_str = match &stream.stream_type {
                scryforge_provider_core::StreamType::Feed => "Feed",
                scryforge_provider_core::StreamType::Collection => "Collection",
                scryforge_provider_core::StreamType::SavedItems => "SavedItems",
                scryforge_provider_core::StreamType::Community => "Community",
                scryforge_provider_core::StreamType::Custom(s) => s.as_str(),
            };

            let metadata_json = Self::serialize_metadata(&stream.metadata)?;
            let last_updated = stream.last_updated.map(|dt| dt.to_rfc3339());

            tx.execute(
                "INSERT INTO streams
                    (id, name, provider_id, stream_type, icon, unread_count,
                     total_count, last_updated, metadata, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    stream_type = excluded.stream_type,
                    icon = excluded.icon,
                    unread_count = excluded.unread_count,
                    total_count = excluded.total_count,
                    last_updated = excluded.last_updated,
                    metadata = excluded.metadata,
                    updated_at = datetime('now')",
                params![
                    stream.id.as_str(),
                    &stream.name,
                    &stream.provider_id,
                    stream_type_str,
                    &stream.icon,
                    stream.unread_count,
                    stream.total_count,
                    last_updated,
                    metadata_json,
                ],
            )?;
        }

        tx.commit()?;
        debug!("Upserted {} streams", streams.len());
        Ok(())
    }

    fn upsert_items(&self, items: &[Item]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for item in items {
            let (content_type, content_data) = Self::serialize_content(&item.content)?;
            let tags_json = Self::serialize_tags(&item.tags)?;
            let metadata_json = Self::serialize_metadata(&item.metadata)?;

            let author_name = item.author.as_ref().map(|a| &a.name);
            let author_email = item.author.as_ref().and_then(|a| a.email.as_ref());
            let author_url = item.author.as_ref().and_then(|a| a.url.as_ref());
            let author_avatar_url = item.author.as_ref().and_then(|a| a.avatar_url.as_ref());

            let published = item.published.map(|dt| dt.to_rfc3339());
            let updated = item.updated.map(|dt| dt.to_rfc3339());

            tx.execute(
                "INSERT INTO items
                    (id, stream_id, title, content_type, content_data,
                     author_name, author_email, author_url, author_avatar_url,
                     published, updated, url, thumbnail_url, is_read, is_saved,
                     tags, metadata, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, datetime('now'))
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    content_type = excluded.content_type,
                    content_data = excluded.content_data,
                    author_name = excluded.author_name,
                    author_email = excluded.author_email,
                    author_url = excluded.author_url,
                    author_avatar_url = excluded.author_avatar_url,
                    published = excluded.published,
                    updated = excluded.updated,
                    url = excluded.url,
                    thumbnail_url = excluded.thumbnail_url,
                    tags = excluded.tags,
                    metadata = excluded.metadata,
                    updated_at = datetime('now')",
                params![
                    item.id.as_str(),
                    item.stream_id.as_str(),
                    &item.title,
                    content_type,
                    content_data,
                    author_name,
                    author_email,
                    author_url,
                    author_avatar_url,
                    published,
                    updated,
                    &item.url,
                    &item.thumbnail_url,
                    item.is_read as i32,
                    item.is_saved as i32,
                    tags_json,
                    metadata_json,
                ],
            )?;
        }

        tx.commit()?;
        debug!("Upserted {} items", items.len());
        Ok(())
    }

    fn mark_read(&self, item_id: &ItemId, is_read: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let rows = conn.execute(
            "UPDATE items SET is_read = ?, updated_at = datetime('now') WHERE id = ?",
            params![is_read as i32, item_id.as_str()],
        )?;

        if rows == 0 {
            warn!("Attempted to mark non-existent item as read: {}", item_id.as_str());
        }

        Ok(())
    }

    fn mark_starred(&self, item_id: &ItemId, is_starred: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let rows = conn.execute(
            "UPDATE items SET is_saved = ?, updated_at = datetime('now') WHERE id = ?",
            params![is_starred as i32, item_id.as_str()],
        )?;

        if rows == 0 {
            warn!("Attempted to mark non-existent item as starred: {}", item_id.as_str());
        }

        Ok(())
    }

    fn get_sync_state(&self, provider_id: &str) -> Result<Option<DateTime<Utc>>> {
        let conn = self.conn.lock().unwrap();

        let result: Option<String> = conn
            .query_row(
                "SELECT last_sync FROM sync_state WHERE provider_id = ?",
                params![provider_id],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc)))
    }

    fn update_sync_state(&self, provider_id: &str, last_sync: DateTime<Utc>) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO sync_state (provider_id, last_sync, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(provider_id) DO UPDATE SET
                last_sync = excluded.last_sync,
                updated_at = datetime('now')",
            params![provider_id, last_sync.to_rfc3339()],
        )?;

        Ok(())
    }

    fn search_items(
        &self,
        query: &str,
        stream_id: Option<&str>,
        content_type: Option<&str>,
        is_read: Option<bool>,
        is_saved: Option<bool>,
    ) -> Result<Vec<Item>> {
        let conn = self.conn.lock().unwrap();

        // Build the query dynamically based on filters
        let mut sql = String::from(
            "SELECT id, stream_id, title, content_type, content_data,
                    author_name, author_email, author_url, author_avatar_url,
                    published, updated, url, thumbnail_url, is_read, is_saved,
                    tags, metadata
             FROM items
             WHERE 1=1"
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Add search query filter (search in title and serialized content)
        if !query.is_empty() {
            sql.push_str(" AND (title LIKE ? OR content_data LIKE ?)");
            let search_pattern = format!("%{}%", query);
            params_vec.push(Box::new(search_pattern.clone()));
            params_vec.push(Box::new(search_pattern));
        }

        // Add stream filter
        if let Some(stream) = stream_id {
            sql.push_str(" AND stream_id = ?");
            params_vec.push(Box::new(stream.to_string()));
        }

        // Add content type filter
        if let Some(ctype) = content_type {
            sql.push_str(" AND content_type = ?");
            params_vec.push(Box::new(ctype.to_string()));
        }

        // Add is_read filter
        if let Some(read_status) = is_read {
            sql.push_str(" AND is_read = ?");
            params_vec.push(Box::new(read_status as i32));
        }

        // Add is_saved filter
        if let Some(saved_status) = is_saved {
            sql.push_str(" AND is_saved = ?");
            params_vec.push(Box::new(saved_status as i32));
        }

        // Order by published date, newest first
        sql.push_str(" ORDER BY published DESC, created_at DESC LIMIT 100");

        let mut stmt = conn.prepare(&sql)?;

        // Convert params to references for query_map
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|p| p.as_ref() as &dyn rusqlite::ToSql)
            .collect();

        let items = stmt.query_map(params_refs.as_slice(), Self::row_to_item)?;

        items.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to search items from cache")
    }
}

// Helper methods for SqliteCache
impl SqliteCache {
    /// Convert a database row to an Item.
    fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<Item> {
        let id: String = row.get(0)?;
        let stream_id: String = row.get(1)?;
        let title: String = row.get(2)?;
        let content_type: String = row.get(3)?;
        let content_data: String = row.get(4)?;
        let author_name: Option<String> = row.get(5)?;
        let author_email: Option<String> = row.get(6)?;
        let author_url: Option<String> = row.get(7)?;
        let author_avatar_url: Option<String> = row.get(8)?;
        let published: Option<String> = row.get(9)?;
        let updated: Option<String> = row.get(10)?;
        let url: Option<String> = row.get(11)?;
        let thumbnail_url: Option<String> = row.get(12)?;
        let is_read: i32 = row.get(13)?;
        let is_saved: i32 = row.get(14)?;
        let tags_json: String = row.get(15)?;
        let metadata_json: String = row.get(16)?;

        let content = Self::deserialize_content(&content_type, &content_data)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            ))?;

        let author = author_name.map(|name| scryforge_provider_core::Author {
            name,
            email: author_email,
            url: author_url,
            avatar_url: author_avatar_url,
        });

        let published = published
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let updated = updated
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let tags = Self::deserialize_tags(&tags_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            ))?;

        let metadata = Self::deserialize_metadata(&metadata_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                16,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            ))?;

        Ok(Item {
            id: ItemId(id),
            stream_id: StreamId(stream_id),
            title,
            content,
            author,
            published,
            updated,
            url,
            thumbnail_url,
            is_read: is_read != 0,
            is_saved: is_saved != 0,
            tags,
            metadata,
        })
    }

    /// Serialize item content to type and JSON data.
    fn serialize_content(content: &scryforge_provider_core::ItemContent) -> Result<(String, String)> {
        use scryforge_provider_core::ItemContent;

        let (content_type, data) = match content {
            ItemContent::Text(text) => ("Text", serde_json::json!({"text": text})),
            ItemContent::Markdown(md) => ("Markdown", serde_json::json!({"markdown": md})),
            ItemContent::Html(html) => ("Html", serde_json::json!({"html": html})),
            ItemContent::Email { subject, body_text, body_html, snippet } => (
                "Email",
                serde_json::json!({
                    "subject": subject,
                    "body_text": body_text,
                    "body_html": body_html,
                    "snippet": snippet,
                }),
            ),
            ItemContent::Article { summary, full_content } => (
                "Article",
                serde_json::json!({
                    "summary": summary,
                    "full_content": full_content,
                }),
            ),
            ItemContent::Video { description, duration_seconds, view_count } => (
                "Video",
                serde_json::json!({
                    "description": description,
                    "duration_seconds": duration_seconds,
                    "view_count": view_count,
                }),
            ),
            ItemContent::Track { album, duration_ms, artists } => (
                "Track",
                serde_json::json!({
                    "album": album,
                    "duration_ms": duration_ms,
                    "artists": artists,
                }),
            ),
            ItemContent::Task { body, due_date, is_completed } => (
                "Task",
                serde_json::json!({
                    "body": body,
                    "due_date": due_date,
                    "is_completed": is_completed,
                }),
            ),
            ItemContent::Event { description, start, end, location, is_all_day } => (
                "Event",
                serde_json::json!({
                    "description": description,
                    "start": start.to_rfc3339(),
                    "end": end.to_rfc3339(),
                    "location": location,
                    "is_all_day": is_all_day,
                }),
            ),
            ItemContent::Bookmark { description } => (
                "Bookmark",
                serde_json::json!({
                    "description": description,
                }),
            ),
            ItemContent::Generic { body } => (
                "Generic",
                serde_json::json!({
                    "body": body,
                }),
            ),
        };

        Ok((content_type.to_string(), serde_json::to_string(&data)?))
    }

    /// Deserialize item content from type and JSON data.
    fn deserialize_content(content_type: &str, content_data: &str) -> Result<scryforge_provider_core::ItemContent> {
        use scryforge_provider_core::ItemContent;

        let data: serde_json::Value = serde_json::from_str(content_data)?;

        let content = match content_type {
            "Text" => ItemContent::Text(
                data["text"]
                    .as_str()
                    .context("Missing text field")?
                    .to_string(),
            ),
            "Markdown" => ItemContent::Markdown(
                data["markdown"]
                    .as_str()
                    .context("Missing markdown field")?
                    .to_string(),
            ),
            "Html" => ItemContent::Html(
                data["html"]
                    .as_str()
                    .context("Missing html field")?
                    .to_string(),
            ),
            "Email" => ItemContent::Email {
                subject: data["subject"]
                    .as_str()
                    .context("Missing subject field")?
                    .to_string(),
                body_text: data["body_text"].as_str().map(|s| s.to_string()),
                body_html: data["body_html"].as_str().map(|s| s.to_string()),
                snippet: data["snippet"]
                    .as_str()
                    .context("Missing snippet field")?
                    .to_string(),
            },
            "Article" => ItemContent::Article {
                summary: data["summary"].as_str().map(|s| s.to_string()),
                full_content: data["full_content"].as_str().map(|s| s.to_string()),
            },
            "Video" => ItemContent::Video {
                description: data["description"]
                    .as_str()
                    .context("Missing description field")?
                    .to_string(),
                duration_seconds: data["duration_seconds"].as_u64().map(|v| v as u32),
                view_count: data["view_count"].as_u64(),
            },
            "Track" => ItemContent::Track {
                album: data["album"].as_str().map(|s| s.to_string()),
                duration_ms: data["duration_ms"].as_u64().map(|v| v as u32),
                artists: data["artists"]
                    .as_array()
                    .context("Missing artists field")?
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
            },
            "Task" => ItemContent::Task {
                body: data["body"].as_str().map(|s| s.to_string()),
                due_date: data["due_date"]
                    .as_str()
                    .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
                is_completed: data["is_completed"].as_bool().unwrap_or(false),
            },
            "Event" => ItemContent::Event {
                description: data["description"].as_str().map(|s| s.to_string()),
                start: data["start"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .context("Missing or invalid start field")?,
                end: data["end"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .context("Missing or invalid end field")?,
                location: data["location"].as_str().map(|s| s.to_string()),
                is_all_day: data["is_all_day"].as_bool().unwrap_or(false),
            },
            "Bookmark" => ItemContent::Bookmark {
                description: data["description"].as_str().map(|s| s.to_string()),
            },
            "Generic" => ItemContent::Generic {
                body: data["body"].as_str().map(|s| s.to_string()),
            },
            _ => ItemContent::Generic {
                body: Some(format!("Unknown content type: {}", content_type)),
            },
        };

        Ok(content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::{ItemContent, StreamType};
    use tempfile::TempDir;

    fn create_test_cache() -> Result<SqliteCache> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("test.db");
        let cache = SqliteCache::open_at(&path)?;
        // Keep tempdir alive by leaking it - tests are short-lived anyway
        std::mem::forget(temp_dir);
        Ok(cache)
    }

    fn create_test_stream(id: &str, provider_id: &str) -> Stream {
        Stream {
            id: StreamId(id.to_string()),
            name: format!("Test Stream {}", id),
            provider_id: provider_id.to_string(),
            stream_type: StreamType::Feed,
            icon: Some("icon.png".to_string()),
            unread_count: Some(5),
            total_count: Some(10),
            last_updated: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    fn create_test_item(id: &str, stream_id: &str) -> Item {
        Item {
            id: ItemId(id.to_string()),
            stream_id: StreamId(stream_id.to_string()),
            title: format!("Test Item {}", id),
            content: ItemContent::Text("Test content".to_string()),
            author: Some(scryforge_provider_core::Author {
                name: "Test Author".to_string(),
                email: Some("test@example.com".to_string()),
                url: None,
                avatar_url: None,
            }),
            published: Some(Utc::now()),
            updated: None,
            url: Some("https://example.com".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec!["test".to_string()],
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_create_cache() -> Result<()> {
        let cache = create_test_cache()?;
        assert!(cache.get_streams(None)?.is_empty());
        Ok(())
    }

    #[test]
    fn test_upsert_and_get_streams() -> Result<()> {
        let cache = create_test_cache()?;

        let stream1 = create_test_stream("test:feed:1", "test-provider");
        let stream2 = create_test_stream("test:feed:2", "test-provider");

        cache.upsert_streams(&[stream1.clone(), stream2.clone()])?;

        let streams = cache.get_streams(None)?;
        assert_eq!(streams.len(), 2);

        let streams = cache.get_streams(Some("test-provider"))?;
        assert_eq!(streams.len(), 2);

        let streams = cache.get_streams(Some("other-provider"))?;
        assert_eq!(streams.len(), 0);

        Ok(())
    }

    #[test]
    fn test_upsert_stream_updates_existing() -> Result<()> {
        let cache = create_test_cache()?;

        let mut stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        stream.name = "Updated Stream".to_string();
        stream.unread_count = Some(15);
        cache.upsert_streams(&[stream.clone()])?;

        let streams = cache.get_streams(None)?;
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].name, "Updated Stream");
        assert_eq!(streams[0].unread_count, Some(15));

        Ok(())
    }

    #[test]
    fn test_upsert_and_get_items() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let item1 = create_test_item("test:item:1", "test:feed:1");
        let item2 = create_test_item("test:item:2", "test:feed:1");

        cache.upsert_items(&[item1.clone(), item2.clone()])?;

        let items = cache.get_items(&StreamId("test:feed:1".to_string()), None)?;
        assert_eq!(items.len(), 2);

        let items = cache.get_items(&StreamId("test:feed:1".to_string()), Some(1))?;
        assert_eq!(items.len(), 1);

        Ok(())
    }

    #[test]
    fn test_mark_read() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let item = create_test_item("test:item:1", "test:feed:1");
        cache.upsert_items(&[item.clone()])?;

        cache.mark_read(&item.id, true)?;

        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_read);

        cache.mark_read(&item.id, false)?;

        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_read);

        Ok(())
    }

    #[test]
    fn test_mark_starred() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let item = create_test_item("test:item:1", "test:feed:1");
        cache.upsert_items(&[item.clone()])?;

        cache.mark_starred(&item.id, true)?;

        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), 1);
        assert!(items[0].is_saved);

        cache.mark_starred(&item.id, false)?;

        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_saved);

        Ok(())
    }

    #[test]
    fn test_sync_state() -> Result<()> {
        let cache = create_test_cache()?;

        let provider_id = "test-provider";

        let state = cache.get_sync_state(provider_id)?;
        assert!(state.is_none());

        let now = Utc::now();
        cache.update_sync_state(provider_id, now)?;

        let state = cache.get_sync_state(provider_id)?;
        assert!(state.is_some());

        // Allow for minor timestamp differences due to serialization
        let diff = (state.unwrap() - now).num_seconds().abs();
        assert!(diff < 2);

        Ok(())
    }

    #[test]
    fn test_different_content_types() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let content_types = vec![
            ItemContent::Text("Plain text".to_string()),
            ItemContent::Markdown("# Markdown".to_string()),
            ItemContent::Html("<p>HTML</p>".to_string()),
            ItemContent::Email {
                subject: "Test Email".to_string(),
                body_text: Some("Body".to_string()),
                body_html: None,
                snippet: "Snippet".to_string(),
            },
            ItemContent::Article {
                summary: Some("Summary".to_string()),
                full_content: None,
            },
            ItemContent::Video {
                description: "Video description".to_string(),
                duration_seconds: Some(120),
                view_count: Some(1000),
            },
            ItemContent::Track {
                album: Some("Album".to_string()),
                duration_ms: Some(180000),
                artists: vec!["Artist 1".to_string(), "Artist 2".to_string()],
            },
            ItemContent::Bookmark {
                description: Some("Bookmark".to_string()),
            },
            ItemContent::Generic {
                body: Some("Generic content".to_string()),
            },
        ];

        for (i, content) in content_types.iter().enumerate() {
            let mut item = create_test_item(&format!("test:item:{}", i), "test:feed:1");
            item.content = content.clone();
            cache.upsert_items(&[item])?;
        }

        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), content_types.len());

        Ok(())
    }

    #[test]
    fn test_foreign_key_cascade() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let item = create_test_item("test:item:1", "test:feed:1");
        cache.upsert_items(&[item.clone()])?;

        // Delete the stream
        let conn = cache.conn.lock().unwrap();
        conn.execute("DELETE FROM streams WHERE id = ?", params![stream.id.as_str()])?;
        drop(conn);

        // Items should also be deleted due to CASCADE
        let items = cache.get_items(&stream.id, None)?;
        assert_eq!(items.len(), 0);

        Ok(())
    }

    #[test]
    fn test_search_items_by_text() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let mut item1 = create_test_item("test:item:1", "test:feed:1");
        item1.title = "Rust programming tutorial".to_string();

        let mut item2 = create_test_item("test:item:2", "test:feed:1");
        item2.title = "Python machine learning".to_string();

        let mut item3 = create_test_item("test:item:3", "test:feed:1");
        item3.title = "Advanced Rust patterns".to_string();

        cache.upsert_items(&[item1, item2, item3])?;

        // Search for "Rust"
        let results = cache.search_items("Rust", None, None, None, None)?;
        assert_eq!(results.len(), 2);

        // Search for "Python"
        let results = cache.search_items("Python", None, None, None, None)?;
        assert_eq!(results.len(), 1);

        // Search for non-existent term
        let results = cache.search_items("JavaScript", None, None, None, None)?;
        assert_eq!(results.len(), 0);

        Ok(())
    }

    #[test]
    fn test_search_items_with_filters() -> Result<()> {
        let cache = create_test_cache()?;

        let stream1 = create_test_stream("test:feed:1", "test-provider");
        let stream2 = create_test_stream("test:feed:2", "test-provider");
        cache.upsert_streams(&[stream1.clone(), stream2.clone()])?;

        let mut item1 = create_test_item("test:item:1", "test:feed:1");
        item1.title = "Test article".to_string();
        item1.is_read = false;
        item1.is_saved = false;

        let mut item2 = create_test_item("test:item:2", "test:feed:1");
        item2.title = "Another test".to_string();
        item2.is_read = true;
        item2.is_saved = false;

        let mut item3 = create_test_item("test:item:3", "test:feed:2");
        item3.title = "Test item".to_string();
        item3.is_read = false;
        item3.is_saved = true;

        cache.upsert_items(&[item1, item2, item3])?;

        // Search for unread items
        let results = cache.search_items("test", None, None, Some(false), None)?;
        assert_eq!(results.len(), 2);

        // Search for read items
        let results = cache.search_items("test", None, None, Some(true), None)?;
        assert_eq!(results.len(), 1);

        // Search for saved items
        let results = cache.search_items("test", None, None, None, Some(true))?;
        assert_eq!(results.len(), 1);

        // Search within specific stream
        let results = cache.search_items("test", Some("test:feed:1"), None, None, None)?;
        assert_eq!(results.len(), 2);

        let results = cache.search_items("test", Some("test:feed:2"), None, None, None)?;
        assert_eq!(results.len(), 1);

        Ok(())
    }

    #[test]
    fn test_search_items_empty_query() -> Result<()> {
        let cache = create_test_cache()?;

        let stream = create_test_stream("test:feed:1", "test-provider");
        cache.upsert_streams(&[stream.clone()])?;

        let item1 = create_test_item("test:item:1", "test:feed:1");
        let item2 = create_test_item("test:item:2", "test:feed:1");

        cache.upsert_items(&[item1, item2])?;

        // Empty query should return all items (up to limit)
        let results = cache.search_items("", None, None, None, None)?;
        assert_eq!(results.len(), 2);

        Ok(())
    }
}

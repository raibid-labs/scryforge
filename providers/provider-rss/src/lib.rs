//! # provider-rss
//!
//! RSS/Atom feed provider for Scryforge.
//!
//! This provider fetches and parses RSS 2.0 and Atom feeds, converting entries into
//! Scryforge items. It supports:
//!
//! - Multiple feed URLs
//! - OPML import for bulk feed subscription
//! - Both RSS and Atom formats via feed-rs
//! - Article content extraction
//!
//! ## Configuration
//!
//! The provider accepts a list of feed URLs via `RssProviderConfig`:
//!
//! ```rust
//! use provider_rss::{RssProvider, RssProviderConfig};
//!
//! let config = RssProviderConfig {
//!     feeds: vec![
//!         "https://example.com/feed.xml".to_string(),
//!         "https://blog.example.com/atom.xml".to_string(),
//!     ],
//! };
//! let provider = RssProvider::new(config);
//! ```
//!
//! ## OPML Import
//!
//! Use `RssProviderConfig::from_opml()` to import feeds from an OPML file:
//!
//! ```rust,no_run
//! use provider_rss::RssProviderConfig;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RssProviderConfig::from_opml("/path/to/subscriptions.opml").await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::Utc;
use feed_rs::parser;
use reqwest::Client;
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::time::Instant;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum RssError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Feed parsing failed: {0}")]
    Parse(String),

    #[error("OPML parsing failed: {0}")]
    Opml(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid feed URL: {0}")]
    InvalidUrl(String),
}

impl From<RssError> for StreamError {
    fn from(err: RssError) -> Self {
        match err {
            RssError::Http(e) => StreamError::Network(e.to_string()),
            RssError::Parse(e) => StreamError::Provider(format!("Feed parsing error: {e}")),
            RssError::Opml(e) => StreamError::Provider(format!("OPML parsing error: {e}")),
            RssError::Io(e) => StreamError::Internal(format!("IO error: {e}")),
            RssError::InvalidUrl(e) => StreamError::Provider(format!("Invalid URL: {e}")),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the RSS provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RssProviderConfig {
    /// List of feed URLs to fetch
    pub feeds: Vec<String>,
}

impl RssProviderConfig {
    /// Create a new configuration with the given feed URLs.
    pub fn new(feeds: Vec<String>) -> Self {
        Self { feeds }
    }

    /// Create a configuration from an OPML file.
    ///
    /// Extracts all feed URLs from the OPML outline structure.
    pub async fn from_opml(path: &str) -> std::result::Result<Self, RssError> {
        let content = tokio::fs::read_to_string(path).await?;
        Self::from_opml_string(&content)
    }

    /// Create a configuration from an OPML string.
    pub fn from_opml_string(content: &str) -> std::result::Result<Self, RssError> {
        let document = opml::OPML::from_str(content).map_err(|e| RssError::Opml(e.to_string()))?;

        let mut feeds = Vec::new();
        Self::extract_feeds_from_outline(&document.body.outlines, &mut feeds);

        Ok(Self { feeds })
    }

    /// Recursively extract feed URLs from OPML outlines.
    fn extract_feeds_from_outline(outlines: &[opml::Outline], feeds: &mut Vec<String>) {
        for outline in outlines {
            // Check for xml_url attribute (the actual feed URL)
            if let Some(xml_url) = &outline.xml_url {
                feeds.push(xml_url.clone());
            }

            // Recursively process child outlines
            if !outline.outlines.is_empty() {
                Self::extract_feeds_from_outline(&outline.outlines, feeds);
            }
        }
    }
}

// ============================================================================
// RSS Provider
// ============================================================================

/// RSS/Atom feed provider.
///
/// Fetches and parses RSS 2.0 and Atom feeds, converting entries to Scryforge items.
pub struct RssProvider {
    config: RssProviderConfig,
    client: Client,
}

impl RssProvider {
    /// Create a new RSS provider with the given configuration.
    pub fn new(config: RssProviderConfig) -> Self {
        let client = Client::builder()
            .user_agent("Scryforge/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    /// Fetch and parse a feed from a URL.
    async fn fetch_feed(&self, url: &str) -> std::result::Result<feed_rs::model::Feed, RssError> {
        let response = self.client.get(url).send().await?.error_for_status()?;

        let content = response.bytes().await?;
        parser::parse(&content[..]).map_err(|e| RssError::Parse(e.to_string()))
    }

    /// Convert a feed-rs entry to a Scryforge Item.
    fn entry_to_item(
        &self,
        entry: &feed_rs::model::Entry,
        stream_id: &StreamId,
        feed_url: &str,
    ) -> Item {
        // Extract the entry ID (use the id field or generate a UUID)
        let entry_id = if !entry.id.is_empty() {
            entry.id.clone()
        } else {
            format!("rss:{}", uuid::Uuid::new_v4())
        };

        // Extract title
        let title = entry
            .title
            .as_ref()
            .map(|t| t.content.trim().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        // Extract author information
        let author = entry.authors.first().map(|person| Author {
            name: person.name.clone(),
            email: person.email.clone(),
            url: person.uri.clone(),
            avatar_url: None,
        });

        // Extract published/updated dates
        let published = entry.published.map(|dt| dt.with_timezone(&Utc));
        let updated = entry.updated.map(|dt| dt.with_timezone(&Utc));

        // Extract URL (prefer links with alternate or first available)
        let url = entry
            .links
            .iter()
            .find(|link| link.rel.as_deref() == Some("alternate"))
            .or_else(|| entry.links.first())
            .map(|link| link.href.clone());

        // Extract thumbnail
        let thumbnail_url = entry.media.iter().find_map(|media| {
            media
                .thumbnails
                .first()
                .map(|thumb| thumb.image.uri.clone())
        });

        // Extract summary and content
        let summary = entry.summary.as_ref().map(|s| s.content.trim().to_string());

        let full_content = entry.content.as_ref().and_then(|c| {
            c.body.as_ref().map(|body| {
                // Prefer text content, fall back to raw HTML
                body.trim().to_string()
            })
        });

        // Build content
        let content = ItemContent::Article {
            summary,
            full_content,
        };

        // Extract categories as tags
        let tags: Vec<String> = entry
            .categories
            .iter()
            .map(|cat| cat.term.clone())
            .collect();

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("feed_url".to_string(), feed_url.to_string());
        if let Some(media_type) = entry.media.first().and_then(|m| m.content.first()) {
            if let Some(mime) = &media_type.content_type {
                metadata.insert("media_type".to_string(), mime.to_string());
            }
        }

        Item {
            id: ItemId::new("rss", &entry_id),
            stream_id: stream_id.clone(),
            title,
            content,
            author,
            published,
            updated,
            url,
            thumbnail_url,
            is_read: false,
            is_saved: false,
            tags,
            metadata,
        }
    }
}

#[async_trait]
impl Provider for RssProvider {
    fn id(&self) -> &'static str {
        "rss"
    }

    fn name(&self) -> &'static str {
        "RSS/Atom Feeds"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch the first feed to verify connectivity
        if let Some(feed_url) = self.config.feeds.first() {
            match self.fetch_feed(feed_url).await {
                Ok(_) => Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some(format!("Successfully fetched feed: {}", feed_url)),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                }),
                Err(e) => Ok(ProviderHealth {
                    is_healthy: false,
                    message: Some(format!("Failed to fetch feed: {}", e)),
                    last_sync: None,
                    error_count: 1,
                }),
            }
        } else {
            Ok(ProviderHealth {
                is_healthy: true,
                message: Some("No feeds configured".to_string()),
                last_sync: None,
                error_count: 0,
            })
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = Instant::now();
        let mut items_added = 0;
        let mut errors = Vec::new();

        for feed_url in &self.config.feeds {
            match self.fetch_feed(feed_url).await {
                Ok(feed) => {
                    items_added += feed.entries.len() as u32;
                }
                Err(e) => {
                    errors.push(format!("Failed to fetch {}: {}", feed_url, e));
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SyncResult {
            success: errors.is_empty(),
            items_added,
            items_updated: 0,
            items_removed: 0,
            errors,
            duration_ms,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: false,
            has_saved_items: false,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
            Action {
                id: "open_browser".to_string(),
                name: "Open in Browser".to_string(),
                description: "Open article in web browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show article preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy article URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Read".to_string(),
                description: "Mark article as read".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
            Action {
                id: "save".to_string(),
                name: "Save Article".to_string(),
                description: "Save article for later".to_string(),
                kind: ActionKind::Save,
                keyboard_shortcut: Some("s".to_string()),
            },
        ])
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser => {
                if let Some(url) = &item.url {
                    Ok(ActionResult {
                        success: true,
                        message: Some(format!("Opening: {}", url)),
                        data: Some(serde_json::json!({ "url": url })),
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available for this item".to_string()),
                        data: None,
                    })
                }
            }
            ActionKind::CopyLink => {
                if let Some(url) = &item.url {
                    Ok(ActionResult {
                        success: true,
                        message: Some("Link copied to clipboard".to_string()),
                        data: Some(serde_json::json!({ "url": url })),
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available for this item".to_string()),
                        data: None,
                    })
                }
            }
            _ => Ok(ActionResult {
                success: true,
                message: Some(format!("Executed action: {}", action.name)),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl HasFeeds for RssProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let mut feeds = Vec::new();

        for (idx, feed_url) in self.config.feeds.iter().enumerate() {
            // Try to fetch the feed to get metadata
            match self.fetch_feed(feed_url).await {
                Ok(feed) => {
                    let feed_title = feed
                        .title
                        .as_ref()
                        .map(|t| t.content.trim().to_string())
                        .unwrap_or_else(|| format!("Feed {}", idx + 1));

                    let feed_description = feed
                        .description
                        .as_ref()
                        .map(|d| d.content.trim().to_string());

                    feeds.push(Feed {
                        id: FeedId(format!("rss:{}", idx)),
                        name: feed_title,
                        description: feed_description,
                        icon: Some("📰".to_string()),
                        unread_count: Some(feed.entries.len() as u32),
                        total_count: Some(feed.entries.len() as u32),
                    });
                }
                Err(_e) => {
                    // If we can't fetch the feed, still list it with minimal info
                    feeds.push(Feed {
                        id: FeedId(format!("rss:{}", idx)),
                        name: feed_url.clone(),
                        description: Some("Failed to fetch feed".to_string()),
                        icon: Some("📰".to_string()),
                        unread_count: None,
                        total_count: None,
                    });
                }
            }
        }

        Ok(feeds)
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        // Extract the feed index from the feed_id
        let feed_index = feed_id
            .0
            .strip_prefix("rss:")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| StreamError::StreamNotFound(feed_id.0.clone()))?;

        // Get the feed URL
        let feed_url = self
            .config
            .feeds
            .get(feed_index)
            .ok_or_else(|| StreamError::StreamNotFound(feed_id.0.clone()))?;

        // Fetch the feed
        let feed = self.fetch_feed(feed_url).await?;

        // Create stream ID
        let stream_id = StreamId::new("rss", "feed", &feed_id.0);

        // Convert entries to items
        let mut items: Vec<Item> = feed
            .entries
            .iter()
            .map(|entry| self.entry_to_item(entry, &stream_id, feed_url))
            .collect();

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Apply since filter
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

        // Sort by published date (newest first)
        items.sort_by(|a, b| {
            let a_date = a.published.unwrap_or_else(Utc::now);
            let b_date = b.published.unwrap_or_else(Utc::now);
            b_date.cmp(&a_date)
        });

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        let items = items.into_iter().skip(offset);
        let items = if let Some(limit) = limit {
            items.take(limit).collect()
        } else {
            items.collect()
        };

        Ok(items)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <link>https://example.com</link>
    <description>A test RSS feed</description>
    <item>
      <title>First Article</title>
      <link>https://example.com/article1</link>
      <description>This is the first article</description>
      <pubDate>Mon, 01 Jan 2024 12:00:00 GMT</pubDate>
      <category>Technology</category>
    </item>
    <item>
      <title>Second Article</title>
      <link>https://example.com/article2</link>
      <description>This is the second article</description>
      <pubDate>Mon, 02 Jan 2024 12:00:00 GMT</pubDate>
      <category>Science</category>
    </item>
  </channel>
</rss>"#;

    const SAMPLE_ATOM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Test Atom Feed</title>
  <link href="https://example.com"/>
  <updated>2024-01-02T12:00:00Z</updated>
  <entry>
    <title>Atom Article</title>
    <link href="https://example.com/atom1"/>
    <id>https://example.com/atom1</id>
    <updated>2024-01-01T12:00:00Z</updated>
    <summary>This is an Atom entry</summary>
    <author>
      <name>Jane Doe</name>
      <email>jane@example.com</email>
    </author>
  </entry>
</feed>"#;

    const SAMPLE_OPML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>Test Subscriptions</title>
  </head>
  <body>
    <outline text="Technology" title="Technology">
      <outline text="Tech Blog" xmlUrl="https://example.com/tech/rss" htmlUrl="https://example.com/tech"/>
      <outline text="Dev Blog" xmlUrl="https://example.com/dev/feed" htmlUrl="https://example.com/dev"/>
    </outline>
    <outline text="News" xmlUrl="https://example.com/news/atom" htmlUrl="https://example.com/news"/>
  </body>
</opml>"#;

    #[test]
    fn test_parse_rss_feed() {
        let feed = parser::parse(SAMPLE_RSS.as_bytes()).unwrap();
        assert_eq!(feed.title.unwrap().content, "Test Feed");
        assert_eq!(feed.entries.len(), 2);
        assert_eq!(
            feed.entries[0].title.as_ref().unwrap().content,
            "First Article"
        );
    }

    #[test]
    fn test_parse_atom_feed() {
        let feed = parser::parse(SAMPLE_ATOM.as_bytes()).unwrap();
        assert_eq!(feed.title.unwrap().content, "Test Atom Feed");
        assert_eq!(feed.entries.len(), 1);
        assert_eq!(
            feed.entries[0].title.as_ref().unwrap().content,
            "Atom Article"
        );
        assert_eq!(feed.entries[0].authors.len(), 1);
        assert_eq!(feed.entries[0].authors[0].name, "Jane Doe");
    }

    #[test]
    fn test_opml_parsing() {
        let config = RssProviderConfig::from_opml_string(SAMPLE_OPML).unwrap();
        assert_eq!(config.feeds.len(), 3);
        assert!(config
            .feeds
            .contains(&"https://example.com/tech/rss".to_string()));
        assert!(config
            .feeds
            .contains(&"https://example.com/dev/feed".to_string()));
        assert!(config
            .feeds
            .contains(&"https://example.com/news/atom".to_string()));
    }

    #[test]
    fn test_entry_to_item_conversion() {
        let feed = parser::parse(SAMPLE_RSS.as_bytes()).unwrap();
        let config = RssProviderConfig::new(vec!["https://example.com/rss".to_string()]);
        let provider = RssProvider::new(config);

        let stream_id = StreamId::new("rss", "feed", "rss:0");
        let item = provider.entry_to_item(&feed.entries[0], &stream_id, "https://example.com/rss");

        assert_eq!(item.title, "First Article");
        assert_eq!(item.url, Some("https://example.com/article1".to_string()));
        assert!(matches!(item.content, ItemContent::Article { .. }));
        assert_eq!(item.tags, vec!["Technology".to_string()]);
        assert!(!item.is_read);
        assert!(!item.is_saved);
    }

    #[test]
    fn test_atom_entry_with_author() {
        let feed = parser::parse(SAMPLE_ATOM.as_bytes()).unwrap();
        let config = RssProviderConfig::new(vec!["https://example.com/atom".to_string()]);
        let provider = RssProvider::new(config);

        let stream_id = StreamId::new("rss", "feed", "rss:0");
        let item = provider.entry_to_item(&feed.entries[0], &stream_id, "https://example.com/atom");

        assert_eq!(item.title, "Atom Article");
        assert!(item.author.is_some());
        let author = item.author.unwrap();
        assert_eq!(author.name, "Jane Doe");
        assert_eq!(author.email, Some("jane@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_provider_basics() {
        let config = RssProviderConfig::new(vec![]);
        let provider = RssProvider::new(config);

        assert_eq!(provider.id(), "rss");
        assert_eq!(provider.name(), "RSS/Atom Feeds");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(!caps.has_collections);
        assert!(!caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_health_check_no_feeds() {
        let config = RssProviderConfig::new(vec![]);
        let provider = RssProvider::new(config);

        let health = provider.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert_eq!(health.error_count, 0);
    }

    #[tokio::test]
    async fn test_available_actions() {
        let config = RssProviderConfig::new(vec![]);
        let provider = RssProvider::new(config);

        let item = Item {
            id: ItemId::new("rss", "test"),
            stream_id: StreamId::new("rss", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();
        assert_eq!(actions.len(), 5);
        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Preview));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
        assert!(actions.iter().any(|a| a.kind == ActionKind::MarkRead));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Save));
    }

    #[tokio::test]
    async fn test_execute_action_open_browser() {
        let config = RssProviderConfig::new(vec![]);
        let provider = RssProvider::new(config);

        let item = Item {
            id: ItemId::new("rss", "test"),
            stream_id: StreamId::new("rss", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let action = Action {
            id: "open_browser".to_string(),
            name: "Open in Browser".to_string(),
            description: "Open article in web browser".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("https://example.com"));
    }

    #[tokio::test]
    async fn test_execute_action_no_url() {
        let config = RssProviderConfig::new(vec![]);
        let provider = RssProvider::new(config);

        let item = Item {
            id: ItemId::new("rss", "test"),
            stream_id: StreamId::new("rss", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let action = Action {
            id: "open_browser".to_string(),
            name: "Open in Browser".to_string(),
            description: "Open article in web browser".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(!result.success);
        assert!(result.message.unwrap().contains("No URL available"));
    }
}

//! # provider-rss
//!
//! RSS/Atom feed provider for Scryforge.
//!
//! This provider fetches and parses RSS 2.0 and Atom feeds, converting them into
//! unified `Item` structs. It supports OPML import for bulk feed configuration
//! and handles common edge cases like missing dates, relative URLs, and HTML in titles.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use feed_rs::parser;
use fusabi_streams_core::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use url::Url;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum RssError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Feed parse error: {0}")]
    Parse(String),

    #[error("OPML parse error: {0}")]
    OpmlParse(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Feed not found: {0}")]
    FeedNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<RssError> for StreamError {
    fn from(err: RssError) -> Self {
        match err {
            RssError::Http(e) => StreamError::Network(e.to_string()),
            RssError::Parse(e) => StreamError::Provider(format!("Parse error: {e}")),
            RssError::OpmlParse(e) => StreamError::Provider(format!("OPML parse error: {e}")),
            RssError::InvalidUrl(e) => StreamError::Provider(format!("Invalid URL: {e}")),
            RssError::FeedNotFound(e) => StreamError::StreamNotFound(e),
            RssError::Io(e) => StreamError::Internal(format!("IO error: {e}")),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for an RSS feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssFeedConfig {
    /// Unique identifier for this feed
    pub id: String,
    /// Display name for the feed
    pub name: String,
    /// Feed URL
    pub url: String,
    /// Optional description
    pub description: Option<String>,
    /// Optional icon/emoji
    pub icon: Option<String>,
}

/// Configuration for the RSS provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssConfig {
    /// List of feeds to track
    pub feeds: Vec<RssFeedConfig>,
}

impl Default for RssConfig {
    fn default() -> Self {
        Self { feeds: Vec::new() }
    }
}

// ============================================================================
// OPML Support
// ============================================================================

/// Parse an OPML file and extract feed URLs.
///
/// OPML (Outline Processor Markup Language) is a common format for exchanging
/// feed subscriptions between feed readers.
pub fn parse_opml(opml_content: &str) -> std::result::Result<Vec<RssFeedConfig>, RssError> {

    let mut feeds = Vec::new();
    let mut in_body = false;

    for line in opml_content.lines() {
        let line = line.trim();

        if line.contains("<body>") {
            in_body = true;
            continue;
        }

        if line.contains("</body>") {
            break;
        }

        if in_body && line.contains("<outline") {
            // Extract attributes from outline tag
            if let Some(url) = extract_xml_attr(line, "xmlUrl").or_else(|| extract_xml_attr(line, "url")) {
                let name = extract_xml_attr(line, "title")
                    .or_else(|| extract_xml_attr(line, "text"))
                    .unwrap_or_else(|| url.clone());

                // Generate a simple ID from the URL
                let id = url
                    .replace("https://", "")
                    .replace("http://", "")
                    .replace('/', "-")
                    .replace('.', "-");

                feeds.push(RssFeedConfig {
                    id,
                    name,
                    url,
                    description: extract_xml_attr(line, "description"),
                    icon: None,
                });
            }
        }
    }

    if feeds.is_empty() {
        return Err(RssError::OpmlParse("No feeds found in OPML".to_string()));
    }

    Ok(feeds)
}

/// Extract an XML attribute value from a tag string.
fn extract_xml_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!(r#"{attr}=""#);
    if let Some(start) = line.find(&pattern) {
        let start = start + pattern.len();
        if let Some(end) = line[start..].find('"') {
            return Some(line[start..start + end].to_string());
        }
    }
    None
}

// ============================================================================
// RSS Provider
// ============================================================================

/// RSS/Atom feed provider.
pub struct RssProvider {
    config: Arc<RssConfig>,
    client: Client,
}

impl RssProvider {
    /// Create a new RSS provider with the given configuration.
    pub fn new(config: RssConfig) -> Self {
        let client = Client::builder()
            .user_agent("Scryforge/0.1.0 (RSS Feed Reader)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    /// Create a new RSS provider from an OPML file.
    pub fn from_opml(opml_content: &str) -> std::result::Result<Self, RssError> {
        let feeds = parse_opml(opml_content)?;
        Ok(Self::new(RssConfig { feeds }))
    }

    /// Fetch and parse a feed from a URL.
    async fn fetch_feed(&self, url: &str) -> std::result::Result<feed_rs::model::Feed, RssError> {
        debug!("Fetching feed from: {}", url);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(RssError::Parse(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let content = response.bytes().await?;
        let feed = parser::parse(&content[..])
            .map_err(|e| RssError::Parse(format!("Feed parse error: {}", e)))?;

        Ok(feed)
    }

    /// Convert a feed-rs Entry to our Item struct.
    fn entry_to_item(&self, entry: &feed_rs::model::Entry, feed_config: &RssFeedConfig) -> Item {
        let stream_id = StreamId::new("rss", "feed", &feed_config.id);

        // Generate item ID from entry ID or link
        let item_id = if !entry.id.is_empty() {
            entry.id.clone()
        } else if let Some(link) = entry.links.first() {
            link.href.clone()
        } else {
            let title = entry.title.as_ref().map(|t| t.content.as_str()).unwrap_or("untitled");
            format!("{}-{}", feed_config.id, title)
        };

        let item_id = ItemId::new("rss", &item_id);

        // Extract title, handling HTML entities
        let title = entry
            .title
            .as_ref()
            .map(|t| decode_html(&t.content))
            .unwrap_or_else(|| "Untitled".to_string());

        // Extract content
        let (summary, full_content) = if let Some(content) = entry.content.as_ref() {
            let body = content.body.as_ref().map(|b| b.to_string());
            (entry.summary.as_ref().map(|s| decode_html(&s.content)), body)
        } else if let Some(summary) = entry.summary.as_ref() {
            (Some(decode_html(&summary.content)), None)
        } else {
            (None, None)
        };

        let content = ItemContent::Article {
            summary,
            full_content,
        };

        // Extract author
        let author = entry.authors.first().map(|a| Author {
            name: a.name.clone(),
            email: a.email.clone(),
            url: a.uri.clone(),
            avatar_url: None,
        });

        // Extract published date, fallback to updated date or current time
        let published = entry
            .published
            .or(entry.updated)
            .or_else(|| Some(Utc::now()));

        let updated = entry.updated;

        // Extract URL, resolving relative URLs if possible
        let url = entry.links.first().map(|link| {
            if let Ok(base_url) = Url::parse(&feed_config.url) {
                base_url
                    .join(&link.href)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| link.href.clone())
            } else {
                link.href.clone()
            }
        });

        // Extract thumbnail from media content
        let thumbnail_url = entry.media.iter().find_map(|media| {
            media.thumbnails.first().map(|t| t.image.uri.clone())
        });

        // Extract categories as tags
        let tags = entry
            .categories
            .iter()
            .map(|c| c.term.clone())
            .collect();

        // Build metadata
        let mut metadata = HashMap::new();
        if let Some(lang) = &entry.language {
            metadata.insert("language".to_string(), lang.clone());
        }

        Item {
            id: item_id,
            stream_id,
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

    /// Find a feed configuration by ID.
    fn find_feed(&self, feed_id: &FeedId) -> Option<&RssFeedConfig> {
        self.config.feeds.iter().find(|f| f.id == feed_id.0)
    }
}

/// Decode HTML entities in a string.
///
/// This handles common HTML entities that might appear in feed titles and content.
fn decode_html(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
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
        // Try to fetch the first feed to check connectivity
        if let Some(feed_config) = self.config.feeds.first() {
            match self.fetch_feed(&feed_config.url).await {
                Ok(_) => Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some("Successfully fetched sample feed".to_string()),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                }),
                Err(e) => {
                    warn!("Health check failed: {}", e);
                    Ok(ProviderHealth {
                        is_healthy: false,
                        message: Some(format!("Failed to fetch feed: {}", e)),
                        last_sync: None,
                        error_count: 1,
                    })
                }
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
        let start = std::time::Instant::now();
        let mut items_added = 0;
        let mut errors = Vec::new();

        info!("Syncing {} RSS feeds", self.config.feeds.len());

        for feed_config in &self.config.feeds {
            match self.fetch_feed(&feed_config.url).await {
                Ok(feed) => {
                    items_added += feed.entries.len() as u32;
                    debug!("Fetched {} items from {}", feed.entries.len(), feed_config.name);
                }
                Err(e) => {
                    error!("Failed to fetch feed {}: {}", feed_config.name, e);
                    errors.push(format!("{}: {}", feed_config.name, e));
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

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show article preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
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
                name: "Save".to_string(),
                description: "Save article for later".to_string(),
                kind: ActionKind::Save,
                keyboard_shortcut: Some("s".to_string()),
            },
        ];

        // Add "Open in Browser" if URL is available
        if item.url.is_some() {
            actions.insert(
                0,
                Action {
                    id: "open_browser".to_string(),
                    name: "Open in Browser".to_string(),
                    description: "Open article in web browser".to_string(),
                    kind: ActionKind::OpenInBrowser,
                    keyboard_shortcut: Some("o".to_string()),
                },
            );

            actions.push(Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy article URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser => {
                if let Some(url) = &item.url {
                    info!("Opening URL in browser: {}", url);
                    Ok(ActionResult {
                        success: true,
                        message: Some(format!("Opening: {}", url)),
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
                        message: Some("No URL available".to_string()),
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
}

#[async_trait]
impl HasFeeds for RssProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        Ok(self
            .config
            .feeds
            .iter()
            .map(|fc| Feed {
                id: FeedId(fc.id.clone()),
                name: fc.name.clone(),
                description: fc.description.clone(),
                icon: fc.icon.clone(),
                unread_count: None, // Would require caching/database
                total_count: None,  // Would require caching/database
            })
            .collect())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let feed_config = self
            .find_feed(feed_id)
            .ok_or_else(|| StreamError::StreamNotFound(feed_id.0.clone()))?;

        let feed = self
            .fetch_feed(&feed_config.url)
            .await
            .map_err(StreamError::from)?;

        let mut items: Vec<Item> = feed
            .entries
            .iter()
            .map(|entry| self.entry_to_item(entry, feed_config))
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
            let a_date = a.published.unwrap_or(DateTime::<Utc>::MIN_UTC);
            let b_date = b.published.unwrap_or(DateTime::<Utc>::MIN_UTC);
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
            <title>Test Article &amp; Title</title>
            <link>https://example.com/article1</link>
            <description>This is a test article summary.</description>
            <pubDate>Mon, 01 Jan 2024 12:00:00 GMT</pubDate>
            <category>Technology</category>
            <category>News</category>
        </item>
        <item>
            <title>Second Article</title>
            <link>https://example.com/article2</link>
            <description>Another test article.</description>
            <pubDate>Tue, 02 Jan 2024 12:00:00 GMT</pubDate>
        </item>
    </channel>
</rss>"#;

    const SAMPLE_ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
    <title>Test Atom Feed</title>
    <link href="https://example.com/atom"/>
    <updated>2024-01-02T12:00:00Z</updated>
    <entry>
        <title>Atom Article &lt;Test&gt;</title>
        <link href="https://example.com/atom-article1"/>
        <id>atom-1</id>
        <updated>2024-01-01T12:00:00Z</updated>
        <summary>Atom summary content.</summary>
        <author>
            <name>John Doe</name>
            <email>john@example.com</email>
        </author>
    </entry>
</feed>"#;

    const SAMPLE_OPML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
    <head>
        <title>My Feeds</title>
    </head>
    <body>
        <outline type="rss" text="Tech News" title="Tech News" xmlUrl="https://example.com/tech/rss" />
        <outline type="rss" text="Programming Blog" title="Programming Blog" xmlUrl="https://blog.example.com/feed.xml" description="A great programming blog" />
        <outline text="Folder">
            <outline type="rss" text="Nested Feed" xmlUrl="https://nested.example.com/rss" />
        </outline>
    </body>
</opml>"#;

    #[test]
    fn test_parse_opml() {
        let feeds = parse_opml(SAMPLE_OPML).unwrap();

        assert_eq!(feeds.len(), 3);

        assert_eq!(feeds[0].name, "Tech News");
        assert_eq!(feeds[0].url, "https://example.com/tech/rss");

        assert_eq!(feeds[1].name, "Programming Blog");
        assert_eq!(feeds[1].url, "https://blog.example.com/feed.xml");
        assert_eq!(
            feeds[1].description,
            Some("A great programming blog".to_string())
        );

        assert_eq!(feeds[2].name, "Nested Feed");
        assert_eq!(feeds[2].url, "https://nested.example.com/rss");
    }

    #[test]
    fn test_decode_html() {
        assert_eq!(decode_html("Test &amp; Title"), "Test & Title");
        assert_eq!(decode_html("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_html("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(decode_html("&#39;apostrophe&#39;"), "'apostrophe'");
    }

    #[test]
    fn test_rss_provider_creation() {
        let config = RssConfig {
            feeds: vec![RssFeedConfig {
                id: "test-feed".to_string(),
                name: "Test Feed".to_string(),
                url: "https://example.com/feed.xml".to_string(),
                description: Some("A test feed".to_string()),
                icon: Some("📰".to_string()),
            }],
        };

        let provider = RssProvider::new(config);

        assert_eq!(provider.id(), "rss");
        assert_eq!(provider.name(), "RSS/Atom Feeds");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(!caps.has_collections);
    }

    #[test]
    fn test_rss_provider_from_opml() {
        let provider = RssProvider::from_opml(SAMPLE_OPML).unwrap();
        assert_eq!(provider.config.feeds.len(), 3);
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let config = RssConfig {
            feeds: vec![
                RssFeedConfig {
                    id: "feed1".to_string(),
                    name: "Feed 1".to_string(),
                    url: "https://example.com/feed1.xml".to_string(),
                    description: Some("First feed".to_string()),
                    icon: Some("📰".to_string()),
                },
                RssFeedConfig {
                    id: "feed2".to_string(),
                    name: "Feed 2".to_string(),
                    url: "https://example.com/feed2.xml".to_string(),
                    description: None,
                    icon: None,
                },
            ],
        };

        let provider = RssProvider::new(config);
        let feeds = provider.list_feeds().await.unwrap();

        assert_eq!(feeds.len(), 2);
        assert_eq!(feeds[0].id.0, "feed1");
        assert_eq!(feeds[0].name, "Feed 1");
        assert_eq!(feeds[1].id.0, "feed2");
    }

    #[test]
    fn test_feed_parsing_rss() {
        // Test that feed-rs can parse our sample RSS
        let feed = parser::parse(SAMPLE_RSS.as_bytes()).unwrap();

        assert_eq!(feed.entries.len(), 2);
        assert!(feed.entries[0].title.is_some());
        assert_eq!(feed.entries[0].categories.len(), 2);
    }

    #[test]
    fn test_feed_parsing_atom() {
        // Test that feed-rs can parse our sample Atom
        let feed = parser::parse(SAMPLE_ATOM.as_bytes()).unwrap();

        assert_eq!(feed.entries.len(), 1);
        assert!(feed.entries[0].title.is_some());
        assert_eq!(feed.entries[0].authors.len(), 1);
    }

    #[tokio::test]
    async fn test_available_actions_with_url() {
        let provider = RssProvider::new(RssConfig::default());
        let item = Item {
            id: ItemId::new("rss", "test"),
            stream_id: StreamId::new("rss", "feed", "test"),
            title: "Test Article".to_string(),
            content: ItemContent::Article {
                summary: Some("Summary".to_string()),
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com/article".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Open in Browser, Preview, Mark Read, Save, Copy Link
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0].kind, ActionKind::OpenInBrowser);
        assert_eq!(actions[1].kind, ActionKind::Preview);
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_available_actions_without_url() {
        let provider = RssProvider::new(RssConfig::default());
        let item = Item {
            id: ItemId::new("rss", "test"),
            stream_id: StreamId::new("rss", "feed", "test"),
            title: "Test Article".to_string(),
            content: ItemContent::Article {
                summary: Some("Summary".to_string()),
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

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Preview, Mark Read, Save (no Open/Copy Link)
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].kind, ActionKind::Preview);
        assert!(!actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(!actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_execute_action_open_browser() {
        let provider = RssProvider::new(RssConfig::default());
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
            description: "Open in browser".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }
}

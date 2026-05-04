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
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;

// ============================================================================
// Public constants
// ============================================================================

/// Metadata key under which the audio enclosure URL (if any) is exposed on
/// `Item.metadata`. Set when an RSS/Atom entry carries an audio enclosure or
/// `<media:content>` element with an audio MIME type or audio file extension.
pub const AUDIO_URL_METADATA_KEY: &str = "audio_url";

/// Stable action ID for the transcribe-to-vault custom action.
pub const TRANSCRIBE_ACTION_ID: &str = "transcribe_to_vault";

/// File extensions treated as audio when MIME info is missing or unhelpful.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "ogg", "wav", "flac", "aac", "opus"];

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

    /// Output directory for the `transcribe-to-vault` action. When `None`,
    /// the provider falls back to `~/transcripts/scryforge` (resolved at
    /// action-execution time via the `directories` crate).
    #[serde(default)]
    pub transcribe_dir: Option<PathBuf>,

    /// Optional override for the `scribe` binary path. When `None`, the
    /// `transcribe-to-vault` action invokes `scribe` from `$PATH`. Primarily
    /// a hook for tests; production deployments should rely on `$PATH`.
    #[serde(default)]
    pub scribe_bin: Option<PathBuf>,
}

impl RssProviderConfig {
    /// Create a new configuration with the given feed URLs.
    pub fn new(feeds: Vec<String>) -> Self {
        Self {
            feeds,
            transcribe_dir: None,
            scribe_bin: None,
        }
    }

    /// Builder-style: set the transcribe output directory.
    pub fn with_transcribe_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.transcribe_dir = Some(dir.into());
        self
    }

    /// Builder-style: set an explicit path to the `scribe` binary
    /// (otherwise resolved from `$PATH`).
    pub fn with_scribe_bin(mut self, bin: impl Into<PathBuf>) -> Self {
        self.scribe_bin = Some(bin.into());
        self
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

        Ok(Self {
            feeds,
            transcribe_dir: None,
            scribe_bin: None,
        })
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
// Audio enclosure detection
// ============================================================================

/// Return `true` if the given MIME type looks like an audio MIME (i.e.
/// starts with `audio/` ASCII-case-insensitively).
fn is_audio_mime(mime: &str) -> bool {
    mime.trim().to_ascii_lowercase().starts_with("audio/")
}

/// Return `true` if the URL's path component ends in a known audio
/// extension. Handles query strings and fragments.
fn url_has_audio_extension(url: &str) -> bool {
    // Strip query string and fragment so a URL like
    // `https://example.com/ep1.mp3?token=abc#t=10` still matches.
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();

    AUDIO_EXTENSIONS
        .iter()
        .any(|ext| path.ends_with(&format!(".{ext}")))
}

/// Walk the entry's `<media:content>` children (which feed-rs uses to surface
/// both Media RSS `<media:content>` and standard RSS `<enclosure>`) and
/// return the first URL that looks audio-y. The check is:
///
/// 1. MIME type starts with `audio/`, OR
/// 2. URL has a known audio extension (`.mp3`, `.m4a`, `.ogg`, ...).
fn extract_audio_url(entry: &feed_rs::model::Entry) -> Option<String> {
    for media in &entry.media {
        for content in &media.content {
            // Prefer the explicit MIME type.
            if let Some(mime) = &content.content_type {
                if is_audio_mime(mime.as_ref()) {
                    if let Some(url) = &content.url {
                        return Some(url.to_string());
                    }
                }
            }
            // Fall back to extension sniffing.
            if let Some(url) = &content.url {
                if url_has_audio_extension(url.as_ref()) {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

/// Resolve the transcribe output directory: prefer the per-stream config,
/// else `~/transcripts/scryforge`, else the current working directory as a
/// last resort. Creates the directory if it does not exist.
fn resolve_transcribe_dir(configured: Option<&Path>) -> std::result::Result<PathBuf, RssError> {
    let dir = if let Some(p) = configured {
        p.to_path_buf()
    } else {
        match directories::UserDirs::new() {
            Some(user_dirs) => user_dirs.home_dir().join("transcripts").join("scryforge"),
            None => PathBuf::from("."),
        }
    };

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(RssError::Io)?;
    }
    Ok(dir)
}

/// Parse scribe's stdout for the path of the produced markdown file. Scribe
/// (per `docs/04-cli-reference.md`) writes a `<slug>-<timestamp>.md` and a
/// matching `.json` to the configured output directory. The conventional way
/// to surface that path is via stdout; if scribe ever changes its stdout
/// format we fall back to a glob of `*.md` in the output directory.
fn parse_scribe_output_path(stdout: &str, out_dir: &Path) -> Option<PathBuf> {
    // Heuristic: scan stdout for tokens ending in `.md`. Choose the last one
    // (scribe is expected to log the result file last).
    let candidate = stdout
        .split_whitespace()
        .rev()
        .find(|tok| tok.ends_with(".md"))
        .map(|s| {
            s.trim_matches(|c: char| !c.is_ascii_graphic() || c == '"' || c == '\'')
                .to_string()
        });

    if let Some(c) = candidate {
        let p = PathBuf::from(&c);
        if p.is_absolute() {
            return Some(p);
        }
        return Some(out_dir.join(p));
    }

    // Fallback: pick the most recently modified .md in out_dir.
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(out_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                            newest = Some((modified, path));
                        }
                    }
                }
            }
        }
    }
    newest.map(|(_, p)| p)
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

        // Detect audio enclosure (podcast support). This walks every
        // `<media:group>` / `<media:content>` on the entry and surfaces the
        // first URL that looks audio-y via either MIME type or extension.
        if let Some(audio_url) = extract_audio_url(entry) {
            metadata.insert(AUDIO_URL_METADATA_KEY.to_string(), audio_url);
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

    /// Run Scribe on the audio enclosure of `item` and return an
    /// `ActionResult` describing the outcome.
    ///
    /// The body shells out to `scribe transcribe --input <url> --out-dir <dir>`
    /// via `tokio::process::Command` so it does not block the daemon's
    /// runtime; the future yields while whisper runs. The configured
    /// `transcribe_dir` (or `~/transcripts/scryforge`) is created if missing.
    /// On success, the path of the produced markdown is parsed from scribe's
    /// stdout and returned in `ActionResult.data`.
    async fn transcribe_item(&self, item: &Item) -> Result<ActionResult> {
        let audio_url = match item.metadata.get(AUDIO_URL_METADATA_KEY) {
            Some(u) => u.clone(),
            None => {
                return Ok(ActionResult {
                    success: false,
                    message: Some(
                        "Item has no audio enclosure; transcribe-to-vault is only \
                         valid on podcast items."
                            .to_string(),
                    ),
                    data: None,
                });
            }
        };

        let out_dir = resolve_transcribe_dir(self.config.transcribe_dir.as_deref())
            .map_err(StreamError::from)?;

        let scribe_bin = self
            .config
            .scribe_bin
            .as_deref()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("scribe"));

        let mut cmd = tokio::process::Command::new(&scribe_bin);
        cmd.arg("transcribe")
            .arg("--input")
            .arg(&audio_url)
            .arg("--out-dir")
            .arg(&out_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = cmd
            .spawn()
            .map_err(|e| {
                StreamError::Provider(format!(
                    "Failed to spawn `{}`: {}. Is Scribe installed and on $PATH?",
                    scribe_bin.display(),
                    e
                ))
            })?
            .wait_with_output()
            .await
            .map_err(|e| StreamError::Provider(format!("Scribe process error: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Ok(ActionResult {
                success: false,
                message: Some(format!(
                    "Scribe exited with status {}: {}",
                    output.status,
                    stderr.trim()
                )),
                data: Some(serde_json::json!({
                    "exit_code": output.status.code(),
                    "stderr": stderr,
                })),
            });
        }

        let md_path = parse_scribe_output_path(&stdout, &out_dir);

        Ok(ActionResult {
            success: true,
            message: Some(match &md_path {
                Some(p) => format!("Transcript written to {}", p.display()),
                None => format!("Transcript written under {}", out_dir.display()),
            }),
            data: Some(serde_json::json!({
                "out_dir": out_dir,
                "output_path": md_path,
                "audio_url": audio_url,
            })),
        })
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

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
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
        ];

        // Surface transcribe-to-vault only on items that look like podcast
        // episodes (i.e. carry an audio enclosure URL in metadata).
        if item.metadata.contains_key(AUDIO_URL_METADATA_KEY) {
            actions.push(Action {
                id: TRANSCRIBE_ACTION_ID.to_string(),
                name: "Transcribe to Vault".to_string(),
                description: "Run Scribe (whisper.cpp) on this episode's audio \
                              and write a Markdown transcript to the configured \
                              transcribe directory."
                    .to_string(),
                kind: ActionKind::Custom(TRANSCRIBE_ACTION_ID.to_string()),
                keyboard_shortcut: Some("t".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match &action.kind {
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
            ActionKind::Custom(id) if id == TRANSCRIBE_ACTION_ID => {
                self.transcribe_item(item).await
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

    // ====================================================================
    // Audio enclosure detection (podcast support)
    // ====================================================================

    /// Synthetic podcast RSS with a typed audio enclosure (the canonical
    /// podcast pattern: `<enclosure type="audio/mpeg" url="..."/>`).
    const SAMPLE_PODCAST_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Podcast</title>
    <link>https://example.com/podcast</link>
    <description>A test podcast</description>
    <item>
      <title>Episode 42 — The Answer</title>
      <link>https://example.com/podcast/ep42</link>
      <description>Show notes for ep 42.</description>
      <pubDate>Mon, 28 Apr 2026 14:30:00 GMT</pubDate>
      <enclosure url="https://cdn.example.com/podcast/ep42.mp3" length="123456789" type="audio/mpeg"/>
    </item>
    <item>
      <title>Episode 43 — Untyped enclosure</title>
      <link>https://example.com/podcast/ep43</link>
      <description>This one omits the type attribute; we should still detect via .mp3 ext.</description>
      <pubDate>Tue, 29 Apr 2026 14:30:00 GMT</pubDate>
      <enclosure url="https://cdn.example.com/podcast/ep43.mp3?token=abc" length="98765" type=""/>
    </item>
    <item>
      <title>Article with no audio</title>
      <link>https://example.com/podcast/post1</link>
      <description>Just text.</description>
      <pubDate>Wed, 30 Apr 2026 14:30:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn test_is_audio_mime() {
        assert!(is_audio_mime("audio/mpeg"));
        assert!(is_audio_mime("AUDIO/MP4"));
        assert!(is_audio_mime("  audio/ogg  "));
        assert!(!is_audio_mime("video/mp4"));
        assert!(!is_audio_mime("text/html"));
        assert!(!is_audio_mime(""));
    }

    #[test]
    fn test_url_has_audio_extension() {
        assert!(url_has_audio_extension("https://example.com/ep1.mp3"));
        assert!(url_has_audio_extension("https://example.com/ep1.MP3"));
        assert!(url_has_audio_extension("https://example.com/ep1.m4a"));
        assert!(url_has_audio_extension("https://example.com/ep1.ogg"));
        assert!(url_has_audio_extension("https://example.com/ep1.wav"));
        assert!(url_has_audio_extension("https://example.com/ep1.flac"));
        assert!(url_has_audio_extension(
            "https://example.com/ep1.mp3?token=abc"
        ));
        assert!(url_has_audio_extension("https://example.com/ep1.mp3#t=10"));
        assert!(!url_has_audio_extension("https://example.com/ep1.mp4"));
        assert!(!url_has_audio_extension("https://example.com/index.html"));
        assert!(!url_has_audio_extension("https://example.com/"));
    }

    #[test]
    fn test_extract_audio_url_from_typed_enclosure() {
        let feed = parser::parse(SAMPLE_PODCAST_RSS.as_bytes()).unwrap();
        // First item: typed audio/mpeg enclosure.
        let url = extract_audio_url(&feed.entries[0]);
        assert_eq!(
            url.as_deref(),
            Some("https://cdn.example.com/podcast/ep42.mp3")
        );
    }

    #[test]
    fn test_extract_audio_url_from_extension_fallback() {
        let feed = parser::parse(SAMPLE_PODCAST_RSS.as_bytes()).unwrap();
        // Second item: empty/missing type but .mp3 extension.
        let url = extract_audio_url(&feed.entries[1]);
        assert!(url.is_some(), "expected audio url via extension fallback");
        let url = url.unwrap();
        assert!(url.starts_with("https://cdn.example.com/podcast/ep43.mp3"));
    }

    #[test]
    fn test_extract_audio_url_none_for_text_item() {
        let feed = parser::parse(SAMPLE_PODCAST_RSS.as_bytes()).unwrap();
        // Third item: no enclosure at all.
        let url = extract_audio_url(&feed.entries[2]);
        assert!(url.is_none());
    }

    #[test]
    fn test_entry_to_item_populates_audio_url_metadata() {
        let feed = parser::parse(SAMPLE_PODCAST_RSS.as_bytes()).unwrap();
        let provider = RssProvider::new(RssProviderConfig::new(vec![
            "https://example.com".to_string()
        ]));
        let stream_id = StreamId::new("rss", "feed", "rss:0");

        let podcast_item =
            provider.entry_to_item(&feed.entries[0], &stream_id, "https://example.com");
        assert_eq!(
            podcast_item
                .metadata
                .get(AUDIO_URL_METADATA_KEY)
                .map(String::as_str),
            Some("https://cdn.example.com/podcast/ep42.mp3"),
        );

        let text_item = provider.entry_to_item(&feed.entries[2], &stream_id, "https://example.com");
        assert!(!text_item.metadata.contains_key(AUDIO_URL_METADATA_KEY));
    }

    // ====================================================================
    // transcribe-to-vault action
    // ====================================================================

    fn make_podcast_item(audio_url: &str) -> Item {
        let mut metadata = HashMap::new();
        metadata.insert(AUDIO_URL_METADATA_KEY.to_string(), audio_url.to_string());
        Item {
            id: ItemId::new("rss", "ep42"),
            stream_id: StreamId::new("rss", "feed", "rss:0"),
            title: "Episode 42".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com/podcast/ep42".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata,
        }
    }

    fn make_article_item() -> Item {
        Item {
            id: ItemId::new("rss", "post1"),
            stream_id: StreamId::new("rss", "feed", "rss:0"),
            title: "Article".to_string(),
            content: ItemContent::Article {
                summary: None,
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://example.com/post1".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_available_actions_includes_transcribe_for_podcast() {
        let provider = RssProvider::new(RssProviderConfig::new(vec![]));
        let podcast = make_podcast_item("https://cdn.example.com/ep1.mp3");

        let actions = provider.available_actions(&podcast).await.unwrap();
        let has_transcribe = actions.iter().any(|a| {
            a.id == TRANSCRIBE_ACTION_ID
                && matches!(&a.kind, ActionKind::Custom(s) if s == TRANSCRIBE_ACTION_ID)
        });
        assert!(
            has_transcribe,
            "expected transcribe-to-vault action on items with audio_url metadata"
        );
    }

    #[tokio::test]
    async fn test_available_actions_omits_transcribe_for_article() {
        let provider = RssProvider::new(RssProviderConfig::new(vec![]));
        let article = make_article_item();

        let actions = provider.available_actions(&article).await.unwrap();
        let has_transcribe = actions.iter().any(|a| a.id == TRANSCRIBE_ACTION_ID);
        assert!(
            !has_transcribe,
            "transcribe-to-vault should be hidden on items without audio enclosure"
        );
    }

    /// Build a fake `scribe` shell script that:
    ///   - parses `--input` and `--out-dir`
    ///   - writes a `<stem>-mock.md` and matching `.json` to the out-dir
    ///   - prints the markdown path to stdout
    ///   - exits 0
    ///
    /// On non-Unix, this test is skipped.
    #[cfg(unix)]
    fn make_fake_scribe(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("scribe");
        let script = r#"#!/usr/bin/env bash
set -euo pipefail
input=""
outdir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    transcribe) shift ;;
    --input) input="$2"; shift 2 ;;
    --out-dir) outdir="$2"; shift 2 ;;
    *) shift ;;
  esac
done
mkdir -p "$outdir"
stem=$(basename "$input" | sed 's/\.[^.]*$//' | tr -c 'a-zA-Z0-9' '-' | sed 's/-\+/-/g; s/^-//; s/-$//')
md="$outdir/${stem}-mock.md"
json="$outdir/${stem}-mock.json"
echo "---" > "$md"
echo "type: transcript" >> "$md"
echo "---" >> "$md"
echo "fake transcript body" >> "$md"
echo "{}" > "$json"
echo "$md"
"#;
        std::fs::write(&path, script).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_transcribe_action_invokes_scribe() {
        let tmp = tempfile::tempdir().unwrap();
        let scribe_bin = make_fake_scribe(tmp.path());
        let out_dir = tmp.path().join("transcripts");

        let config = RssProviderConfig::new(vec![])
            .with_transcribe_dir(&out_dir)
            .with_scribe_bin(&scribe_bin);
        let provider = RssProvider::new(config);

        let item = make_podcast_item("https://cdn.example.com/podcast/ep42.mp3");
        let action = Action {
            id: TRANSCRIBE_ACTION_ID.to_string(),
            name: "Transcribe to Vault".to_string(),
            description: String::new(),
            kind: ActionKind::Custom(TRANSCRIBE_ACTION_ID.to_string()),
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(
            result.success,
            "expected success, got: {:?}",
            result.message
        );
        let data = result.data.expect("expected data payload");
        let output_path = data
            .get("output_path")
            .and_then(|v| v.as_str())
            .expect("output_path should be present");
        assert!(
            output_path.ends_with(".md"),
            "expected markdown path, got {output_path}"
        );
        assert!(
            std::path::Path::new(output_path).exists(),
            "fake scribe should have written {output_path}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_transcribe_action_propagates_failure() {
        let tmp = tempfile::tempdir().unwrap();
        // Fake "scribe" that always fails.
        let scribe_bin = tmp.path().join("scribe");
        std::fs::write(
            &scribe_bin,
            "#!/usr/bin/env bash\necho 'boom' 1>&2\nexit 7\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&scribe_bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&scribe_bin, perms).unwrap();

        let out_dir = tmp.path().join("transcripts");
        let config = RssProviderConfig::new(vec![])
            .with_transcribe_dir(&out_dir)
            .with_scribe_bin(&scribe_bin);
        let provider = RssProvider::new(config);

        let item = make_podcast_item("https://cdn.example.com/ep1.mp3");
        let action = Action {
            id: TRANSCRIBE_ACTION_ID.to_string(),
            name: "Transcribe to Vault".to_string(),
            description: String::new(),
            kind: ActionKind::Custom(TRANSCRIBE_ACTION_ID.to_string()),
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(
            !result.success,
            "expected failure when scribe exits non-zero"
        );
        assert!(result.message.unwrap().contains("boom"));
    }

    #[tokio::test]
    async fn test_transcribe_action_without_audio_url_returns_clear_error() {
        let tmp = tempfile::tempdir().unwrap();
        let provider =
            RssProvider::new(RssProviderConfig::new(vec![]).with_transcribe_dir(tmp.path()));

        let item = make_article_item(); // no audio_url metadata
        let action = Action {
            id: TRANSCRIBE_ACTION_ID.to_string(),
            name: "Transcribe to Vault".to_string(),
            description: String::new(),
            kind: ActionKind::Custom(TRANSCRIBE_ACTION_ID.to_string()),
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(!result.success);
        assert!(result.message.unwrap().contains("audio enclosure"));
    }

    /// Real-scribe smoke test. Skipped by default; opt in with
    /// `cargo test -p provider-rss --ignored`.
    #[cfg(unix)]
    #[tokio::test]
    #[ignore]
    async fn test_transcribe_action_real_scribe_smoke() {
        // Requires `scribe` on $PATH AND a small audio fixture at
        // $SCRYFORGE_TEST_AUDIO. Otherwise it's a no-op pass.
        let audio = match std::env::var("SCRYFORGE_TEST_AUDIO") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("SCRYFORGE_TEST_AUDIO not set; skipping real-scribe smoke");
                return;
            }
        };
        let tmp = tempfile::tempdir().unwrap();
        let provider =
            RssProvider::new(RssProviderConfig::new(vec![]).with_transcribe_dir(tmp.path()));
        let item = make_podcast_item(&audio);
        let action = Action {
            id: TRANSCRIBE_ACTION_ID.to_string(),
            name: "Transcribe to Vault".to_string(),
            description: String::new(),
            kind: ActionKind::Custom(TRANSCRIBE_ACTION_ID.to_string()),
            keyboard_shortcut: None,
        };
        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success, "real scribe failed: {:?}", result.message);
    }

    // ====================================================================
    // parse_scribe_output_path
    // ====================================================================

    #[test]
    fn test_parse_scribe_output_path_from_stdout() {
        let tmp = tempfile::tempdir().unwrap();
        let stdout = "/abs/path/to/episode-42-20260428-143000.md\n";
        let p = parse_scribe_output_path(stdout, tmp.path());
        assert_eq!(
            p.as_deref(),
            Some(Path::new("/abs/path/to/episode-42-20260428-143000.md"))
        );
    }

    #[test]
    fn test_parse_scribe_output_path_relative_resolves_against_outdir() {
        let tmp = tempfile::tempdir().unwrap();
        let stdout = "ep1.md\n";
        let p = parse_scribe_output_path(stdout, tmp.path());
        assert_eq!(p, Some(tmp.path().join("ep1.md")));
    }

    #[test]
    fn test_parse_scribe_output_path_falls_back_to_dir_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let md = tmp.path().join("only-one.md");
        std::fs::write(&md, "x").unwrap();
        let p = parse_scribe_output_path("done\n", tmp.path());
        assert_eq!(p, Some(md));
    }
}

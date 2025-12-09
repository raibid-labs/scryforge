//! # provider-youtube
//!
//! YouTube provider for Scryforge using the YouTube Data API v3.
//!
//! This provider fetches subscriptions, playlists, and Watch Later videos from YouTube,
//! converting them into unified `Item` structs. It requires OAuth 2.0 authentication
//! with appropriate YouTube API scopes.
//!
//! ## Features
//!
//! - **Subscriptions (HasFeeds)**: List subscribed channels and fetch recent videos
//! - **Playlists (HasCollections)**: List user playlists and fetch playlist videos
//! - **Watch Later (HasSavedItems)**: Fetch videos from Watch Later playlist
//!
//! ## Authentication
//!
//! This provider requires an OAuth 2.0 access token with the following scopes:
//! - `https://www.googleapis.com/auth/youtube.readonly`
//!
//! ## API Rate Limits
//!
//! YouTube Data API has quota limits. This provider does not implement rate limiting
//! internally - consumers should handle quota exhaustion errors appropriately.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusabi_streams_core::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum YouTubeError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Playlist not found: {0}")]
    PlaylistNotFound(String),

    #[error("Quota exceeded")]
    QuotaExceeded,
}

impl From<YouTubeError> for StreamError {
    fn from(err: YouTubeError) -> Self {
        match err {
            YouTubeError::Http(e) => StreamError::Network(e.to_string()),
            YouTubeError::Api(e) => StreamError::Provider(format!("YouTube API error: {e}")),
            YouTubeError::Auth(e) => StreamError::AuthRequired(e),
            YouTubeError::InvalidResponse(e) => {
                StreamError::Provider(format!("Invalid response: {e}"))
            }
            YouTubeError::ChannelNotFound(e) => StreamError::StreamNotFound(e),
            YouTubeError::PlaylistNotFound(e) => StreamError::StreamNotFound(e),
            YouTubeError::QuotaExceeded => {
                StreamError::RateLimited(3600) // Suggest retry after 1 hour
            }
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the YouTube provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeConfig {
    /// OAuth 2.0 access token with YouTube API scopes
    pub access_token: String,
    /// Optional API key (not required if using OAuth)
    pub api_key: Option<String>,
}

// ============================================================================
// YouTube API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct YouTubeListResponse<T> {
    items: Vec<T>,
    #[serde(rename = "nextPageToken")]
    #[allow(dead_code)]
    next_page_token: Option<String>,
    #[serde(rename = "pageInfo")]
    #[allow(dead_code)]
    page_info: Option<PageInfo>,
}

#[derive(Debug, Deserialize)]
struct PageInfo {
    #[serde(rename = "totalResults")]
    #[allow(dead_code)]
    total_results: Option<u32>,
    #[serde(rename = "resultsPerPage")]
    #[allow(dead_code)]
    results_per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct YouTubeErrorResponse {
    error: YouTubeErrorDetail,
}

#[derive(Debug, Deserialize)]
struct YouTubeErrorDetail {
    code: u16,
    message: String,
}

// Subscription API response
#[derive(Debug, Deserialize)]
struct Subscription {
    #[allow(dead_code)]
    id: String,
    snippet: SubscriptionSnippet,
}

#[derive(Debug, Deserialize)]
struct SubscriptionSnippet {
    title: String,
    description: Option<String>,
    #[serde(rename = "resourceId")]
    resource_id: ResourceId,
    thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
struct ResourceId {
    #[serde(rename = "channelId")]
    channel_id: String,
}

// Playlist API response
#[derive(Debug, Deserialize)]
struct Playlist {
    id: String,
    snippet: PlaylistSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<PlaylistContentDetails>,
}

#[derive(Debug, Deserialize)]
struct PlaylistSnippet {
    title: String,
    description: Option<String>,
    thumbnails: Option<Thumbnails>,
    #[serde(rename = "channelTitle")]
    channel_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaylistContentDetails {
    #[serde(rename = "itemCount")]
    item_count: Option<u32>,
}

// PlaylistItem API response
#[derive(Debug, Deserialize)]
struct PlaylistItem {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    snippet: PlaylistItemSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<PlaylistItemContentDetails>,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemSnippet {
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(rename = "channelTitle")]
    #[allow(dead_code)]
    channel_title: Option<String>,
    #[serde(rename = "publishedAt")]
    #[allow(dead_code)]
    published_at: Option<String>,
    #[allow(dead_code)]
    thumbnails: Option<Thumbnails>,
    #[serde(rename = "resourceId")]
    #[allow(dead_code)]
    resource_id: ResourceId,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemContentDetails {
    #[serde(rename = "videoId")]
    video_id: String,
}

// Video API response
#[derive(Debug, Deserialize)]
struct Video {
    id: String,
    snippet: VideoSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<VideoContentDetails>,
    statistics: Option<VideoStatistics>,
}

#[derive(Debug, Deserialize)]
struct VideoSnippet {
    title: String,
    description: Option<String>,
    #[serde(rename = "channelTitle")]
    channel_title: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
struct VideoContentDetails {
    duration: Option<String>, // ISO 8601 duration format (e.g., PT15M33S)
}

#[derive(Debug, Deserialize)]
struct VideoStatistics {
    #[serde(rename = "viewCount")]
    view_count: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Thumbnails {
    default: Option<Thumbnail>,
    medium: Option<Thumbnail>,
    high: Option<Thumbnail>,
    standard: Option<Thumbnail>,
    maxres: Option<Thumbnail>,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
    #[allow(dead_code)]
    width: Option<u32>,
    #[allow(dead_code)]
    height: Option<u32>,
}

// ============================================================================
// YouTube Provider
// ============================================================================

/// YouTube provider implementation.
pub struct YouTubeProvider {
    config: Arc<YouTubeConfig>,
    client: Client,
    base_url: String,
}

impl YouTubeProvider {
    const API_BASE_URL: &'static str = "https://www.googleapis.com/youtube/v3";
    const WATCH_LATER_PLAYLIST_ID: &'static str = "WL";

    /// Create a new YouTube provider with the given configuration.
    pub fn new(config: YouTubeConfig) -> Self {
        let client = Client::builder()
            .user_agent("Scryforge/0.1.0 (YouTube Provider)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
            base_url: Self::API_BASE_URL.to_string(),
        }
    }

    /// Make an authenticated request to the YouTube API.
    async fn api_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> std::result::Result<T, YouTubeError> {
        let url = format!("{}/{}", self.base_url, endpoint);
        debug!("YouTube API request: {} with params: {:?}", url, params);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.config.access_token)
            .query(params)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse as YouTube error response
            if let Ok(error_response) = serde_json::from_str::<YouTubeErrorResponse>(&error_text)
            {
                if error_response.error.code == 401 || error_response.error.code == 403 {
                    if error_response.error.message.contains("quota") {
                        return Err(YouTubeError::QuotaExceeded);
                    }
                    return Err(YouTubeError::Auth(error_response.error.message));
                }
                return Err(YouTubeError::Api(format!(
                    "HTTP {}: {}",
                    error_response.error.code, error_response.error.message
                )));
            }

            return Err(YouTubeError::Api(format!("HTTP {}: {}", status, error_text)));
        }

        let body = response.text().await?;
        serde_json::from_str(&body).map_err(|e| {
            YouTubeError::InvalidResponse(format!("Failed to parse response: {}", e))
        })
    }

    /// Fetch subscriptions from the YouTube API.
    async fn fetch_subscriptions(
        &self,
    ) -> std::result::Result<Vec<Subscription>, YouTubeError> {
        let params = [("part", "snippet"), ("mine", "true"), ("maxResults", "50")];

        let response: YouTubeListResponse<Subscription> =
            self.api_request("subscriptions", &params).await?;

        Ok(response.items)
    }

    /// Fetch recent videos from a channel.
    async fn fetch_channel_videos(
        &self,
        channel_id: &str,
        max_results: u32,
    ) -> std::result::Result<Vec<Video>, YouTubeError> {
        // First, get the channel's uploads playlist ID
        let params = [("part", "contentDetails"), ("id", channel_id)];

        #[derive(Debug, Deserialize)]
        struct ChannelResponse {
            items: Vec<ChannelItem>,
        }

        #[derive(Debug, Deserialize)]
        struct ChannelItem {
            #[serde(rename = "contentDetails")]
            content_details: ChannelContentDetails,
        }

        #[derive(Debug, Deserialize)]
        struct ChannelContentDetails {
            #[serde(rename = "relatedPlaylists")]
            related_playlists: RelatedPlaylists,
        }

        #[derive(Debug, Deserialize)]
        struct RelatedPlaylists {
            uploads: String,
        }

        let response: ChannelResponse = self.api_request("channels", &params).await?;

        let uploads_playlist_id = response
            .items
            .first()
            .map(|item| item.content_details.related_playlists.uploads.clone())
            .ok_or_else(|| YouTubeError::ChannelNotFound(channel_id.to_string()))?;

        // Fetch videos from the uploads playlist
        self.fetch_playlist_videos(&uploads_playlist_id, max_results)
            .await
    }

    /// Fetch user's playlists.
    async fn fetch_playlists(&self) -> std::result::Result<Vec<Playlist>, YouTubeError> {
        let params = [
            ("part", "snippet,contentDetails"),
            ("mine", "true"),
            ("maxResults", "50"),
        ];

        let response: YouTubeListResponse<Playlist> =
            self.api_request("playlists", &params).await?;

        Ok(response.items)
    }

    /// Fetch videos from a playlist.
    async fn fetch_playlist_videos(
        &self,
        playlist_id: &str,
        max_results: u32,
    ) -> std::result::Result<Vec<Video>, YouTubeError> {
        let max_results_str = max_results.to_string();
        let params = [
            ("part", "snippet,contentDetails"),
            ("playlistId", playlist_id),
            ("maxResults", &max_results_str),
        ];

        let response: YouTubeListResponse<PlaylistItem> =
            self.api_request("playlistItems", &params).await?;

        // Extract video IDs and fetch full video details
        let video_ids: Vec<String> = response
            .items
            .iter()
            .filter_map(|item| item.content_details.as_ref().map(|cd| cd.video_id.clone()))
            .collect();

        if video_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.fetch_videos_by_ids(&video_ids).await
    }

    /// Fetch video details by IDs.
    async fn fetch_videos_by_ids(
        &self,
        video_ids: &[String],
    ) -> std::result::Result<Vec<Video>, YouTubeError> {
        if video_ids.is_empty() {
            return Ok(Vec::new());
        }

        let ids_joined = video_ids.join(",");
        let params = [
            ("part", "snippet,contentDetails,statistics"),
            ("id", &ids_joined),
        ];

        let response: YouTubeListResponse<Video> = self.api_request("videos", &params).await?;

        Ok(response.items)
    }

    /// Convert a YouTube video to an Item.
    fn video_to_item(&self, video: Video, stream_id: StreamId) -> Item {
        let item_id = ItemId::new("youtube", &video.id);

        let title = video.snippet.title;
        let description = video
            .snippet
            .description
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect::<String>();

        let duration_seconds = video
            .content_details
            .as_ref()
            .and_then(|cd| cd.duration.as_ref())
            .and_then(|d| parse_iso8601_duration(d));

        let view_count = video
            .statistics
            .as_ref()
            .and_then(|s| s.view_count.as_ref())
            .and_then(|vc| vc.parse::<u64>().ok());

        let content = ItemContent::Video {
            description: description.clone(),
            duration_seconds,
            view_count,
        };

        let author = video.snippet.channel_title.map(|name| Author {
            name,
            email: None,
            url: None,
            avatar_url: None,
        });

        let published = video
            .snippet
            .published_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let url = Some(format!("https://www.youtube.com/watch?v={}", video.id));

        let thumbnail_url = video
            .snippet
            .thumbnails
            .and_then(|t| extract_best_thumbnail(t));

        let mut metadata = HashMap::new();
        if let Some(duration) = duration_seconds {
            metadata.insert("duration_seconds".to_string(), duration.to_string());
        }
        if let Some(views) = view_count {
            metadata.insert("view_count".to_string(), views.to_string());
        }

        Item {
            id: item_id,
            stream_id,
            title,
            content,
            author,
            published,
            updated: None,
            url,
            thumbnail_url,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata,
        }
    }
}

/// Parse ISO 8601 duration format (e.g., PT15M33S) to seconds.
fn parse_iso8601_duration(duration: &str) -> Option<u32> {
    // Simple parser for YouTube's ISO 8601 duration format
    // Format: PT#H#M#S (e.g., PT1H30M15S, PT15M33S, PT45S)
    if !duration.starts_with("PT") {
        return None;
    }

    let duration = &duration[2..]; // Remove "PT"
    let mut total_seconds = 0u32;

    let mut current_num = String::new();
    for ch in duration.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else {
            if let Ok(num) = current_num.parse::<u32>() {
                match ch {
                    'H' => total_seconds += num * 3600,
                    'M' => total_seconds += num * 60,
                    'S' => total_seconds += num,
                    _ => {}
                }
            }
            current_num.clear();
        }
    }

    Some(total_seconds)
}

/// Extract the best available thumbnail URL.
fn extract_best_thumbnail(thumbnails: Thumbnails) -> Option<String> {
    thumbnails
        .maxres
        .or(thumbnails.standard)
        .or(thumbnails.high)
        .or(thumbnails.medium)
        .or(thumbnails.default)
        .map(|t| t.url)
}

// ============================================================================
// Provider Trait Implementation
// ============================================================================

#[async_trait]
impl Provider for YouTubeProvider {
    fn id(&self) -> &'static str {
        "youtube"
    }

    fn name(&self) -> &'static str {
        "YouTube"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch subscriptions to verify authentication
        match self.fetch_subscriptions().await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("Successfully authenticated with YouTube API".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => {
                warn!("YouTube health check failed: {}", e);
                Ok(ProviderHealth {
                    is_healthy: false,
                    message: Some(format!("Authentication failed: {}", e)),
                    last_sync: None,
                    error_count: 1,
                })
            }
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();
        let mut items_added = 0;
        let mut errors = Vec::new();

        info!("Syncing YouTube data");

        // Fetch subscriptions
        match self.fetch_subscriptions().await {
            Ok(subscriptions) => {
                items_added += subscriptions.len() as u32;
                debug!("Fetched {} subscriptions", subscriptions.len());
            }
            Err(e) => {
                error!("Failed to fetch subscriptions: {}", e);
                errors.push(format!("Subscriptions: {}", e));
            }
        }

        // Fetch playlists
        match self.fetch_playlists().await {
            Ok(playlists) => {
                items_added += playlists.len() as u32;
                debug!("Fetched {} playlists", playlists.len());
            }
            Err(e) => {
                error!("Failed to fetch playlists: {}", e);
                errors.push(format!("Playlists: {}", e));
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
            has_collections: true,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show video preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Watched".to_string(),
                description: "Mark video as watched".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
            Action {
                id: "save".to_string(),
                name: "Add to Watch Later".to_string(),
                description: "Add to Watch Later playlist".to_string(),
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
                    name: "Open in YouTube".to_string(),
                    description: "Open video on YouTube".to_string(),
                    kind: ActionKind::OpenInBrowser,
                    keyboard_shortcut: Some("o".to_string()),
                },
            );

            actions.push(Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy video URL to clipboard".to_string(),
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
                    info!("Opening YouTube video: {}", url);
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

// ============================================================================
// HasFeeds Trait Implementation (Subscriptions)
// ============================================================================

#[async_trait]
impl HasFeeds for YouTubeProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let subscriptions = self
            .fetch_subscriptions()
            .await
            .map_err(StreamError::from)?;

        Ok(subscriptions
            .into_iter()
            .map(|sub| Feed {
                id: FeedId(sub.snippet.resource_id.channel_id.clone()),
                name: sub.snippet.title,
                description: sub.snippet.description,
                icon: sub.snippet.thumbnails.and_then(extract_best_thumbnail),
                unread_count: None,
                total_count: None,
            })
            .collect())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let channel_id = &feed_id.0;
        let max_results = options.limit.unwrap_or(25).min(50);

        let videos = self
            .fetch_channel_videos(channel_id, max_results)
            .await
            .map_err(StreamError::from)?;

        let stream_id = StreamId::new("youtube", "subscription", channel_id);

        let mut items: Vec<Item> = videos
            .into_iter()
            .map(|video| self.video_to_item(video, stream_id.clone()))
            .collect();

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Apply since filter
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

        // Apply offset
        let offset = options.offset.unwrap_or(0) as usize;
        if offset > 0 {
            items = items.into_iter().skip(offset).collect();
        }

        Ok(items)
    }
}

// ============================================================================
// HasCollections Trait Implementation (Playlists)
// ============================================================================

#[async_trait]
impl HasCollections for YouTubeProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let playlists = self.fetch_playlists().await.map_err(StreamError::from)?;

        Ok(playlists
            .into_iter()
            .map(|playlist| Collection {
                id: CollectionId(playlist.id.clone()),
                name: playlist.snippet.title,
                description: playlist.snippet.description,
                icon: playlist.snippet.thumbnails.and_then(extract_best_thumbnail),
                item_count: playlist
                    .content_details
                    .and_then(|cd| cd.item_count)
                    .unwrap_or(0),
                is_editable: true, // User's own playlists are editable
                owner: playlist.snippet.channel_title,
            })
            .collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let playlist_id = &collection_id.0;

        let videos = self
            .fetch_playlist_videos(playlist_id, 50)
            .await
            .map_err(StreamError::from)?;

        let stream_id = StreamId::new("youtube", "playlist", playlist_id);

        Ok(videos
            .into_iter()
            .map(|video| self.video_to_item(video, stream_id.clone()))
            .collect())
    }
}

// ============================================================================
// HasSavedItems Trait Implementation (Watch Later)
// ============================================================================

#[async_trait]
impl HasSavedItems for YouTubeProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let max_results = options.limit.unwrap_or(25).min(50);

        let videos = self
            .fetch_playlist_videos(Self::WATCH_LATER_PLAYLIST_ID, max_results)
            .await
            .map_err(StreamError::from)?;

        let stream_id = StreamId::new("youtube", "saved", "watch-later");

        let mut items: Vec<Item> = videos
            .into_iter()
            .map(|video| {
                let mut item = self.video_to_item(video, stream_id.clone());
                item.is_saved = true;
                item
            })
            .collect();

        // Apply offset
        let offset = options.offset.unwrap_or(0) as usize;
        if offset > 0 {
            items = items.into_iter().skip(offset).collect();
        }

        Ok(items)
    }

    async fn is_saved(&self, _item_id: &ItemId) -> Result<bool> {
        // This would require fetching the Watch Later playlist and checking if the video is in it
        // For now, we'll return false as this is a read-only check
        Ok(false)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso8601_duration() {
        assert_eq!(parse_iso8601_duration("PT15M33S"), Some(933));
        assert_eq!(parse_iso8601_duration("PT1H30M15S"), Some(5415));
        assert_eq!(parse_iso8601_duration("PT45S"), Some(45));
        assert_eq!(parse_iso8601_duration("PT2H"), Some(7200));
        assert_eq!(parse_iso8601_duration("PT30M"), Some(1800));
        assert_eq!(parse_iso8601_duration("PT0S"), Some(0));
        assert_eq!(parse_iso8601_duration("INVALID"), None);
    }

    #[test]
    fn test_youtube_provider_creation() {
        let config = YouTubeConfig {
            access_token: "test_token".to_string(),
            api_key: None,
        };

        let provider = YouTubeProvider::new(config);

        assert_eq!(provider.id(), "youtube");
        assert_eq!(provider.name(), "YouTube");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_available_actions_with_url() {
        let config = YouTubeConfig {
            access_token: "test_token".to_string(),
            api_key: None,
        };
        let provider = YouTubeProvider::new(config);

        let item = Item {
            id: ItemId::new("youtube", "test_video_id"),
            stream_id: StreamId::new("youtube", "subscription", "test_channel"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test description".to_string(),
                duration_seconds: Some(300),
                view_count: Some(1000),
            },
            author: Some(Author {
                name: "Test Channel".to_string(),
                email: None,
                url: None,
                avatar_url: None,
            }),
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=test_video_id".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Open in YouTube, Preview, Mark as Watched, Add to Watch Later, Copy Link
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0].kind, ActionKind::OpenInBrowser);
        assert_eq!(actions[1].kind, ActionKind::Preview);
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_execute_action_open_browser() {
        let config = YouTubeConfig {
            access_token: "test_token".to_string(),
            api_key: None,
        };
        let provider = YouTubeProvider::new(config);

        let item = Item {
            id: ItemId::new("youtube", "test"),
            stream_id: StreamId::new("youtube", "subscription", "test"),
            title: "Test".to_string(),
            content: ItemContent::Video {
                description: "".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let action = Action {
            id: "open_browser".to_string(),
            name: "Open in YouTube".to_string(),
            description: "Open on YouTube".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }

    // Mock response tests would go here if we had a mock HTTP client
    // For now, these tests verify the basic structure and behavior
}

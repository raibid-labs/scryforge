//! # provider-youtube
//!
//! YouTube provider for Scryforge using YouTube Data API v3.
//!
//! This provider connects to YouTube using OAuth 2.0 authentication via the
//! Sigilforge daemon. It implements:
//!
//! - **Feeds**: Subscribed channels and their recent uploads
//! - **Collections**: User playlists
//! - **Saved Items**: Watch Later playlist and Liked Videos
//!
//! ## Authentication
//!
//! This provider requires OAuth 2.0 authentication. The token is fetched from
//! Sigilforge daemon. Ensure the daemon is running and configured with YouTube
//! credentials.
//!
//! ## API Reference
//!
//! - [YouTube Data API v3](https://developers.google.com/youtube/v3)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use scryforge_provider_core::auth::TokenFetcher;
use scryforge_provider_core::prelude::*;
use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum YouTubeError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Failed to fetch auth token: {0}")]
    AuthError(String),

    #[error("YouTube API error: {0}")]
    ApiError(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

impl From<YouTubeError> for StreamError {
    fn from(err: YouTubeError) -> Self {
        match err {
            YouTubeError::HttpError(e) => StreamError::Network(e.to_string()),
            YouTubeError::AuthError(e) => StreamError::AuthRequired(e),
            YouTubeError::ApiError(e) => StreamError::Provider(e),
            YouTubeError::ParseError(e) => StreamError::Internal(e),
        }
    }
}

// ============================================================================
// YouTube API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YouTubeResponse<T> {
    items: Vec<T>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "pageInfo")]
    page_info: Option<PageInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PageInfo {
    #[serde(rename = "totalResults")]
    total_results: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YouTubeSubscription {
    id: String,
    snippet: SubscriptionSnippet,
}

#[derive(Debug, Deserialize)]
struct SubscriptionSnippet {
    title: String,
    description: String,
    #[serde(rename = "resourceId")]
    resource_id: ResourceId,
    thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
struct ResourceId {
    #[serde(rename = "channelId")]
    channel_id: String,
}

#[derive(Debug, Deserialize)]
struct YouTubeVideo {
    id: String,
    snippet: VideoSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<ContentDetails>,
    statistics: Option<Statistics>,
}

#[derive(Debug, Deserialize)]
struct VideoSnippet {
    #[serde(rename = "publishedAt")]
    published_at: String,
    #[serde(rename = "channelId")]
    channel_id: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    title: String,
    description: String,
    thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
struct ContentDetails {
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Statistics {
    #[serde(rename = "viewCount")]
    view_count: Option<String>,
    #[serde(rename = "likeCount")]
    like_count: Option<String>,
    #[serde(rename = "commentCount")]
    comment_count: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Thumbnails {
    default: Option<Thumbnail>,
    medium: Option<Thumbnail>,
    high: Option<Thumbnail>,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
}

#[derive(Debug, Deserialize)]
struct YouTubePlaylist {
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
}

#[derive(Debug, Deserialize)]
struct PlaylistContentDetails {
    #[serde(rename = "itemCount")]
    item_count: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YouTubePlaylistItem {
    id: String,
    snippet: PlaylistItemSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<PlaylistItemContentDetails>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PlaylistItemSnippet {
    #[serde(rename = "publishedAt")]
    published_at: String,
    #[serde(rename = "channelId")]
    channel_id: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    title: String,
    description: String,
    thumbnails: Option<Thumbnails>,
    #[serde(rename = "resourceId")]
    resource_id: ResourceId,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PlaylistItemContentDetails {
    #[serde(rename = "videoId")]
    video_id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YouTubeChannel {
    id: String,
    snippet: ChannelSnippet,
    statistics: Option<ChannelStatistics>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChannelSnippet {
    title: String,
    description: Option<String>,
    thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChannelStatistics {
    #[serde(rename = "videoCount")]
    video_count: Option<String>,
}

// ============================================================================
// YouTube Provider Implementation
// ============================================================================

/// YouTube provider that connects to YouTube Data API v3.
pub struct YouTubeProvider {
    client: Client,
    token_fetcher: Arc<dyn TokenFetcher>,
    account_name: String,
}

impl YouTubeProvider {
    const API_BASE: &'static str = "https://www.googleapis.com/youtube/v3";

    /// Create a new YouTube provider instance.
    ///
    /// # Arguments
    ///
    /// * `token_fetcher` - Token fetcher for OAuth authentication
    /// * `account_name` - Account name for token lookup (e.g., "personal")
    pub fn new(token_fetcher: Arc<dyn TokenFetcher>, account_name: String) -> Self {
        Self {
            client: Client::new(),
            token_fetcher,
            account_name,
        }
    }

    /// Fetch the OAuth access token from Sigilforge.
    async fn get_access_token(&self) -> Result<String> {
        self.token_fetcher
            .fetch_token("youtube", &self.account_name)
            .await
            .map_err(|e| YouTubeError::AuthError(e.to_string()).into())
    }

    /// Make an authenticated GET request to the YouTube API.
    async fn api_get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> std::result::Result<T, YouTubeError> {
        let token = self
            .token_fetcher
            .fetch_token("youtube", &self.account_name)
            .await
            .map_err(|e| YouTubeError::AuthError(e.to_string()))?;

        let url = format!("{}{}", Self::API_BASE, endpoint);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .query(params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(YouTubeError::ApiError(format!(
                "API returned status {}: {}",
                status, error_text
            )));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| YouTubeError::ParseError(e.to_string()))
    }

    /// Parse ISO 8601 duration (PT1H30M15S) to seconds.
    fn parse_duration(duration: &str) -> Option<u32> {
        // Simple parser for ISO 8601 duration format
        // Format: PT[hours]H[minutes]M[seconds]S
        let duration = duration.strip_prefix("PT")?;
        let mut total_seconds = 0u32;

        let mut current_num = String::new();
        for ch in duration.chars() {
            if ch.is_ascii_digit() {
                current_num.push(ch);
            } else {
                let num: u32 = current_num.parse().ok()?;
                match ch {
                    'H' => total_seconds += num * 3600,
                    'M' => total_seconds += num * 60,
                    'S' => total_seconds += num,
                    _ => return None,
                }
                current_num.clear();
            }
        }

        Some(total_seconds)
    }

    /// Parse user timestamp input to seconds.
    /// Accepts formats: "1:23:45", "5:30", "45", "1h30m", "5m30s"
    #[cfg(test)]
    fn parse_user_timestamp(input: &str) -> Option<u32> {
        let input = input.trim();

        // Try colon format first: "1:23:45" or "5:30" or "45"
        if input.contains(':') {
            let parts: Vec<&str> = input.split(':').collect();
            return match parts.len() {
                // H:M:S
                3 => {
                    let hours: u32 = parts[0].parse().ok()?;
                    let minutes: u32 = parts[1].parse().ok()?;
                    let seconds: u32 = parts[2].parse().ok()?;
                    Some(hours * 3600 + minutes * 60 + seconds)
                }
                // M:S
                2 => {
                    let minutes: u32 = parts[0].parse().ok()?;
                    let seconds: u32 = parts[1].parse().ok()?;
                    Some(minutes * 60 + seconds)
                }
                _ => None,
            };
        }

        // Try plain seconds: "45"
        if let Ok(seconds) = input.parse::<u32>() {
            return Some(seconds);
        }

        // Try h/m/s format: "1h30m15s" or "5m" or "30s"
        let mut total = 0u32;
        let mut current_num = String::new();

        for ch in input.chars() {
            if ch.is_ascii_digit() {
                current_num.push(ch);
            } else {
                if let Ok(num) = current_num.parse::<u32>() {
                    match ch.to_ascii_lowercase() {
                        'h' => total += num * 3600,
                        'm' => total += num * 60,
                        's' => total += num,
                        _ => return None,
                    }
                }
                current_num.clear();
            }
        }

        if total > 0 {
            Some(total)
        } else {
            None
        }
    }

    /// Extract video ID from a YouTube URL or item.
    fn extract_video_id(item: &Item) -> Option<String> {
        // Try extracting from URL first (more reliable)
        if let Some(url) = &item.url {
            // Handle standard format: https://www.youtube.com/watch?v=VIDEO_ID
            if let Some(start) = url.find("watch?v=") {
                let video_id = &url[start + 8..];
                if let Some(end) = video_id.find('&') {
                    return Some(video_id[..end].to_string());
                } else {
                    return Some(video_id.to_string());
                }
            }

            // Handle short format: https://youtu.be/VIDEO_ID
            if let Some(start) = url.find("youtu.be/") {
                let video_id = &url[start + 9..];
                if let Some(end) = video_id.find('?') {
                    return Some(video_id[..end].to_string());
                } else {
                    return Some(video_id.to_string());
                }
            }
        }

        // Fall back to extracting from item ID (format: "youtube:VIDEO_ID")
        if let Some(video_id) = item.id.0.strip_prefix("youtube:") {
            return Some(video_id.to_string());
        }

        None
    }

    /// Generate a short URL (youtu.be format) from a video ID.
    fn make_short_url(video_id: &str) -> String {
        format!("https://youtu.be/{}", video_id)
    }

    /// Check if yt-dlp or youtube-dl is available in PATH.
    fn find_download_tool() -> Option<&'static str> {
        use std::process::Command;

        // Try yt-dlp first (preferred)
        if Command::new("yt-dlp")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("yt-dlp");
        }

        // Fall back to youtube-dl
        if Command::new("youtube-dl")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("youtube-dl");
        }

        None
    }

    /// Generate download command for a video.
    fn generate_download_command(tool: &str, video_url: &str) -> String {
        format!("{} \"{}\"", tool, video_url)
    }


    /// Parse RFC 3339 timestamp to DateTime<Utc>.
    fn parse_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(timestamp)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Get the best thumbnail URL from a Thumbnails object.
    fn get_thumbnail_url(thumbnails: &Option<Thumbnails>) -> Option<String> {
        thumbnails.as_ref().and_then(|t| {
            t.high
                .as_ref()
                .or(t.medium.as_ref())
                .or(t.default.as_ref())
                .map(|thumb| thumb.url.clone())
        })
    }

    /// Convert a YouTube video to an Item.
    fn video_to_item(&self, video: YouTubeVideo, stream_id: StreamId) -> Item {
        let video_id = video.id.clone();
        let url = format!("https://www.youtube.com/watch?v={}", video_id);

        let duration_seconds = video
            .content_details
            .and_then(|cd| cd.duration)
            .and_then(|d| Self::parse_duration(&d));

        let statistics = video.statistics;
        let view_count = statistics
            .as_ref()
            .and_then(|s| s.view_count.as_ref())
            .and_then(|vc| vc.parse::<u64>().ok());

        let like_count = statistics
            .as_ref()
            .and_then(|s| s.like_count.as_ref())
            .and_then(|lc| lc.parse::<u64>().ok());

        let comment_count = statistics
            .as_ref()
            .and_then(|s| s.comment_count.as_ref())
            .and_then(|cc| cc.parse::<u64>().ok());

        let mut metadata = HashMap::new();
        if let Some(likes) = like_count {
            metadata.insert("like_count".to_string(), likes.to_string());
        }
        if let Some(comments) = comment_count {
            metadata.insert("comment_count".to_string(), comments.to_string());
        }

        Item {
            id: ItemId::new("youtube", &video_id),
            stream_id,
            title: video.snippet.title,
            content: ItemContent::Video {
                description: video.snippet.description,
                duration_seconds,
                view_count,
            },
            author: Some(Author {
                name: video.snippet.channel_title,
                email: None,
                url: Some(format!(
                    "https://www.youtube.com/channel/{}",
                    video.snippet.channel_id
                )),
                avatar_url: None,
            }),
            published: Self::parse_timestamp(&video.snippet.published_at),
            updated: None,
            url: Some(url),
            thumbnail_url: Self::get_thumbnail_url(&video.snippet.thumbnails),
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata,
        }
    }

    /// Fetch video details by IDs.
    async fn fetch_video_details(
        &self,
        video_ids: &[String],
    ) -> std::result::Result<Vec<YouTubeVideo>, YouTubeError> {
        if video_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids = video_ids.join(",");
        let response: YouTubeResponse<YouTubeVideo> = self
            .api_get(
                "/videos",
                &[
                    ("part", "snippet,contentDetails,statistics"),
                    ("id", &ids),
                    ("maxResults", "50"),
                ],
            )
            .await?;

        Ok(response.items)
    }

    /// Rate a video (like, dislike, or none).
    async fn rate_video(&self, video_id: &str, rating: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let url = format!("{}/videos/rate", Self::API_BASE);

        let response = self.client
            .post(&url)
            .bearer_auth(&token)
            .query(&[("id", video_id), ("rating", rating)])
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to rate video: {} - {}", status, error_text
            )));
        }

        Ok(())
    }

    /// Subscribe to a channel.
    async fn subscribe_to_channel(&self, channel_id: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let url = format!("{}/subscriptions", Self::API_BASE);

        let body = serde_json::json!({
            "snippet": {
                "resourceId": {
                    "kind": "youtube#channel",
                    "channelId": channel_id
                }
            }
        });

        let response = self.client
            .post(&url)
            .bearer_auth(&token)
            .query(&[("part", "snippet")])
            .json(&body)
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to subscribe: {} - {}", status, error_text
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Provider for YouTubeProvider {
    fn id(&self) -> &'static str {
        "youtube"
    }

    fn name(&self) -> &'static str {
        "YouTube"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch a minimal response to check connectivity
        match self.get_access_token().await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("YouTube provider is connected".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Authentication failed: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        // In a real implementation, this would sync data to a local cache
        // For now, we'll just verify we can connect
        match self.get_access_token().await {
            Ok(_) => Ok(SyncResult {
                success: true,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
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
            has_feeds: true,
            has_collections: true,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
            Action {
                id: "open".to_string(),
                name: "Open in YouTube".to_string(),
                description: "Open video in YouTube".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy video URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            },
            Action {
                id: "copy_short_link".to_string(),
                name: "Copy Short Link".to_string(),
                description: "Copy youtu.be short URL".to_string(),
                kind: ActionKind::Custom("copy_short_link".to_string()),
                keyboard_shortcut: Some("y".to_string()),
            },
            Action {
                id: "open_at_time".to_string(),
                name: "Open at Timestamp".to_string(),
                description: "Open video at a specific time (format: 1:23:45 or 5:30)".to_string(),
                kind: ActionKind::Custom("open_at_time".to_string()),
                keyboard_shortcut: Some("t".to_string()),
            },
            Action {
                id: "download".to_string(),
                name: "Download Video".to_string(),
                description: "Download video with yt-dlp".to_string(),
                kind: ActionKind::Custom("download".to_string()),
                keyboard_shortcut: Some("D".to_string()),
            },
            Action {
                id: "save".to_string(),
                name: "Add to Watch Later".to_string(),
                description: "Add video to Watch Later playlist".to_string(),
                kind: ActionKind::Save,
                keyboard_shortcut: Some("s".to_string()),
            },
            Action {
                id: "like".to_string(),
                name: "Like Video".to_string(),
                description: "Like this video".to_string(),
                kind: ActionKind::Custom("like".to_string()),
                keyboard_shortcut: Some("L".to_string()),
            },
            Action {
                id: "unlike".to_string(),
                name: "Unlike Video".to_string(),
                description: "Remove like from this video".to_string(),
                kind: ActionKind::Custom("unlike".to_string()),
                keyboard_shortcut: Some("U".to_string()),
            },
            Action {
                id: "subscribe".to_string(),
                name: "Subscribe to Channel".to_string(),
                description: "Subscribe to this video's channel".to_string(),
                kind: ActionKind::Custom("subscribe".to_string()),
                keyboard_shortcut: Some("S".to_string()),
            },
        ])
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
            ActionKind::Custom(custom_action) => match custom_action.as_str() {
                "copy_short_link" => {
                    if let Some(video_id) = Self::extract_video_id(item) {
                        let short_url = Self::make_short_url(&video_id);
                        Ok(ActionResult {
                            success: true,
                            message: Some("Short link copied to clipboard".to_string()),
                            data: Some(serde_json::json!({
                                "url": short_url,
                                "format": "short"
                            })),
                        })
                    } else {
                        Ok(ActionResult {
                            success: false,
                            message: Some("Could not extract video ID".to_string()),
                            data: None,
                        })
                    }
                }
                "open_at_time" => {
                    // This action requires user input for the timestamp
                    // The client should prompt for input and provide it in the data field
                    // For now, we'll return a result indicating input is needed
                    if let Some(video_id) = Self::extract_video_id(item) {
                        Ok(ActionResult {
                            success: true,
                            message: Some(
                                "Enter timestamp (e.g., 1:23:45, 5:30, or 45):".to_string(),
                            ),
                            data: Some(serde_json::json!({
                                "video_id": video_id,
                                "requires_input": true,
                                "input_type": "timestamp"
                            })),
                        })
                    } else {
                        Ok(ActionResult {
                            success: false,
                            message: Some("Could not extract video ID".to_string()),
                            data: None,
                        })
                    }
                }
                "download" => {
                    if let Some(url) = &item.url {
                        match Self::find_download_tool() {
                            Some(tool) => {
                                let command = Self::generate_download_command(tool, url);
                                Ok(ActionResult {
                                    success: true,
                                    message: Some(format!("Run: {}", command)),
                                    data: Some(serde_json::json!({
                                        "tool": tool,
                                        "url": url,
                                        "command": command,
                                        "action": "execute_command"
                                    })),
                                })
                            }
                            None => Ok(ActionResult {
                                success: false,
                                message: Some("yt-dlp or youtube-dl not found. Install with: pip install yt-dlp".to_string()),
                                data: Some(serde_json::json!({
                                    "install_hint": "pip install yt-dlp",
                                    "url": url
                                })),
                            }),
                        }
                    } else {
                        Ok(ActionResult {
                            success: false,
                            message: Some("No URL available for download".to_string()),
                            data: None,
                        })
                    }
                }
                "like" => {
                    let video_id = item.id.0.strip_prefix("youtube:").unwrap_or(&item.id.0);
                    match self.rate_video(video_id, "like").await {
                        Ok(()) => Ok(ActionResult {
                            success: true,
                            message: Some("Video liked!".to_string()),
                            data: None,
                        }),
                        Err(e) => Ok(ActionResult {
                            success: false,
                            message: Some(format!("Failed to like: {}", e)),
                            data: None,
                        }),
                    }
                }
                "unlike" => {
                    let video_id = item.id.0.strip_prefix("youtube:").unwrap_or(&item.id.0);
                    match self.rate_video(video_id, "none").await {
                        Ok(()) => Ok(ActionResult {
                            success: true,
                            message: Some("Like removed".to_string()),
                            data: None,
                        }),
                        Err(e) => Ok(ActionResult {
                            success: false,
                            message: Some(format!("Failed to unlike: {}", e)),
                            data: None,
                        }),
                    }
                }
                "subscribe" => {
                    // Get channel_id from item author URL or metadata
                    if let Some(ref author) = item.author {
                        if let Some(ref url) = author.url {
                            if let Some(channel_id) = url.strip_prefix("https://www.youtube.com/channel/") {
                                match self.subscribe_to_channel(channel_id).await {
                                    Ok(()) => return Ok(ActionResult {
                                        success: true,
                                        message: Some(format!("Subscribed to {}!", author.name)),
                                        data: None,
                                    }),
                                    Err(e) => return Ok(ActionResult {
                                        success: false,
                                        message: Some(format!("Failed to subscribe: {}", e)),
                                        data: None,
                                    }),
                                }
                            }
                        }
                    }
                    Ok(ActionResult {
                        success: false,
                        message: Some("Could not determine channel ID".to_string()),
                        data: None,
                    })
                }
                _ => Ok(ActionResult {
                    success: false,
                    message: Some(format!("Unknown custom action: {}", custom_action)),
                    data: None,
                }),
            },
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
impl HasFeeds for YouTubeProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let response: YouTubeResponse<YouTubeSubscription> = self
            .api_get(
                "/subscriptions",
                &[("part", "snippet"), ("mine", "true"), ("maxResults", "50")],
            )
            .await
            .map_err(StreamError::from)?;

        let feeds = response
            .items
            .into_iter()
            .map(|sub| Feed {
                id: FeedId(sub.snippet.resource_id.channel_id.clone()),
                name: sub.snippet.title,
                description: Some(sub.snippet.description),
                icon: Self::get_thumbnail_url(&sub.snippet.thumbnails),
                unread_count: None,
                total_count: None,
            })
            .collect();

        Ok(feeds)
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let channel_id = &feed_id.0;
        let stream_id = StreamId::new("youtube", "feed", channel_id);

        // Get the uploads playlist ID for the channel
        let channel_response: YouTubeResponse<YouTubeChannel> = self
            .api_get(
                "/channels",
                &[("part", "contentDetails"), ("id", channel_id)],
            )
            .await
            .map_err(StreamError::from)?;

        if channel_response.items.is_empty() {
            return Err(StreamError::StreamNotFound(format!(
                "Channel not found: {}",
                channel_id
            )));
        }

        // Get recent uploads from the channel
        // YouTube channels have an "uploads" playlist we can query
        let limit = options.limit.unwrap_or(25).min(50);
        let limit_str = limit.to_string();
        let params = vec![
            ("part", "snippet,contentDetails"),
            ("channelId", channel_id),
            ("maxResults", limit_str.as_str()),
            ("order", "date"),
            ("type", "video"),
        ];

        let search_response: YouTubeResponse<serde_json::Value> = self
            .api_get("/search", &params)
            .await
            .map_err(StreamError::from)?;

        // Extract video IDs from search results
        let video_ids: Vec<String> = search_response
            .items
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(|id| id.get("videoId"))
                    .and_then(|vid| vid.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        // Fetch full video details
        let videos = self
            .fetch_video_details(&video_ids)
            .await
            .map_err(StreamError::from)?;

        let mut items: Vec<Item> = videos
            .into_iter()
            .map(|video| self.video_to_item(video, stream_id.clone()))
            .collect();

        // Apply filters
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        items = items.into_iter().skip(offset).collect();

        Ok(items)
    }
}

#[async_trait]
impl HasCollections for YouTubeProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let response: YouTubeResponse<YouTubePlaylist> = self
            .api_get(
                "/playlists",
                &[
                    ("part", "snippet,contentDetails"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                ],
            )
            .await
            .map_err(StreamError::from)?;

        let collections = response
            .items
            .into_iter()
            .map(|playlist| Collection {
                id: CollectionId(playlist.id),
                name: playlist.snippet.title,
                description: playlist.snippet.description,
                icon: Self::get_thumbnail_url(&playlist.snippet.thumbnails),
                item_count: playlist
                    .content_details
                    .map(|cd| cd.item_count)
                    .unwrap_or(0),
                is_editable: true, // User's own playlists are editable
                owner: Some("me".to_string()),
            })
            .collect();

        Ok(collections)
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let playlist_id = &collection_id.0;
        let stream_id = StreamId::new("youtube", "playlist", playlist_id);

        let response: YouTubeResponse<YouTubePlaylistItem> = self
            .api_get(
                "/playlistItems",
                &[
                    ("part", "snippet,contentDetails"),
                    ("playlistId", playlist_id),
                    ("maxResults", "50"),
                ],
            )
            .await
            .map_err(StreamError::from)?;

        // Extract video IDs
        let video_ids: Vec<String> = response
            .items
            .iter()
            .filter_map(|item| item.content_details.as_ref())
            .map(|cd| cd.video_id.clone())
            .collect();

        // Fetch full video details
        let videos = self
            .fetch_video_details(&video_ids)
            .await
            .map_err(StreamError::from)?;

        let items = videos
            .into_iter()
            .map(|video| self.video_to_item(video, stream_id.clone()))
            .collect();

        Ok(items)
    }

    async fn add_to_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        // Extract video ID from item_id (format: "youtube:VIDEO_ID")
        let video_id = item_id
            .0
            .strip_prefix("youtube:")
            .ok_or_else(|| StreamError::Provider("Invalid item ID format".to_string()))?;

        let playlist_id = &collection_id.0;

        // Add video to playlist
        let token = self.get_access_token().await?;
        let url = format!("{}/playlistItems", Self::API_BASE);

        let body = serde_json::json!({
            "snippet": {
                "playlistId": playlist_id,
                "resourceId": {
                    "kind": "youtube#video",
                    "videoId": video_id
                }
            }
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .query(&[("part", "snippet")])
            .json(&body)
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to add to collection: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn remove_from_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        // To remove an item, we need to find the playlistItem ID first
        // This is a simplified implementation - in production, you'd cache this mapping
        let playlist_id = &collection_id.0;
        let video_id = item_id
            .0
            .strip_prefix("youtube:")
            .ok_or_else(|| StreamError::Provider("Invalid item ID format".to_string()))?;

        // Get playlist items to find the specific playlistItem ID
        let response: YouTubeResponse<YouTubePlaylistItem> = self
            .api_get(
                "/playlistItems",
                &[
                    ("part", "id,contentDetails"),
                    ("playlistId", playlist_id),
                    ("maxResults", "50"),
                ],
            )
            .await
            .map_err(StreamError::from)?;

        // Find the playlist item with matching video ID
        let playlist_item_id = response
            .items
            .iter()
            .find(|item| {
                item.content_details
                    .as_ref()
                    .map(|cd| cd.video_id == video_id)
                    .unwrap_or(false)
            })
            .map(|item| item.id.clone())
            .ok_or_else(|| StreamError::ItemNotFound("Item not found in collection".to_string()))?;

        // Delete the playlist item
        let token = self.get_access_token().await?;
        let url = format!("{}/playlistItems", Self::API_BASE);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .query(&[("id", playlist_item_id.as_str())])
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to remove from collection: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> Result<Collection> {
        let token = self.get_access_token().await?;
        let url = format!("{}/playlists", Self::API_BASE);

        let body = serde_json::json!({
            "snippet": {
                "title": name,
                "description": format!("Created by Scryforge")
            },
            "status": {
                "privacyStatus": "private"
            }
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .query(&[("part", "snippet,status,contentDetails")])
            .json(&body)
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to create collection: {} - {}",
                status, error_text
            )));
        }

        let playlist: YouTubePlaylist = response
            .json()
            .await
            .map_err(|e| StreamError::Internal(format!("Failed to parse response: {}", e)))?;

        Ok(Collection {
            id: CollectionId(playlist.id),
            name: playlist.snippet.title,
            description: playlist.snippet.description,
            icon: Self::get_thumbnail_url(&playlist.snippet.thumbnails),
            item_count: playlist
                .content_details
                .map(|cd| cd.item_count)
                .unwrap_or(0),
            is_editable: true,
            owner: Some("me".to_string()),
        })
    }
}

#[async_trait]
impl HasSavedItems for YouTubeProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let stream_id = StreamId::new("youtube", "saved", "watch-later-liked");

        // Fetch both Watch Later (WL) and Liked Videos (LL)
        // These are special playlist IDs in YouTube
        let mut all_videos = Vec::new();

        // Fetch Watch Later
        if options.category.is_none() || options.category.as_deref() == Some("watch-later") {
            if let Ok(response) = self
                .api_get::<YouTubeResponse<YouTubePlaylistItem>>(
                    "/playlistItems",
                    &[
                        ("part", "snippet,contentDetails"),
                        ("playlistId", "WL"), // Watch Later playlist
                        ("maxResults", "50"),
                    ],
                )
                .await
            {
                let video_ids: Vec<String> = response
                    .items
                    .iter()
                    .filter_map(|item| item.content_details.as_ref())
                    .map(|cd| cd.video_id.clone())
                    .collect();

                if let Ok(videos) = self.fetch_video_details(&video_ids).await {
                    all_videos.extend(videos);
                }
            }
        }

        // Fetch Liked Videos
        if options.category.is_none() || options.category.as_deref() == Some("liked") {
            if let Ok(response) = self
                .api_get::<YouTubeResponse<YouTubePlaylistItem>>(
                    "/playlistItems",
                    &[
                        ("part", "snippet,contentDetails"),
                        ("playlistId", "LL"), // Liked videos playlist
                        ("maxResults", "50"),
                    ],
                )
                .await
            {
                let video_ids: Vec<String> = response
                    .items
                    .iter()
                    .filter_map(|item| item.content_details.as_ref())
                    .map(|cd| cd.video_id.clone())
                    .collect();

                if let Ok(videos) = self.fetch_video_details(&video_ids).await {
                    all_videos.extend(videos);
                }
            }
        }

        let mut items: Vec<Item> = all_videos
            .into_iter()
            .map(|video| self.video_to_item(video, stream_id.clone()))
            .collect();

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        items = items.into_iter().skip(offset).collect();
        if let Some(limit) = limit {
            items.truncate(limit);
        }

        Ok(items)
    }

    async fn is_saved(&self, _item_id: &ItemId) -> Result<bool> {
        // This would require checking if the video exists in Watch Later or Liked Videos
        // For simplicity, we'll return false (not implemented in this version)
        Ok(false)
    }

    async fn save_item(&self, item_id: &ItemId) -> Result<()> {
        // Save to Watch Later playlist
        let video_id = item_id
            .0
            .strip_prefix("youtube:")
            .ok_or_else(|| StreamError::Provider("Invalid item ID format".to_string()))?;

        let token = self.get_access_token().await?;
        let url = format!("{}/playlistItems", Self::API_BASE);

        let body = serde_json::json!({
            "snippet": {
                "playlistId": "WL", // Watch Later playlist ID
                "resourceId": {
                    "kind": "youtube#video",
                    "videoId": video_id
                }
            }
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .query(&[("part", "snippet")])
            .json(&body)
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to save item: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn unsave_item(&self, item_id: &ItemId) -> Result<()> {
        // Remove from Watch Later playlist
        let video_id = item_id
            .0
            .strip_prefix("youtube:")
            .ok_or_else(|| StreamError::Provider("Invalid item ID format".to_string()))?;

        // Get Watch Later playlist items to find the playlistItem ID
        let response: YouTubeResponse<YouTubePlaylistItem> = self
            .api_get(
                "/playlistItems",
                &[
                    ("part", "id,contentDetails"),
                    ("playlistId", "WL"),
                    ("maxResults", "50"),
                ],
            )
            .await
            .map_err(StreamError::from)?;

        // Find the playlist item with matching video ID
        let playlist_item_id = response
            .items
            .iter()
            .find(|item| {
                item.content_details
                    .as_ref()
                    .map(|cd| cd.video_id == video_id)
                    .unwrap_or(false)
            })
            .map(|item| item.id.clone())
            .ok_or_else(|| {
                StreamError::ItemNotFound("Item not found in Watch Later".to_string())
            })?;

        // Delete the playlist item
        let token = self.get_access_token().await?;
        let url = format!("{}/playlistItems", Self::API_BASE);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .query(&[("id", playlist_item_id.as_str())])
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to unsave item: {} - {}",
                status, error_text
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::auth::MockTokenFetcher;

    fn create_test_provider() -> YouTubeProvider {
        let mock_fetcher = MockTokenFetcher::empty().with_token(
            "youtube".to_string(),
            "test".to_string(),
            "test-token".to_string(),
        );
        YouTubeProvider::new(Arc::new(mock_fetcher), "test".to_string())
    }

    #[test]
    fn test_provider_id() {
        let provider = create_test_provider();
        assert_eq!(provider.id(), "youtube");
        assert_eq!(provider.name(), "YouTube");
    }

    #[test]
    fn test_capabilities() {
        let provider = create_test_provider();
        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(YouTubeProvider::parse_duration("PT1H30M15S"), Some(5415));
        assert_eq!(YouTubeProvider::parse_duration("PT5M"), Some(300));
        assert_eq!(YouTubeProvider::parse_duration("PT45S"), Some(45));
        assert_eq!(YouTubeProvider::parse_duration("PT1H"), Some(3600));
        assert_eq!(YouTubeProvider::parse_duration("PT0S"), Some(0));
        assert_eq!(YouTubeProvider::parse_duration("invalid"), None);
    }

    #[test]
    fn test_parse_timestamp() {
        let result = YouTubeProvider::parse_timestamp("2023-12-09T10:30:00Z");
        assert!(result.is_some());

        let result = YouTubeProvider::parse_timestamp("invalid");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_available_actions() {
        let provider = create_test_provider();
        let item = Item {
            id: ItemId::new("youtube", "test-video"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test description".to_string(),
                duration_seconds: Some(300),
                view_count: Some(1000),
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

        let actions = provider.available_actions(&item).await.unwrap();
        assert_eq!(actions.len(), 9);
        assert_eq!(actions[0].kind, ActionKind::OpenInBrowser);
        assert_eq!(actions[1].kind, ActionKind::CopyLink);
        assert_eq!(
            actions[2].kind,
            ActionKind::Custom("copy_short_link".to_string())
        );
        assert_eq!(
            actions[3].kind,
            ActionKind::Custom("open_at_time".to_string())
        );
        assert_eq!(actions[4].kind, ActionKind::Custom("download".to_string()));
        assert_eq!(actions[5].kind, ActionKind::Save);
    }

    #[tokio::test]
    async fn test_execute_action_open_in_browser() {
        let provider = create_test_provider();
        let item = Item {
            id: ItemId::new("youtube", "test-video"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test".to_string(),
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
            id: "open".to_string(),
            name: "Open".to_string(),
            description: "Open video".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
        assert!(result.message.is_some());
    }

    #[test]
    fn test_video_to_item_conversion() {
        let provider = create_test_provider();
        let video = YouTubeVideo {
            id: "test-video-id".to_string(),
            snippet: VideoSnippet {
                published_at: "2023-12-09T10:30:00Z".to_string(),
                channel_id: "test-channel-id".to_string(),
                channel_title: "Test Channel".to_string(),
                title: "Test Video".to_string(),
                description: "Test description".to_string(),
                thumbnails: None,
            },
            content_details: Some(ContentDetails {
                duration: Some("PT5M30S".to_string()),
            }),
            statistics: Some(Statistics {
                view_count: Some("1000".to_string()),
                like_count: Some("50".to_string()),
                comment_count: Some("10".to_string()),
            }),
        };

        let stream_id = StreamId::new("youtube", "feed", "test");
        let item = provider.video_to_item(video, stream_id.clone());

        assert_eq!(item.id.0, "youtube:test-video-id");
        assert_eq!(item.title, "Test Video");
        assert_eq!(item.stream_id, stream_id);
        assert!(item.url.is_some());
        assert_eq!(
            item.url.unwrap(),
            "https://www.youtube.com/watch?v=test-video-id"
        );

        match item.content {
            ItemContent::Video {
                description,
                duration_seconds,
                view_count,
            } => {
                assert_eq!(description, "Test description");
                assert_eq!(duration_seconds, Some(330)); // 5 minutes 30 seconds
                assert_eq!(view_count, Some(1000));
            }
            _ => panic!("Expected Video content"),
        }

        // Verify like_count and comment_count are in metadata
        assert_eq!(item.metadata.get("like_count"), Some(&"50".to_string()));
        assert_eq!(item.metadata.get("comment_count"), Some(&"10".to_string()));
    }

    #[tokio::test]
    async fn test_health_check_with_mock() {
        let provider = create_test_provider();
        // Mock fetcher will return a test token
        let health = provider.health_check().await.unwrap();
        assert!(health.is_healthy);
    }

    #[test]
    fn test_parse_user_timestamp() {
        // Colon format: H:M:S
        assert_eq!(YouTubeProvider::parse_user_timestamp("1:23:45"), Some(5025));
        assert_eq!(YouTubeProvider::parse_user_timestamp("0:05:30"), Some(330));

        // Colon format: M:S
        assert_eq!(YouTubeProvider::parse_user_timestamp("5:30"), Some(330));
        assert_eq!(YouTubeProvider::parse_user_timestamp("0:45"), Some(45));

        // Plain seconds
        assert_eq!(YouTubeProvider::parse_user_timestamp("45"), Some(45));
        assert_eq!(YouTubeProvider::parse_user_timestamp("120"), Some(120));

        // h/m/s format
        assert_eq!(
            YouTubeProvider::parse_user_timestamp("1h30m15s"),
            Some(5415)
        );
        assert_eq!(YouTubeProvider::parse_user_timestamp("5m"), Some(300));
        assert_eq!(YouTubeProvider::parse_user_timestamp("45s"), Some(45));
        assert_eq!(YouTubeProvider::parse_user_timestamp("2h"), Some(7200));
        assert_eq!(YouTubeProvider::parse_user_timestamp("1h5m"), Some(3900));

        // Whitespace handling
        assert_eq!(YouTubeProvider::parse_user_timestamp("  5:30  "), Some(330));

        // Invalid inputs
        assert_eq!(YouTubeProvider::parse_user_timestamp("invalid"), None);
        assert_eq!(YouTubeProvider::parse_user_timestamp(""), None);
        assert_eq!(YouTubeProvider::parse_user_timestamp("1:2:3:4"), None);
    }

    #[test]
    fn test_extract_video_id() {
        // From item ID
        let item = Item {
            id: ItemId::new("youtube", "dQw4w9WgXcQ"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Video {
                description: "".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };
        assert_eq!(
            YouTubeProvider::extract_video_id(&item),
            Some("dQw4w9WgXcQ".to_string())
        );

        // From standard URL
        let item = Item {
            id: ItemId::new("youtube", "test"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Video {
                description: "".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };
        assert_eq!(
            YouTubeProvider::extract_video_id(&item),
            Some("dQw4w9WgXcQ".to_string())
        );

        // From standard URL with query params
        let item = Item {
            id: ItemId::new("youtube", "test"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Video {
                description: "".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=30".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };
        assert_eq!(
            YouTubeProvider::extract_video_id(&item),
            Some("dQw4w9WgXcQ".to_string())
        );

        // From short URL
        let item = Item {
            id: ItemId::new("youtube", "test"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Video {
                description: "".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://youtu.be/dQw4w9WgXcQ".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };
        assert_eq!(
            YouTubeProvider::extract_video_id(&item),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_make_short_url() {
        assert_eq!(
            YouTubeProvider::make_short_url("dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[tokio::test]
    async fn test_execute_action_copy_short_link() {
        let provider = create_test_provider();
        let item = Item {
            id: ItemId::new("youtube", "dQw4w9WgXcQ"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let action = Action {
            id: "copy_short_link".to_string(),
            name: "Copy Short Link".to_string(),
            description: "Copy short link".to_string(),
            kind: ActionKind::Custom("copy_short_link".to_string()),
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
        assert!(result.message.is_some());

        // Check that the URL in data is the short format
        if let Some(data) = result.data {
            assert_eq!(data["url"], "https://youtu.be/dQw4w9WgXcQ");
            assert_eq!(data["format"], "short");
        } else {
            panic!("Expected data in result");
        }
    }

    #[tokio::test]
    async fn test_execute_action_open_at_time() {
        let provider = create_test_provider();
        let item = Item {
            id: ItemId::new("youtube", "dQw4w9WgXcQ"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test".to_string(),
                duration_seconds: None,
                view_count: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let action = Action {
            id: "open_at_time".to_string(),
            name: "Open at Timestamp".to_string(),
            description: "Open at timestamp".to_string(),
            kind: ActionKind::Custom("open_at_time".to_string()),
            keyboard_shortcut: None,
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);

        // Should indicate that input is required
        if let Some(data) = result.data {
            assert_eq!(data["video_id"], "dQw4w9WgXcQ");
            assert_eq!(data["requires_input"], true);
            assert_eq!(data["input_type"], "timestamp");
        } else {
            panic!("Expected data in result");
        }
    }
    #[tokio::test]
    async fn test_like_action() {
        let provider = create_test_provider();
        let item = create_test_video_item();
        let actions = provider.available_actions(&item).await.unwrap();
        assert!(actions.iter().any(|a| a.id == "like"));
        assert!(actions.iter().any(|a| a.id == "unlike"));
        assert!(actions.iter().any(|a| a.id == "subscribe"));
    }

    fn create_test_video_item() -> Item {
        Item {
            id: ItemId::new("youtube", "test-video"),
            stream_id: StreamId::new("youtube", "feed", "test"),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Test description".to_string(),
                duration_seconds: Some(300),
                view_count: Some(1000),
            },
            author: Some(Author {
                name: "Test Channel".to_string(),
                email: None,
                url: Some("https://www.youtube.com/channel/UCtest".to_string()),
                avatar_url: None,
            }),
            published: None,
            updated: None,
            url: Some("https://www.youtube.com/watch?v=test-video".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        }
    }

}

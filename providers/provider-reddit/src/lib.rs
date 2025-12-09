//! # provider-reddit
//!
//! Reddit provider for Scryforge.
//!
//! This provider fetches and parses Reddit feeds, saved posts, and subscribed subreddits
//! via the Reddit JSON API. It supports authentication via OAuth2 access tokens.
//!
//! ## Features
//!
//! - Access Reddit home feed and popular posts
//! - Browse subscribed subreddits
//! - Retrieve saved posts
//! - Support for both self-posts and link posts
//! - Rich metadata including scores, comments, flairs

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
pub enum RedditError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Reddit API error: {0}")]
    Api(String),

    #[error("Authentication required")]
    AuthRequired,

    #[error("Rate limited")]
    RateLimited,

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Subreddit not found: {0}")]
    SubredditNotFound(String),
}

impl From<RedditError> for StreamError {
    fn from(err: RedditError) -> Self {
        match err {
            RedditError::Http(e) => StreamError::Network(e.to_string()),
            RedditError::Api(e) => StreamError::Provider(format!("Reddit API error: {e}")),
            RedditError::AuthRequired => StreamError::AuthRequired("Reddit OAuth token required".to_string()),
            RedditError::RateLimited => StreamError::RateLimited(60),
            RedditError::Parse(e) => StreamError::Provider(format!("Parse error: {e}")),
            RedditError::SubredditNotFound(e) => StreamError::StreamNotFound(e),
        }
    }
}

// ============================================================================
// Reddit API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct RedditListing {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    children: Vec<RedditChild>,
    #[allow(dead_code)]
    after: Option<String>,
    #[allow(dead_code)]
    before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditChild {
    data: RedditPost,
}

#[derive(Debug, Deserialize, Serialize)]
struct RedditPost {
    id: String,
    subreddit: String,
    subreddit_name_prefixed: Option<String>,
    title: String,
    selftext: Option<String>,
    author: String,
    url: Option<String>,
    permalink: String,
    score: i64,
    num_comments: u64,
    created_utc: f64,
    link_flair_text: Option<String>,
    is_self: bool,
    thumbnail: Option<String>,
    over_18: Option<bool>,
    stickied: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SubredditData {
    data: SubredditInfo,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SubredditInfo {
    display_name: String,
    display_name_prefixed: String,
    title: Option<String>,
    public_description: Option<String>,
    icon_img: Option<String>,
    subscribers: Option<u64>,
    url: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RedditUserInfo {
    name: String,
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the Reddit provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedditConfig {
    /// OAuth2 access token for Reddit API
    pub access_token: String,

    /// Optional custom user agent (defaults to "Scryforge/0.1.0")
    pub user_agent: Option<String>,

    /// Username for saved posts (optional, will be auto-detected if not provided)
    pub username: Option<String>,
}

// ============================================================================
// Reddit Provider
// ============================================================================

/// Reddit provider for accessing feeds, saved posts, and subreddits.
pub struct RedditProvider {
    config: Arc<RedditConfig>,
    client: Client,
}

impl RedditProvider {
    /// Create a new Reddit provider with the given configuration.
    pub fn new(config: RedditConfig) -> Self {
        let user_agent = config.user_agent.clone()
            .unwrap_or_else(|| "Scryforge/0.1.0 (Reddit Feed Reader)".to_string());

        let client = Client::builder()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    /// Make an authenticated request to the Reddit API.
    async fn api_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
    ) -> std::result::Result<T, RedditError> {
        let url = if endpoint.starts_with("http") {
            endpoint.to_string()
        } else {
            format!("https://oauth.reddit.com{}", endpoint)
        };

        debug!("Fetching Reddit API: {}", url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.config.access_token)
            .send()
            .await?;

        if response.status() == 401 {
            return Err(RedditError::AuthRequired);
        }

        if response.status() == 429 {
            return Err(RedditError::RateLimited);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(RedditError::Api(format!("HTTP {}: {}", status, error_text)));
        }

        let data = response.json::<T>().await
            .map_err(|e| RedditError::Parse(e.to_string()))?;

        Ok(data)
    }

    /// Get the authenticated user's username.
    async fn get_username(&self) -> std::result::Result<String, RedditError> {
        if let Some(username) = &self.config.username {
            return Ok(username.clone());
        }

        #[derive(Deserialize)]
        struct MeResponse {
            name: String,
        }

        let me: MeResponse = self.api_request("/api/v1/me").await?;
        Ok(me.name)
    }

    /// Convert a Reddit post to an Item.
    fn post_to_item(&self, post: RedditPost) -> Item {
        let stream_id = StreamId::new("reddit", "feed", &post.subreddit);
        let item_id = ItemId::new("reddit", &post.id);

        // Determine content type and build content
        let (content, url) = if post.is_self {
            // Self-post (text post)
            let text = post.selftext.unwrap_or_default();
            let content = if text.is_empty() {
                ItemContent::Text(post.title.clone())
            } else {
                ItemContent::Article {
                    summary: Some(post.title.clone()),
                    full_content: Some(text),
                }
            };
            let url = format!("https://reddit.com{}", post.permalink);
            (content, url)
        } else {
            // Link post
            let url = post.url.clone().unwrap_or_else(|| {
                format!("https://reddit.com{}", post.permalink)
            });
            let content = ItemContent::Generic {
                body: Some(format!("Link: {}", url)),
            };
            (content, url)
        };

        // Build author
        let author = Author {
            name: format!("u/{}", post.author),
            email: None,
            url: Some(format!("https://reddit.com/u/{}", post.author)),
            avatar_url: None,
        };

        // Convert timestamp
        let published = DateTime::from_timestamp(post.created_utc as i64, 0);

        // Extract tags from flair
        let tags = if let Some(flair) = &post.link_flair_text {
            vec![flair.clone()]
        } else {
            vec![]
        };

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("score".to_string(), post.score.to_string());
        metadata.insert("num_comments".to_string(), post.num_comments.to_string());
        metadata.insert("subreddit".to_string(), post.subreddit.clone());
        if let Some(flair) = &post.link_flair_text {
            metadata.insert("flair".to_string(), flair.clone());
        }
        if let Some(nsfw) = post.over_18 {
            metadata.insert("nsfw".to_string(), nsfw.to_string());
        }
        if let Some(stickied) = post.stickied {
            metadata.insert("stickied".to_string(), stickied.to_string());
        }

        // Use thumbnail if available and valid
        let thumbnail_url = post.thumbnail
            .filter(|t| t.starts_with("http"));

        Item {
            id: item_id,
            stream_id,
            title: post.title,
            content,
            author: Some(author),
            published,
            updated: None,
            url: Some(url),
            thumbnail_url,
            is_read: false,
            is_saved: false,
            tags,
            metadata,
        }
    }

    /// Fetch posts from a subreddit or feed.
    async fn fetch_posts(&self, path: &str) -> std::result::Result<Vec<RedditPost>, RedditError> {
        let listing: RedditListing = self.api_request(path).await?;
        Ok(listing.data.children.into_iter().map(|c| c.data).collect())
    }
}

#[async_trait]
impl Provider for RedditProvider {
    fn id(&self) -> &'static str {
        "reddit"
    }

    fn name(&self) -> &'static str {
        "Reddit"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch user info to verify authentication
        match self.get_username().await {
            Ok(_) => {
                Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some("Successfully authenticated with Reddit".to_string()),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                })
            }
            Err(e) => {
                warn!("Reddit health check failed: {}", e);
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

        info!("Syncing Reddit feeds");

        // Try to fetch home feed
        match self.fetch_posts("/").await {
            Ok(posts) => {
                items_added += posts.len() as u32;
                debug!("Fetched {} posts from Reddit home", posts.len());
            }
            Err(e) => {
                error!("Failed to fetch Reddit home: {}", e);
                errors.push(format!("Home feed: {}", e));
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
            has_saved_items: true,
            has_communities: true,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show post preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Read".to_string(),
                description: "Mark post as read".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
            Action {
                id: "save".to_string(),
                name: "Save".to_string(),
                description: "Save post to Reddit".to_string(),
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
                    description: "Open post in web browser".to_string(),
                    kind: ActionKind::OpenInBrowser,
                    keyboard_shortcut: Some("o".to_string()),
                },
            );

            actions.push(Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy post URL to clipboard".to_string(),
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
                    info!("Opening Reddit URL in browser: {}", url);
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
impl HasFeeds for RedditProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let mut feeds = vec![
            Feed {
                id: FeedId("home".to_string()),
                name: "Home".to_string(),
                description: Some("Your personalized Reddit home feed".to_string()),
                icon: Some("🏠".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("popular".to_string()),
                name: "Popular".to_string(),
                description: Some("Popular posts across Reddit".to_string()),
                icon: Some("🔥".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("all".to_string()),
                name: "All".to_string(),
                description: Some("All posts from all subreddits".to_string()),
                icon: Some("🌍".to_string()),
                unread_count: None,
                total_count: None,
            },
        ];

        // Fetch subscribed subreddits and add them as feeds
        match self.api_request::<RedditListing>("/subreddits/mine/subscriber?limit=100").await {
            Ok(listing) => {
                for child in listing.data.children {
                    let subreddit = child.data;
                    // Parse subreddit info from the listing
                    if let Ok(sub_name) = serde_json::from_value::<String>(
                        serde_json::to_value(&subreddit).unwrap().get("display_name").unwrap().clone()
                    ) {
                        feeds.push(Feed {
                            id: FeedId(sub_name.clone()),
                            name: format!("r/{}", sub_name),
                            description: None,
                            icon: Some("📋".to_string()),
                            unread_count: None,
                            total_count: None,
                        });
                    }
                }
            }
            Err(e) => {
                warn!("Failed to fetch subscribed subreddits: {}", e);
            }
        }

        Ok(feeds)
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let path = match feed_id.0.as_str() {
            "home" => "/".to_string(),
            "popular" => "/r/popular/hot".to_string(),
            "all" => "/r/all/hot".to_string(),
            subreddit => {
                // Remove r/ prefix if present
                let sub = subreddit.strip_prefix("r/").unwrap_or(subreddit);
                format!("/r/{}/hot", sub)
            }
        };

        let posts = self.fetch_posts(&path)
            .await
            .map_err(StreamError::from)?;

        let mut items: Vec<Item> = posts
            .into_iter()
            .map(|post| self.post_to_item(post))
            .collect();

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Apply since filter
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

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

#[async_trait]
impl HasSavedItems for RedditProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        // Get username first
        let username = self.get_username()
            .await
            .map_err(StreamError::from)?;

        let path = format!("/user/{}/saved", username);

        let posts = self.fetch_posts(&path)
            .await
            .map_err(StreamError::from)?;

        let mut items: Vec<Item> = posts
            .into_iter()
            .map(|post| self.post_to_item(post))
            .collect();

        // Mark all saved items as saved
        for item in &mut items {
            item.is_saved = true;
        }

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

    async fn is_saved(&self, _item_id: &ItemId) -> Result<bool> {
        // This would require additional API calls to check saved status
        // For now, return false as a placeholder
        Ok(false)
    }
}

#[async_trait]
impl HasCommunities for RedditProvider {
    async fn list_communities(&self) -> Result<Vec<Community>> {
        let listing: RedditListing = self
            .api_request("/subreddits/mine/subscriber?limit=100")
            .await
            .map_err(StreamError::from)?;

        let mut communities = Vec::new();

        for child in listing.data.children {
            // The child.data is a RedditPost but for subreddits it contains different fields
            // We need to deserialize it as a generic JSON value first
            if let Ok(value) = serde_json::to_value(&child.data) {
                if let Ok(display_name) = serde_json::from_value::<String>(
                    value.get("display_name").cloned().unwrap_or_default()
                ) {
                    let title = value.get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let description = value.get("public_description")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    let subscribers = value.get("subscribers")
                        .and_then(|v| v.as_u64());

                    let icon = value.get("icon_img")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    communities.push(Community {
                        id: CommunityId(display_name.clone()),
                        name: title.unwrap_or_else(|| format!("r/{}", display_name)),
                        description,
                        icon,
                        member_count: subscribers,
                        url: Some(format!("https://reddit.com/r/{}", display_name)),
                    });
                }
            }
        }

        Ok(communities)
    }

    async fn get_community(&self, id: &CommunityId) -> Result<Community> {
        let path = format!("/r/{}/about", id.0);

        #[derive(Deserialize)]
        struct AboutResponse {
            data: serde_json::Value,
        }

        let response: AboutResponse = self
            .api_request(&path)
            .await
            .map_err(StreamError::from)?;

        let data = response.data;

        let display_name = data.get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id.0)
            .to_string();

        let title = data.get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let description = data.get("public_description")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let subscribers = data.get("subscribers")
            .and_then(|v| v.as_u64());

        let icon = data.get("icon_img")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        Ok(Community {
            id: CommunityId(display_name.clone()),
            name: title.unwrap_or_else(|| format!("r/{}", display_name)),
            description,
            icon,
            member_count: subscribers,
            url: Some(format!("https://reddit.com/r/{}", display_name)),
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> RedditConfig {
        RedditConfig {
            access_token: "test_token_12345".to_string(),
            user_agent: Some("Scryforge Test/0.1.0".to_string()),
            username: Some("test_user".to_string()),
        }
    }

    #[test]
    fn test_reddit_provider_creation() {
        let config = create_test_config();
        let provider = RedditProvider::new(config);

        assert_eq!(provider.id(), "reddit");
        assert_eq!(provider.name(), "Reddit");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_saved_items);
        assert!(caps.has_communities);
        assert!(!caps.has_collections);
    }

    #[test]
    fn test_post_to_item_self_post() {
        let config = create_test_config();
        let provider = RedditProvider::new(config);

        let post = RedditPost {
            id: "abc123".to_string(),
            subreddit: "rust".to_string(),
            subreddit_name_prefixed: Some("r/rust".to_string()),
            title: "Learning Rust".to_string(),
            selftext: Some("This is a great language!".to_string()),
            author: "rustacean".to_string(),
            url: None,
            permalink: "/r/rust/comments/abc123/learning_rust/".to_string(),
            score: 42,
            num_comments: 10,
            created_utc: 1704067200.0, // 2024-01-01 00:00:00 UTC
            link_flair_text: Some("Question".to_string()),
            is_self: true,
            thumbnail: None,
            over_18: Some(false),
            stickied: Some(false),
        };

        let item = provider.post_to_item(post);

        assert_eq!(item.id.0, "reddit:abc123");
        assert_eq!(item.title, "Learning Rust");
        assert!(matches!(item.content, ItemContent::Article { .. }));
        assert_eq!(item.author.as_ref().unwrap().name, "u/rustacean");
        assert_eq!(item.metadata.get("score").unwrap(), "42");
        assert_eq!(item.metadata.get("num_comments").unwrap(), "10");
        assert_eq!(item.metadata.get("subreddit").unwrap(), "rust");
        assert_eq!(item.metadata.get("flair").unwrap(), "Question");
        assert_eq!(item.tags, vec!["Question"]);
    }

    #[test]
    fn test_post_to_item_link_post() {
        let config = create_test_config();
        let provider = RedditProvider::new(config);

        let post = RedditPost {
            id: "xyz789".to_string(),
            subreddit: "programming".to_string(),
            subreddit_name_prefixed: Some("r/programming".to_string()),
            title: "Interesting Article".to_string(),
            selftext: None,
            author: "coder".to_string(),
            url: Some("https://example.com/article".to_string()),
            permalink: "/r/programming/comments/xyz789/interesting_article/".to_string(),
            score: 100,
            num_comments: 25,
            created_utc: 1704153600.0, // 2024-01-02 00:00:00 UTC
            link_flair_text: None,
            is_self: false,
            thumbnail: Some("https://example.com/thumb.jpg".to_string()),
            over_18: Some(false),
            stickied: Some(false),
        };

        let item = provider.post_to_item(post);

        assert_eq!(item.id.0, "reddit:xyz789");
        assert_eq!(item.title, "Interesting Article");
        assert!(matches!(item.content, ItemContent::Generic { .. }));
        assert_eq!(item.url.as_ref().unwrap(), "https://example.com/article");
        assert_eq!(item.thumbnail_url.as_ref().unwrap(), "https://example.com/thumb.jpg");
    }

    #[tokio::test]
    async fn test_available_actions() {
        let config = create_test_config();
        let provider = RedditProvider::new(config);

        let item = Item {
            id: ItemId::new("reddit", "test"),
            stream_id: StreamId::new("reddit", "feed", "rust"),
            title: "Test Post".to_string(),
            content: ItemContent::Text("Test content".to_string()),
            author: Some(Author {
                name: "u/test".to_string(),
                email: None,
                url: Some("https://reddit.com/u/test".to_string()),
                avatar_url: None,
            }),
            published: None,
            updated: None,
            url: Some("https://reddit.com/r/rust/comments/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Open in Browser, Preview, Mark Read, Save, Copy Link
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0].kind, ActionKind::OpenInBrowser);
        assert_eq!(actions[1].kind, ActionKind::Preview);
        assert!(actions.iter().any(|a| a.kind == ActionKind::Save));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_execute_action_open_browser() {
        let config = create_test_config();
        let provider = RedditProvider::new(config);

        let item = Item {
            id: ItemId::new("reddit", "test"),
            stream_id: StreamId::new("reddit", "feed", "rust"),
            title: "Test Post".to_string(),
            content: ItemContent::Text("Test".to_string()),
            author: None,
            published: None,
            updated: None,
            url: Some("https://reddit.com/r/rust/comments/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
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
        assert!(result.message.is_some());
    }

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RedditConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.access_token, deserialized.access_token);
        assert_eq!(config.user_agent, deserialized.user_agent);
        assert_eq!(config.username, deserialized.username);
    }
}

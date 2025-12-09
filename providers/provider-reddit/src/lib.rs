//! # provider-reddit
//!
//! Reddit provider implementation for Scryforge.
//!
//! This provider connects to Reddit's OAuth API to fetch posts from subreddits,
//! retrieve saved items, and list subscribed communities.
//!
//! ## Features
//!
//! - Fetch home feed, popular feed, and subscribed subreddit feeds
//! - Retrieve saved posts and comments
//! - List subscribed subreddits
//! - OAuth authentication via Sigilforge
//!
//! ## Authentication
//!
//! This provider requires OAuth tokens from Reddit. Tokens are fetched
//! via the Sigilforge client using the service name "reddit".

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use scryforge_provider_core::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

// ============================================================================
// Reddit API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RedditListing {
    kind: String,
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RedditListingData {
    children: Vec<RedditThing>,
    after: Option<String>,
    before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditThing {
    kind: String,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RedditPost {
    id: String,
    name: String,
    title: String,
    selftext: Option<String>,
    selftext_html: Option<String>,
    author: String,
    subreddit: String,
    subreddit_name_prefixed: String,
    created_utc: f64,
    url: Option<String>,
    permalink: String,
    thumbnail: Option<String>,
    is_self: bool,
    score: i32,
    num_comments: i32,
    saved: Option<bool>,
    over_18: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RedditSubreddit {
    id: String,
    name: String,
    display_name: String,
    display_name_prefixed: String,
    title: String,
    public_description: Option<String>,
    icon_img: Option<String>,
    community_icon: Option<String>,
    subscribers: Option<i64>,
    url: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RedditErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

// ============================================================================
// Reddit Provider
// ============================================================================

/// Reddit provider for Scryforge.
///
/// Connects to Reddit's OAuth API to fetch posts, saved items, and subreddit information.
pub struct RedditProvider {
    token_fetcher: Arc<dyn auth::TokenFetcher>,
    account: String,
    client: Client,
}

impl RedditProvider {
    /// Create a new Reddit provider instance.
    ///
    /// # Arguments
    ///
    /// * `token_fetcher` - Token fetcher for OAuth authentication
    /// * `account` - Account identifier for the token (e.g., "personal", "work")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use provider_reddit::RedditProvider;
    /// use scryforge_provider_core::auth::SigilforgeClient;
    /// use std::sync::Arc;
    ///
    /// let token_fetcher = Arc::new(SigilforgeClient::with_default_path());
    /// let provider = RedditProvider::new(token_fetcher, "personal".to_string());
    /// ```
    pub fn new(token_fetcher: Arc<dyn auth::TokenFetcher>, account: String) -> Self {
        let client = Client::builder()
            .user_agent("scryforge/0.1.0")
            .build()
            .unwrap();

        Self {
            token_fetcher,
            account,
            client,
        }
    }

    /// Fetch an OAuth token for Reddit API requests.
    async fn get_token(&self) -> Result<String> {
        self.token_fetcher
            .fetch_token("reddit", &self.account)
            .await
            .map_err(|e| StreamError::AuthRequired(format!("Failed to fetch token: {}", e)))
    }

    /// Make an authenticated GET request to the Reddit API.
    async fn api_get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let token = self.get_token().await?;
        let url = format!("https://oauth.reddit.com{}", endpoint);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| StreamError::Network(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(StreamError::AuthRequired(
                "Invalid or expired token".to_string(),
            ));
        }

        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(StreamError::RateLimited(retry_after));
        }

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(StreamError::Provider(format!(
                "API error ({}): {}",
                status, error_body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| StreamError::Provider(format!("Failed to parse response: {}", e)))
    }

    /// Fetch posts from a Reddit listing endpoint.
    async fn fetch_posts(&self, endpoint: &str, limit: Option<u32>) -> Result<Vec<Item>> {
        let limit = limit.unwrap_or(25).min(100);
        let endpoint_with_params = format!("{}?limit={}", endpoint, limit);

        let response = self.api_get(&endpoint_with_params).await?;
        let listing: RedditListing = serde_json::from_value(response)
            .map_err(|e| StreamError::Provider(format!("Failed to parse listing: {}", e)))?;

        let mut items = Vec::new();

        for thing in listing.data.children {
            if thing.kind == "t3" {
                // t3 is a post
                let post: RedditPost = serde_json::from_value(thing.data)
                    .map_err(|e| StreamError::Provider(format!("Failed to parse post: {}", e)))?;
                items.push(self.post_to_item(post)?);
            }
        }

        Ok(items)
    }

    /// Convert a Reddit post to a Scryforge Item.
    fn post_to_item(&self, post: RedditPost) -> Result<Item> {
        let published = DateTime::from_timestamp(post.created_utc as i64, 0)
            .ok_or_else(|| StreamError::Provider("Invalid timestamp".to_string()))?;

        let summary = if post.is_self {
            post.selftext.clone()
        } else {
            Some(format!(
                "Link post to: {}",
                post.url.as_deref().unwrap_or("")
            ))
        };

        let full_content = if post.is_self {
            post.selftext_html.clone()
        } else {
            None
        };

        let content = ItemContent::Article {
            summary,
            full_content,
        };

        let url = if post.is_self {
            Some(format!("https://reddit.com{}", post.permalink))
        } else {
            post.url.clone()
        };

        let thumbnail_url = post.thumbnail.and_then(|t| {
            if t.starts_with("http") && !t.contains("self") && !t.contains("default") {
                Some(t)
            } else {
                None
            }
        });

        Ok(Item {
            id: ItemId::new("reddit", &post.id),
            stream_id: StreamId::new("reddit", "feed", &post.subreddit),
            title: post.title,
            content,
            author: Some(Author {
                name: post.author.clone(),
                email: None,
                url: Some(format!("https://reddit.com/u/{}", post.author)),
                avatar_url: None,
            }),
            published: Some(published),
            updated: None,
            url,
            thumbnail_url,
            is_read: false,
            is_saved: post.saved.unwrap_or(false),
            tags: vec![post.subreddit_name_prefixed.clone()],
            metadata: [
                ("score".to_string(), post.score.to_string()),
                ("num_comments".to_string(), post.num_comments.to_string()),
                ("subreddit".to_string(), post.subreddit.clone()),
                ("over_18".to_string(), post.over_18.to_string()),
            ]
            .into_iter()
            .collect(),
        })
    }

    /// Convert a Reddit subreddit to a Community.
    fn subreddit_to_community(&self, subreddit: RedditSubreddit) -> Community {
        let icon_url = subreddit
            .community_icon
            .or(subreddit.icon_img)
            .filter(|s| !s.is_empty());

        Community {
            id: CommunityId(subreddit.name.clone()),
            name: subreddit.display_name_prefixed.clone(),
            description: subreddit.public_description,
            icon: icon_url,
            member_count: subreddit.subscribers.map(|s| s as u64),
            url: Some(format!("https://reddit.com{}", subreddit.url)),
        }
    }
}

// ============================================================================
// Provider Implementation
// ============================================================================

#[async_trait]
impl Provider for RedditProvider {
    fn id(&self) -> &'static str {
        "reddit"
    }

    fn name(&self) -> &'static str {
        "Reddit"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        match self.get_token().await {
            Ok(_) => {
                // Try a simple API call to verify connectivity
                match self.api_get("/api/v1/me").await {
                    Ok(_) => Ok(ProviderHealth {
                        is_healthy: true,
                        message: Some("Connected to Reddit API".to_string()),
                        last_sync: Some(Utc::now()),
                        error_count: 0,
                    }),
                    Err(e) => Ok(ProviderHealth {
                        is_healthy: false,
                        message: Some(format!("API error: {}", e)),
                        last_sync: None,
                        error_count: 1,
                    }),
                }
            }
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Authentication error: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        // For now, sync just validates the connection
        match self.health_check().await {
            Ok(health) if health.is_healthy => Ok(SyncResult {
                success: true,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Ok(health) => Ok(SyncResult {
                success: false,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![health.message.unwrap_or_default()],
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
            has_collections: false,
            has_saved_items: true,
            has_communities: true,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "open".to_string(),
                name: "Open".to_string(),
                description: "Open in browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
        ];

        if !item.is_saved {
            actions.push(Action {
                id: "save".to_string(),
                name: "Save".to_string(),
                description: "Save post to Reddit".to_string(),
                kind: ActionKind::Save,
                keyboard_shortcut: Some("s".to_string()),
            });
        } else {
            actions.push(Action {
                id: "unsave".to_string(),
                name: "Unsave".to_string(),
                description: "Remove from saved".to_string(),
                kind: ActionKind::Unsave,
                keyboard_shortcut: Some("u".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser | ActionKind::Open => {
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
            ActionKind::Preview => Ok(ActionResult {
                success: true,
                message: Some("Preview action triggered".to_string()),
                data: None,
            }),
            ActionKind::Save => match self.save_item(&item.id).await {
                Ok(()) => Ok(ActionResult {
                    success: true,
                    message: Some("Item saved successfully".to_string()),
                    data: None,
                }),
                Err(e) => Ok(ActionResult {
                    success: false,
                    message: Some(format!("Failed to save item: {}", e)),
                    data: None,
                }),
            },
            ActionKind::Unsave => match self.unsave_item(&item.id).await {
                Ok(()) => Ok(ActionResult {
                    success: true,
                    message: Some("Item unsaved successfully".to_string()),
                    data: None,
                }),
                Err(e) => Ok(ActionResult {
                    success: false,
                    message: Some(format!("Failed to unsave item: {}", e)),
                    data: None,
                }),
            },
            _ => Ok(ActionResult {
                success: false,
                message: Some("Action not supported".to_string()),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// Capability Trait Implementations
// ============================================================================

#[async_trait]
impl HasFeeds for RedditProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        Ok(vec![
            Feed {
                id: FeedId("home".to_string()),
                name: "Home".to_string(),
                description: Some("Your personalized home feed".to_string()),
                icon: Some("🏠".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("popular".to_string()),
                name: "Popular".to_string(),
                description: Some("Popular posts from all of Reddit".to_string()),
                icon: Some("🔥".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("all".to_string()),
                name: "All".to_string(),
                description: Some("Posts from all subreddits".to_string()),
                icon: Some("🌐".to_string()),
                unread_count: None,
                total_count: None,
            },
        ])
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let endpoint = match feed_id.0.as_str() {
            "home" => "/",
            "popular" => "/r/popular",
            "all" => "/r/all",
            other => {
                // Treat as subreddit name
                if other.starts_with("r/") {
                    other
                } else {
                    return Err(StreamError::StreamNotFound(format!(
                        "Unknown feed: {}",
                        feed_id.0
                    )));
                }
            }
        };

        self.fetch_posts(endpoint, options.limit).await
    }
}

#[async_trait]
impl HasSavedItems for RedditProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let limit = options.limit.unwrap_or(25).min(100);
        let endpoint = format!("/user/{}/saved?limit={}", self.account, limit);

        self.fetch_posts(&endpoint, Some(limit)).await
    }

    async fn is_saved(&self, item_id: &ItemId) -> Result<bool> {
        // Extract the Reddit post ID from the ItemId
        let _reddit_id = item_id
            .as_str()
            .strip_prefix("reddit:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID format".to_string()))?;

        // Fetch saved items and check if this ID is present
        let saved_items = self.get_saved_items(SavedItemsOptions::default()).await?;

        Ok(saved_items
            .iter()
            .any(|item| item.id.as_str() == item_id.as_str()))
    }

    async fn save_item(&self, item_id: &ItemId) -> Result<()> {
        // Extract the Reddit post ID from the ItemId
        let reddit_id = item_id
            .as_str()
            .strip_prefix("reddit:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID format".to_string()))?;

        // Call Reddit API to save the item
        let token = self.get_token().await?;
        let url = "https://oauth.reddit.com/api/save";

        let response = self
            .client
            .post(url)
            .bearer_auth(&token)
            .form(&[("id", format!("t3_{}", reddit_id))])
            .send()
            .await
            .map_err(|e| StreamError::Network(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(StreamError::Provider(format!(
                "Failed to save item: {}",
                error_body
            )));
        }

        Ok(())
    }

    async fn unsave_item(&self, item_id: &ItemId) -> Result<()> {
        // Extract the Reddit post ID from the ItemId
        let reddit_id = item_id
            .as_str()
            .strip_prefix("reddit:")
            .ok_or_else(|| StreamError::ItemNotFound("Invalid item ID format".to_string()))?;

        // Call Reddit API to unsave the item
        let token = self.get_token().await?;
        let url = "https://oauth.reddit.com/api/unsave";

        let response = self
            .client
            .post(url)
            .bearer_auth(&token)
            .form(&[("id", format!("t3_{}", reddit_id))])
            .send()
            .await
            .map_err(|e| StreamError::Network(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(StreamError::Provider(format!(
                "Failed to unsave item: {}",
                error_body
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl HasCommunities for RedditProvider {
    async fn list_communities(&self) -> Result<Vec<Community>> {
        let response = self
            .api_get("/subreddits/mine/subscriber?limit=100")
            .await?;

        let listing: RedditListing = serde_json::from_value(response)
            .map_err(|e| StreamError::Provider(format!("Failed to parse listing: {}", e)))?;

        let mut communities = Vec::new();

        for thing in listing.data.children {
            if thing.kind == "t5" {
                // t5 is a subreddit
                let subreddit: RedditSubreddit =
                    serde_json::from_value(thing.data).map_err(|e| {
                        StreamError::Provider(format!("Failed to parse subreddit: {}", e))
                    })?;
                communities.push(self.subreddit_to_community(subreddit));
            }
        }

        Ok(communities)
    }

    async fn get_community(&self, id: &CommunityId) -> Result<Community> {
        let endpoint = format!("/r/{}/about", id.0);
        let response = self.api_get(&endpoint).await?;

        let thing: RedditThing = serde_json::from_value(response)
            .map_err(|e| StreamError::Provider(format!("Failed to parse response: {}", e)))?;

        let subreddit: RedditSubreddit = serde_json::from_value(thing.data)
            .map_err(|e| StreamError::Provider(format!("Failed to parse subreddit: {}", e)))?;

        Ok(self.subreddit_to_community(subreddit))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::auth::MockTokenFetcher;
    use std::collections::HashMap;

    fn mock_token_fetcher() -> Arc<MockTokenFetcher> {
        let mut tokens = HashMap::new();
        tokens.insert(
            ("reddit".to_string(), "test".to_string()),
            "mock_reddit_token".to_string(),
        );
        Arc::new(MockTokenFetcher::new(tokens))
    }

    #[tokio::test]
    async fn test_provider_basics() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());

        assert_eq!(provider.id(), "reddit");
        assert_eq!(provider.name(), "Reddit");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(!caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(caps.has_communities);
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());
        let feeds = provider.list_feeds().await.unwrap();

        assert_eq!(feeds.len(), 3);
        assert_eq!(feeds[0].id.0, "home");
        assert_eq!(feeds[1].id.0, "popular");
        assert_eq!(feeds[2].id.0, "all");
    }

    #[tokio::test]
    async fn test_post_to_item_conversion() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());

        let post = RedditPost {
            id: "abc123".to_string(),
            name: "t3_abc123".to_string(),
            title: "Test Post".to_string(),
            selftext: Some("This is a test post".to_string()),
            selftext_html: Some("<p>This is a test post</p>".to_string()),
            author: "test_user".to_string(),
            subreddit: "rust".to_string(),
            subreddit_name_prefixed: "r/rust".to_string(),
            created_utc: 1234567890.0,
            url: Some("https://reddit.com/r/rust/comments/abc123".to_string()),
            permalink: "/r/rust/comments/abc123/test_post/".to_string(),
            thumbnail: Some("https://example.com/thumb.jpg".to_string()),
            is_self: true,
            score: 42,
            num_comments: 10,
            saved: Some(false),
            over_18: false,
        };

        let item = provider.post_to_item(post).unwrap();

        assert_eq!(item.id.as_str(), "reddit:abc123");
        assert_eq!(item.title, "Test Post");
        assert_eq!(item.stream_id.as_str(), "reddit:feed:rust");
        assert!(!item.is_saved);
        assert_eq!(item.tags, vec!["r/rust"]);

        // Check metadata
        assert_eq!(item.metadata.get("score"), Some(&"42".to_string()));
        assert_eq!(item.metadata.get("num_comments"), Some(&"10".to_string()));
        assert_eq!(item.metadata.get("subreddit"), Some(&"rust".to_string()));
    }

    #[tokio::test]
    async fn test_available_actions() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());

        let item = Item {
            id: ItemId::new("reddit", "test"),
            stream_id: StreamId::new("reddit", "feed", "rust"),
            title: "Test".to_string(),
            content: ItemContent::Article {
                summary: Some("Test".to_string()),
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://reddit.com/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        assert!(actions.len() >= 2);
        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Preview));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Save));
    }

    #[tokio::test]
    async fn test_execute_action_open() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());

        let item = Item {
            id: ItemId::new("reddit", "test"),
            stream_id: StreamId::new("reddit", "feed", "rust"),
            title: "Test".to_string(),
            content: ItemContent::Article {
                summary: Some("Test".to_string()),
                full_content: None,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://reddit.com/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let action = Action {
            id: "open".to_string(),
            name: "Open".to_string(),
            description: "Open in browser".to_string(),
            kind: ActionKind::OpenInBrowser,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_subreddit_to_community_conversion() {
        let provider = RedditProvider::new(mock_token_fetcher(), "test".to_string());

        let subreddit = RedditSubreddit {
            id: "2qh1i".to_string(),
            name: "t5_2qh1i".to_string(),
            display_name: "rust".to_string(),
            display_name_prefixed: "r/rust".to_string(),
            title: "Rust Programming Language".to_string(),
            public_description: Some("A place for all things Rust".to_string()),
            icon_img: Some("https://example.com/icon.png".to_string()),
            community_icon: None,
            subscribers: Some(100000),
            url: "/r/rust/".to_string(),
        };

        let community = provider.subreddit_to_community(subreddit);

        assert_eq!(community.id.0, "t5_2qh1i");
        assert_eq!(community.name, "r/rust");
        assert_eq!(
            community.description,
            Some("A place for all things Rust".to_string())
        );
        assert_eq!(community.member_count, Some(100000));
    }
}

//! Typed Miniflux JSON API client.
//!
//! This module is a thin wrapper around `reqwest` that exposes only the subset
//! of the Miniflux API surface used by [`MinifluxProvider`](crate::MinifluxProvider).
//!
//! All requests authenticate via the `X-Auth-Token` header.

use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Errors
// ============================================================================

/// Errors raised by the Miniflux API client.
#[derive(Debug, Error)]
pub enum MinifluxApiError {
    /// Underlying transport error (DNS, TLS, connection refused, decode, ...).
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// 401 Unauthorized — usually a missing or revoked API token.
    #[error("Unauthorized (check API token)")]
    Unauthorized,

    /// 403 Forbidden — token lacks permission for the requested resource.
    #[error("Forbidden")]
    Forbidden,

    /// 404 Not Found — feed/entry id does not exist.
    #[error("Not found")]
    NotFound,

    /// 429 Too Many Requests — Miniflux rate-limited us.
    #[error("Rate limited")]
    RateLimited,

    /// Generic non-success response with the body text.
    #[error("Miniflux API error ({status}): {body}")]
    Status { status: u16, body: String },

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid configuration (bad URL etc.).
    #[error("Invalid config: {0}")]
    Config(String),
}

// ============================================================================
// Response types (subset)
// ============================================================================

/// Logged-in user information returned by `GET /v1/me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(default)]
    pub is_admin: bool,
}

/// Category metadata returned by `GET /v1/categories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub user_id: i64,
}

/// Subscribed feed returned by `GET /v1/feeds`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: i64,
    #[serde(default)]
    pub user_id: i64,
    pub feed_url: String,
    pub site_url: Option<String>,
    pub title: String,
    pub checked_at: Option<DateTime<Utc>>,
    pub category: Option<Category>,
    #[serde(default)]
    pub icon: Option<FeedIcon>,
}

/// Lightweight feed-icon reference embedded in a feed payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedIcon {
    pub feed_id: i64,
    pub icon_id: i64,
}

/// An enclosure attached to an entry (image/audio/video link).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enclosure {
    pub id: i64,
    pub url: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub size: i64,
}

/// Feed reference embedded in an `Entry`. Subset of the full `Feed` shape, kept
/// permissive so we degrade gracefully if Miniflux adds fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryFeed {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub site_url: Option<String>,
    #[serde(default)]
    pub feed_url: Option<String>,
    #[serde(default)]
    pub category: Option<Category>,
}

/// An article/entry. `status` is one of `"unread"`, `"read"`, or `"removed"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub user_id: i64,
    pub feed_id: i64,
    pub status: String,
    pub hash: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub comments_url: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub changed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub share_code: String,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub reading_time: i64,
    #[serde(default)]
    pub enclosures: Vec<Enclosure>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub feed: Option<EntryFeed>,
}

/// Paged entry collection returned by `GET /v1/entries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntriesResponse {
    pub total: i64,
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// Filter knobs for `GET /v1/entries`.
#[derive(Debug, Clone, Default)]
pub struct EntryFilter {
    pub status: Option<String>,
    pub feed_id: Option<i64>,
    pub starred: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order: Option<String>,
    pub direction: Option<String>,
    pub published_after: Option<i64>,
}

#[derive(Debug, Serialize)]
struct UpdateEntriesRequest<'a> {
    entry_ids: &'a [i64],
    status: &'a str,
}

// ============================================================================
// Client
// ============================================================================

/// Minimal typed Miniflux API client.
///
/// Re-uses a single `reqwest::Client` across calls so connections are pooled.
#[derive(Debug, Clone)]
pub struct MinifluxClient {
    base_url: String,
    api_token: String,
    http: Client,
}

impl MinifluxClient {
    /// Build a new client. `base_url` must be the server root; trailing slashes
    /// are normalised away.
    pub fn new(base_url: impl Into<String>, api_token: impl Into<String>) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        let http = Client::builder()
            .user_agent("scryforge-provider-miniflux/0.1.0")
            .build()
            .unwrap_or_default();
        Self {
            base_url: base,
            api_token: api_token.into(),
            http,
        }
    }

    /// Build a new client with an explicit `reqwest::Client` (mainly for tests).
    pub fn with_http(
        base_url: impl Into<String>,
        api_token: impl Into<String>,
        http: Client,
    ) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        Self {
            base_url: base,
            api_token: api_token.into(),
            http,
        }
    }

    /// Server base URL (without trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn check_status(
        response: reqwest::Response,
    ) -> std::result::Result<reqwest::Response, MinifluxApiError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        match status {
            StatusCode::UNAUTHORIZED => Err(MinifluxApiError::Unauthorized),
            StatusCode::FORBIDDEN => Err(MinifluxApiError::Forbidden),
            StatusCode::NOT_FOUND => Err(MinifluxApiError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => Err(MinifluxApiError::RateLimited),
            other => {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<unreadable body>".to_string());
                Err(MinifluxApiError::Status {
                    status: other.as_u16(),
                    body,
                })
            }
        }
    }

    async fn get_json<T>(&self, path: &str) -> std::result::Result<T, MinifluxApiError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self
            .http
            .get(self.url(path))
            .header("X-Auth-Token", &self.api_token)
            .send()
            .await?;
        let response = Self::check_status(response).await?;
        Ok(response.json::<T>().await?)
    }

    /// `GET /v1/me` — return information about the authenticated user.
    pub async fn me(&self) -> std::result::Result<User, MinifluxApiError> {
        self.get_json("/v1/me").await
    }

    /// `GET /v1/feeds` — list every feed the authenticated user subscribes to.
    pub async fn list_feeds(&self) -> std::result::Result<Vec<Feed>, MinifluxApiError> {
        self.get_json("/v1/feeds").await
    }

    /// `GET /v1/categories` — list user-defined categories.
    pub async fn list_categories(&self) -> std::result::Result<Vec<Category>, MinifluxApiError> {
        self.get_json("/v1/categories").await
    }

    /// `GET /v1/entries` with the given filters.
    pub async fn list_entries(
        &self,
        filter: &EntryFilter,
    ) -> std::result::Result<EntriesResponse, MinifluxApiError> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(status) = &filter.status {
            query.push(("status", status.clone()));
        }
        if let Some(feed_id) = filter.feed_id {
            query.push(("feed_id", feed_id.to_string()));
        }
        if let Some(starred) = filter.starred {
            query.push(("starred", starred.to_string()));
        }
        if let Some(limit) = filter.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = filter.offset {
            query.push(("offset", offset.to_string()));
        }
        if let Some(order) = &filter.order {
            query.push(("order", order.clone()));
        }
        if let Some(direction) = &filter.direction {
            query.push(("direction", direction.clone()));
        }
        if let Some(after) = filter.published_after {
            query.push(("published_after", after.to_string()));
        }

        let response = self
            .http
            .get(self.url("/v1/entries"))
            .header("X-Auth-Token", &self.api_token)
            .query(&query)
            .send()
            .await?;
        let response = Self::check_status(response).await?;
        Ok(response.json::<EntriesResponse>().await?)
    }

    /// `PUT /v1/entries` — bulk-update entry status (`"read"`, `"unread"`, or
    /// `"removed"`).
    pub async fn update_entries_status(
        &self,
        entry_ids: &[i64],
        status: &str,
    ) -> std::result::Result<(), MinifluxApiError> {
        let body = UpdateEntriesRequest { entry_ids, status };
        let response = self
            .http
            .put(self.url("/v1/entries"))
            .header("X-Auth-Token", &self.api_token)
            .json(&body)
            .send()
            .await?;
        Self::check_status(response).await?;
        Ok(())
    }

    /// `PUT /v1/entries/<id>/bookmark` — toggle the entry's `starred` state.
    pub async fn toggle_bookmark(
        &self,
        entry_id: i64,
    ) -> std::result::Result<(), MinifluxApiError> {
        let response = self
            .http
            .put(self.url(&format!("/v1/entries/{}/bookmark", entry_id)))
            .header("X-Auth-Token", &self.api_token)
            .send()
            .await?;
        Self::check_status(response).await?;
        Ok(())
    }
}

// ============================================================================
// Error mapping
// ============================================================================

impl From<MinifluxApiError> for scryforge_provider_core::StreamError {
    fn from(err: MinifluxApiError) -> Self {
        use scryforge_provider_core::StreamError;
        match err {
            MinifluxApiError::Http(e) => StreamError::Network(e.to_string()),
            MinifluxApiError::Unauthorized => {
                StreamError::AuthRequired("Miniflux API token missing or invalid".to_string())
            }
            MinifluxApiError::Forbidden => {
                StreamError::AuthRequired("Miniflux API token forbidden".to_string())
            }
            MinifluxApiError::NotFound => StreamError::Provider("Miniflux: not found".to_string()),
            MinifluxApiError::RateLimited => StreamError::RateLimited(60),
            MinifluxApiError::Status { status, body } => {
                StreamError::Provider(format!("Miniflux HTTP {status}: {body}"))
            }
            MinifluxApiError::Json(e) => {
                StreamError::Provider(format!("Miniflux JSON decode failed: {e}"))
            }
            MinifluxApiError::Config(e) => StreamError::Provider(format!("Miniflux config: {e}")),
        }
    }
}

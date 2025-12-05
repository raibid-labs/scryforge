# Scryforge Provider Model

This document describes the provider capability model and maps targeted services to their capabilities.

## Capability Traits

Providers implement one or more capability traits. Each trait represents a distinct way of accessing and organizing information.

### `HasFeeds`

**Purpose**: Access to logical feeds that produce a stream of items over time.

```rust
#[async_trait]
pub trait HasFeeds: Provider {
    /// List all available feeds for this provider
    async fn list_feeds(&self) -> Result<Vec<Feed>>;

    /// Get items from a specific feed
    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>>;
}

pub struct Feed {
    pub id: FeedId,
    pub name: String,
    pub unread_count: Option<u32>,
    pub icon: Option<String>,
}

pub struct FeedOptions {
    pub limit: Option<u32>,
    pub since: Option<DateTime<Utc>>,
    pub include_read: bool,
}
```

**Examples**:
- Email inboxes (messages arriving over time)
- RSS/Atom feeds (articles)
- Reddit home feed and subreddit feeds (posts)
- YouTube subscription feed (videos)
- Medium followed authors/publications (articles)

### `HasCollections`

**Purpose**: Access to named, ordered collections of items.

```rust
#[async_trait]
pub trait HasCollections: Provider {
    /// List all collections
    async fn list_collections(&self) -> Result<Vec<Collection>>;

    /// Get items in a collection (ordered)
    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>>;
}

pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub description: Option<String>,
    pub item_count: u32,
    pub is_editable: bool,  // Can items be added/removed?
}
```

**Examples**:
- Spotify playlists
- YouTube playlists
- Bookmark folders
- Email labels/folders (when viewed as a collection)

### `HasSavedItems`

**Purpose**: Access to saved, bookmarked, or liked items.

```rust
#[async_trait]
pub trait HasSavedItems: Provider {
    /// Get all saved items
    async fn get_saved_items(&self, options: SavedOptions) -> Result<Vec<Item>>;

    /// Check if an item is saved
    async fn is_saved(&self, item_id: &ItemId) -> Result<bool>;
}

pub struct SavedOptions {
    pub limit: Option<u32>,
    pub category: Option<String>,  // e.g., "posts", "comments" for Reddit
}
```

**Examples**:
- Reddit saved posts/comments
- YouTube Watch Later
- Spotify Liked Songs
- Medium bookmarks
- Browser bookmarks
- Pocket/Instapaper saves

### `HasCommunities`

**Purpose**: Access to subscriptions and memberships.

```rust
#[async_trait]
pub trait HasCommunities: Provider {
    /// List subscribed communities/channels
    async fn list_communities(&self) -> Result<Vec<Community>>;

    /// Get community details
    async fn get_community(&self, id: &CommunityId) -> Result<Community>;
}

pub struct Community {
    pub id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub member_count: Option<u64>,
    pub icon: Option<String>,
}
```

**Examples**:
- Reddit subreddits
- YouTube channel subscriptions
- Medium publications
- RSS feed sources (the feed itself, not items)

## Base Provider Trait

All providers implement the base `Provider` trait:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    /// Unique identifier for this provider type
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Check provider health/connectivity
    async fn health_check(&self) -> Result<ProviderHealth>;

    /// Trigger a sync operation
    async fn sync(&self) -> Result<SyncResult>;

    /// Get capabilities this provider supports
    fn capabilities(&self) -> ProviderCapabilities;
}

pub struct ProviderCapabilities {
    pub has_feeds: bool,
    pub has_collections: bool,
    pub has_saved_items: bool,
    pub has_communities: bool,
}
```

## Provider Capability Matrix

| Provider | HasFeeds | HasCollections | HasSavedItems | HasCommunities |
|----------|----------|----------------|---------------|----------------|
| Email (IMAP) | Inbox, folders | Labels/folders | - | - |
| RSS | Feed items | - | - | Feed sources |
| Spotify | - | Playlists | Liked Songs | - |
| YouTube | Subscriptions | Playlists | Watch Later | Channels |
| Reddit | Home, subreddits | - | Saved posts | Subreddits |
| MS To Do | - | Task lists | - | - |
| MS Calendar | - | Calendars | - | - |
| Bookmarks | - | Folders | All bookmarks | - |
| Medium | Following | - | Bookmarks | Publications |

## Provider Implementations

### `provider-email-imap`

**Auth**: `auth://imap/{account}` (Sigilforge stores IMAP credentials or OAuth tokens)

**Capabilities**:
- `HasFeeds`: Inbox and other mailboxes as feeds
- `HasCollections`: Email labels/folders as collections

**Item Schema**:
```rust
pub struct EmailItem {
    pub message_id: String,
    pub subject: String,
    pub from: Address,
    pub to: Vec<Address>,
    pub date: DateTime<Utc>,
    pub snippet: String,           // Plain text preview
    pub body_text: Option<String>, // Full plain text body
    pub is_read: bool,
    pub labels: Vec<String>,
}
```

**Notes**:
- Gmail via IMAP with OAuth (via Sigilforge)
- Outlook.com via IMAP with OAuth
- Generic IMAP servers with username/password
- No HTML rendering in MVP

### `provider-rss`

**Auth**: None (RSS feeds are public)

**Capabilities**:
- `HasFeeds`: Each RSS/Atom feed as a feed
- `HasCommunities`: The feed sources themselves

**Item Schema**:
```rust
pub struct RssItem {
    pub guid: String,
    pub title: String,
    pub link: String,
    pub author: Option<String>,
    pub published: DateTime<Utc>,
    pub summary: Option<String>,
    pub content: Option<String>,  // Full content if available
}
```

**Notes**:
- Support RSS 2.0 and Atom 1.0
- OPML import for feed list
- Medium articles via RSS (https://medium.com/feed/@username)
- Configurable poll intervals

### `provider-spotify`

**Auth**: `auth://spotify/{account}` (OAuth via Sigilforge)

**Capabilities**:
- `HasCollections`: Playlists
- `HasSavedItems`: Liked Songs

**Item Schema**:
```rust
pub struct SpotifyTrack {
    pub track_id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub album_art_url: Option<String>,
    pub duration_ms: u32,
    pub added_at: DateTime<Utc>,
    pub spotify_uri: String,
}
```

**Notes**:
- Requires Spotify Developer application
- Read-only for MVP (Phase 4 adds playlist editing)
- Deep link support for opening in Spotify

### `provider-youtube`

**Auth**: `auth://youtube/{account}` (OAuth via Sigilforge)

**Capabilities**:
- `HasFeeds`: Subscription feed
- `HasCollections`: Playlists
- `HasSavedItems`: Watch Later
- `HasCommunities`: Subscribed channels

**Item Schema**:
```rust
pub struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub channel_name: String,
    pub channel_id: String,
    pub published_at: DateTime<Utc>,
    pub description: String,
    pub thumbnail_url: String,
    pub duration: Duration,
    pub view_count: Option<u64>,
}
```

**Notes**:
- Uses YouTube Data API v3
- Quota management (API has daily limits)
- Thumbnail display in TUI (sixel/kitty for Phase 5)

### `provider-reddit`

**Auth**: `auth://reddit/{account}` (OAuth via Sigilforge)

**Capabilities**:
- `HasFeeds`: Home feed, individual subreddit feeds
- `HasSavedItems`: Saved posts and comments
- `HasCommunities`: Subscribed subreddits

**Item Schema**:
```rust
pub struct RedditPost {
    pub post_id: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub permalink: String,
    pub url: Option<String>,       // For link posts
    pub selftext: Option<String>,  // For text posts
    pub score: i32,
    pub num_comments: u32,
    pub created_utc: DateTime<Utc>,
    pub is_saved: bool,
}
```

**Notes**:
- Uses Reddit API with OAuth
- Respects rate limits
- Markdown rendering for selftext

### `provider-mstodo`

**Auth**: `auth://microsoft/{account}` (OAuth via Sigilforge)

**Capabilities**:
- `HasCollections`: Task lists as collections

**Item Schema**:
```rust
pub struct TodoTask {
    pub task_id: String,
    pub title: String,
    pub body: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub reminder: Option<DateTime<Utc>>,
    pub is_completed: bool,
    pub importance: Importance,
    pub list_id: String,
}
```

**Notes**:
- Microsoft Graph API
- Shared auth with Calendar
- Phase 4 adds task completion

### `provider-calendar` (MS Calendar)

**Auth**: `auth://microsoft/{account}` (shared with To Do)

**Capabilities**:
- `HasFeeds`: Upcoming events as a feed

**Item Schema**:
```rust
pub struct CalendarEvent {
    pub event_id: String,
    pub subject: String,
    pub body: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub location: Option<String>,
    pub is_all_day: bool,
    pub organizer: Option<String>,
    pub attendees: Vec<String>,
}
```

**Notes**:
- Microsoft Graph API
- Time zone handling
- Recurring event expansion

### `provider-bookmarks`

**Auth**: None (local storage)

**Capabilities**:
- `HasCollections`: Bookmark folders
- `HasSavedItems`: All bookmarks (flat view)

**Item Schema**:
```rust
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

**Notes**:
- Native SQLite/JSON storage
- Optional buku integration (reads buku's SQLite DB)
- Browser bookmark import (Chrome, Firefox)

## Adding New Providers

To add a new provider:

1. Create a new crate in `providers/provider-{name}/`
2. Implement the `Provider` trait
3. Implement relevant capability traits
4. Define the item schema for this provider
5. Add auth reference format to Sigilforge (if needed)
6. Register in daemon's provider registry
7. Add provider configuration schema to `docs/`

### Provider Crate Template

```rust
// providers/provider-example/src/lib.rs

use async_trait::async_trait;
use fusabi_streams_core::*;

pub struct ExampleProvider {
    config: ExampleConfig,
}

#[async_trait]
impl Provider for ExampleProvider {
    fn id(&self) -> &'static str { "example" }
    fn name(&self) -> &'static str { "Example Service" }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Check connectivity
    }

    async fn sync(&self) -> Result<SyncResult> {
        // Fetch new data
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: false,
            has_saved_items: true,
            has_communities: false,
        }
    }
}

#[async_trait]
impl HasFeeds for ExampleProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        // Implementation
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        // Implementation
    }
}
```

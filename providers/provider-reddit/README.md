# provider-reddit

Reddit provider for Scryforge.

## Features

- Fetches posts from Reddit via OAuth API
- Supports home feed, popular feed, and all subreddits
- Retrieves saved posts and comments
- Lists subscribed subreddits
- Save and unsave posts directly from Scryforge
- Converts Reddit posts to Scryforge items with `ItemContent::Article`
- OAuth authentication via Sigilforge
- Comprehensive error handling with rate limiting support

## Usage

### Basic Configuration

```rust
use provider_reddit::RedditProvider;
use scryforge_provider_core::auth::SigilforgeClient;
use std::sync::Arc;

// Create token fetcher for OAuth authentication
let token_fetcher = Arc::new(SigilforgeClient::with_default_path());

// Create provider with account identifier
let provider = RedditProvider::new(token_fetcher, "personal".to_string());
```

### Using the Provider

The provider implements `Provider`, `HasFeeds`, `HasSavedItems`, and `HasCommunities` traits:

```rust
use scryforge_provider_core::prelude::*;

// List all feeds
let feeds = provider.list_feeds().await?;

// Get items from home feed
let feed_id = FeedId("home".to_string());
let options = FeedOptions {
    limit: Some(25),
    include_read: true,
    ..Default::default()
};
let items = provider.get_feed_items(&feed_id, options).await?;

// Get saved items
let saved_options = SavedItemsOptions {
    limit: Some(50),
    ..Default::default()
};
let saved_items = provider.get_saved_items(saved_options).await?;

// List subscribed subreddits
let communities = provider.list_communities().await?;
```

## Authentication

This provider requires OAuth authentication with Reddit. Tokens are managed by the Sigilforge daemon and fetched automatically when needed.

The provider will:
1. Request a token from Sigilforge for the "reddit" service
2. Use the provided account identifier (e.g., "personal", "work")
3. Automatically include the token in API requests to oauth.reddit.com
4. Handle token expiration and re-authentication

## Feeds

The Reddit provider offers three built-in feeds:

| Feed ID | Name | Description |
|---------|------|-------------|
| `home` | Home | Your personalized home feed |
| `popular` | Popular | Popular posts from all of Reddit |
| `all` | All | Posts from all subreddits |

You can also fetch items from specific subreddits by using the subreddit name (e.g., `r/rust`) as the feed ID.

## Item Mapping

Reddit posts are mapped to Scryforge items as follows:

| Reddit Field | Scryforge Item |
|--------------|----------------|
| Post ID | `ItemId` (prefixed with "reddit:") |
| Title | `title` |
| Selftext | `ItemContent::Article.summary` |
| Selftext HTML | `ItemContent::Article.full_content` |
| Author | `author.name` |
| Created UTC | `published` |
| URL/Permalink | `url` |
| Thumbnail | `thumbnail_url` |
| Subreddit | `tags` (as "r/subreddit") |
| Score | `metadata["score"]` |
| Comment Count | `metadata["num_comments"]` |
| NSFW Flag | `metadata["over_18"]` |

## Available Actions

The Reddit provider supports these actions on items:

- **Open in Browser** - Opens the post in a web browser
- **Preview** - Shows post preview within the TUI
- **Save** - Saves the post to your Reddit saved items
- **Unsave** - Removes the post from your saved items

## Communities

The provider implements `HasCommunities` to list and retrieve information about subscribed subreddits:

```rust
// List all subscribed subreddits
let communities = provider.list_communities().await?;

// Get details for a specific subreddit
let community_id = CommunityId("t5_2qh1i".to_string());
let community = provider.get_community(&community_id).await?;
```

Each community includes:
- Name (display name with "r/" prefix)
- Description (public description)
- Icon URL
- Subscriber count
- URL to the subreddit

## Error Handling

The provider includes comprehensive error handling:

- `StreamError::AuthRequired` - Missing or invalid OAuth token
- `StreamError::RateLimited` - Too many requests (includes retry-after time)
- `StreamError::Network` - HTTP request failures
- `StreamError::Provider` - Reddit API errors
- `StreamError::StreamNotFound` - Invalid feed ID
- `StreamError::ItemNotFound` - Invalid item ID format

Rate limiting is handled according to Reddit's API guidelines, with retry-after headers respected.

## Testing

Run the test suite:

```bash
cargo test -p provider-reddit
```

The test suite includes:
- Provider basics and capabilities
- Feed listing
- Post to Item conversion
- Available actions
- Action execution
- Subreddit to Community conversion
- Mock token fetcher for offline testing

## Dependencies

- **scryforge-provider-core** (workspace) - Core provider traits and types
- **reqwest** (0.12) - HTTP client with rustls-tls
- **async-trait** - Async trait support
- **chrono** - Date/time handling
- **serde** - Serialization support
- **serde_json** - JSON parsing
- **tokio** - Async runtime

## Reddit API

This provider uses Reddit's OAuth API (oauth.reddit.com) which requires:

- Valid OAuth 2.0 access token
- User-Agent header (set to "scryforge/0.1.0")
- HTTPS connections with rustls-tls

API endpoints used:
- `GET /` - Home feed
- `GET /r/{subreddit}` - Subreddit posts
- `GET /r/popular` - Popular posts
- `GET /r/all` - All posts
- `GET /user/{username}/saved` - Saved items
- `GET /subreddits/mine/subscriber` - Subscribed subreddits
- `GET /r/{subreddit}/about` - Subreddit details
- `POST /api/save` - Save a post
- `POST /api/unsave` - Unsave a post
- `GET /api/v1/me` - User identity (health check)

## License

MIT OR Apache-2.0

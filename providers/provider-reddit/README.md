# provider-reddit

Reddit provider for Scryforge - access feeds, saved posts, and subscribed subreddits.

## Features

- Access Reddit home feed, popular, and all feeds
- Browse subscribed subreddits
- Retrieve saved posts
- Support for both self-posts (text) and link posts
- Rich metadata including scores, comments, flairs, and NSFW flags
- Proper thumbnail handling

## Authentication

This provider requires a Reddit OAuth2 access token. You can obtain one through Reddit's OAuth2 flow:

1. Register an application at https://www.reddit.com/prefs/apps
2. Use the OAuth2 authorization flow to obtain an access token
3. Configure the provider with your access token

## Configuration

```rust
use provider_reddit::{RedditProvider, RedditConfig};

let config = RedditConfig {
    access_token: "your_oauth_token_here".to_string(),
    user_agent: Some("MyApp/1.0.0".to_string()), // Optional
    username: Some("your_username".to_string()), // Optional, auto-detected if not provided
};

let provider = RedditProvider::new(config);
```

## Capabilities

This provider implements:
- `Provider` - Base provider functionality
- `HasFeeds` - Access to Reddit feeds and subreddits
- `HasSavedItems` - Access to saved posts
- `HasCommunities` - List and browse subscribed subreddits

## Available Feeds

- **Home**: Your personalized Reddit home feed
- **Popular**: Popular posts across Reddit
- **All**: All posts from all subreddits
- **Subreddits**: All your subscribed subreddits (e.g., r/rust, r/programming)

## Item Mapping

Reddit posts are mapped to `Item` structs with the following fields:

- **title**: Post title
- **content**:
  - Self-posts: `ItemContent::Article` with summary and full text
  - Link posts: `ItemContent::Generic` with link
- **author**: Username in format "u/username"
- **url**: Reddit post URL or external link
- **thumbnail_url**: Post thumbnail (if available)
- **tags**: Link flair (if present)
- **metadata**:
  - `score`: Post score
  - `num_comments`: Number of comments
  - `subreddit`: Subreddit name
  - `flair`: Link flair text (if present)
  - `nsfw`: NSFW flag (if present)
  - `stickied`: Stickied status (if present)

## Reddit API Endpoints Used

- `GET /` - Home feed
- `GET /r/{subreddit}/hot` - Subreddit hot posts
- `GET /r/popular/hot` - Popular posts
- `GET /r/all/hot` - All posts
- `GET /subreddits/mine/subscriber` - Subscribed subreddits
- `GET /user/{username}/saved` - Saved posts
- `GET /r/{subreddit}/about` - Subreddit details
- `GET /api/v1/me` - Current user info

## Example Usage

```rust
use provider_reddit::{RedditProvider, RedditConfig};
use fusabi_streams_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let config = RedditConfig {
        access_token: std::env::var("REDDIT_TOKEN")?,
        user_agent: None,
        username: None,
    };

    let provider = RedditProvider::new(config);

    // List all feeds
    let feeds = provider.list_feeds().await?;
    for feed in feeds {
        println!("Feed: {} ({})", feed.name, feed.id.0);
    }

    // Get home feed items
    let home_feed = FeedId("home".to_string());
    let options = FeedOptions {
        limit: Some(25),
        ..Default::default()
    };
    let items = provider.get_feed_items(&home_feed, options).await?;

    for item in items {
        println!("{} - {} ({} comments, {} score)",
            item.title,
            item.author.as_ref().map(|a| &a.name).unwrap_or(&"unknown".to_string()),
            item.metadata.get("num_comments").unwrap_or(&"0".to_string()),
            item.metadata.get("score").unwrap_or(&"0".to_string())
        );
    }

    // Get saved items
    let saved_options = SavedItemsOptions {
        limit: Some(10),
        ..Default::default()
    };
    let saved = provider.get_saved_items(saved_options).await?;
    println!("Saved items: {}", saved.len());

    Ok(())
}
```

## Rate Limiting

Reddit API has rate limits. The provider will return `StreamError::RateLimited` when rate limited, suggesting a 60-second retry delay.

## Error Handling

The provider uses `RedditError` for provider-specific errors, which are converted to `StreamError` for the common interface:

- `RedditError::AuthRequired` → `StreamError::AuthRequired`
- `RedditError::RateLimited` → `StreamError::RateLimited`
- `RedditError::Api` → `StreamError::Provider`
- `RedditError::Http` → `StreamError::Network`

## Dependencies

- `fusabi-streams-core`: Core provider traits and types
- `reqwest`: HTTP client with rustls-tls
- `serde`/`serde_json`: JSON serialization
- `async-trait`: Async trait support
- `chrono`: Date/time handling
- `thiserror`: Error handling
- `tracing`: Logging

## License

MIT OR Apache-2.0

# provider-rss

RSS/Atom feed provider for Scryforge.

## Features

- Fetches and parses RSS 2.0 and Atom feeds
- OPML import for bulk feed configuration
- Handles common edge cases:
  - Missing or invalid dates (falls back to current time)
  - Relative URLs (resolved against feed base URL)
  - HTML entities in titles and content (decoded)
  - Missing content (gracefully degrades)
- Maps feed entries to unified `Item` structs with `Article` content type
- Supports filtering by date, read status, and pagination

## Usage

### Basic Setup

```rust
use provider_rss::{RssProvider, RssConfig, RssFeedConfig};
use fusabi_streams_core::prelude::*;

#[tokio::main]
async fn main() {
    // Create configuration
    let config = RssConfig {
        feeds: vec![
            RssFeedConfig {
                id: "hn".to_string(),
                name: "Hacker News".to_string(),
                url: "https://news.ycombinator.com/rss".to_string(),
                description: Some("Tech news and discussions".to_string()),
                icon: Some("🔶".to_string()),
            },
            RssFeedConfig {
                id: "rust-blog".to_string(),
                name: "Rust Blog".to_string(),
                url: "https://blog.rust-lang.org/feed.xml".to_string(),
                description: Some("Official Rust blog".to_string()),
                icon: Some("🦀".to_string()),
            },
        ],
    };

    // Create provider
    let provider = RssProvider::new(config);

    // List feeds
    let feeds = provider.list_feeds().await.unwrap();
    for feed in feeds {
        println!("Feed: {} ({})", feed.name, feed.id.0);
    }

    // Get items from a feed
    let feed_id = FeedId("hn".to_string());
    let options = FeedOptions {
        limit: Some(10),
        include_read: true,
        ..Default::default()
    };

    let items = provider.get_feed_items(&feed_id, options).await.unwrap();
    for item in items {
        println!("- {}", item.title);
        if let Some(url) = item.url {
            println!("  {}", url);
        }
    }
}
```

### OPML Import

Import feeds from an OPML file (commonly exported from other feed readers):

```rust
use provider_rss::RssProvider;

let opml_content = std::fs::read_to_string("my_feeds.opml").unwrap();
let provider = RssProvider::from_opml(&opml_content).unwrap();

// Now you can use the provider with all imported feeds
let feeds = provider.list_feeds().await.unwrap();
println!("Imported {} feeds", feeds.len());
```

### Health Check and Sync

```rust
// Check provider health
let health = provider.health_check().await.unwrap();
if health.is_healthy {
    println!("Provider is healthy");
}

// Sync all feeds
let result = provider.sync().await.unwrap();
println!("Synced {} items", result.items_added);
if !result.errors.is_empty() {
    println!("Errors: {:?}", result.errors);
}
```

### Filtering Options

```rust
use chrono::{Utc, Duration};

// Get only unread items from the last 7 days
let options = FeedOptions {
    limit: Some(50),
    include_read: false,
    since: Some(Utc::now() - Duration::days(7)),
    ..Default::default()
};

let items = provider.get_feed_items(&feed_id, options).await.unwrap();
```

## Content Mapping

RSS/Atom entries are mapped to `Item` structs with the following content type:

```rust
ItemContent::Article {
    summary: Option<String>,    // From feed entry summary/description
    full_content: Option<String>, // From feed entry content (if available)
}
```

Additional metadata:
- **Title**: HTML entities decoded
- **Author**: First author from feed entry
- **Published**: Entry published date, falls back to updated date or current time
- **URL**: Absolute URL (relative URLs resolved against feed base URL)
- **Thumbnail**: First media thumbnail if available
- **Tags**: Categories from feed entry

## Edge Cases Handled

1. **Missing Dates**: Falls back to updated date, then current time
2. **Relative URLs**: Resolved against the feed's base URL
3. **HTML Entities**: Common entities (`&amp;`, `&lt;`, etc.) decoded in titles
4. **Missing Content**: Gracefully degrades to summary or empty content
5. **Invalid Feed**: Returns appropriate error with details
6. **Network Issues**: HTTP errors wrapped in `StreamError::Network`

## Testing

Run tests with:

```bash
cargo test -p provider-rss
```

The test suite includes:
- OPML parsing
- HTML entity decoding
- Feed parsing (RSS and Atom)
- Provider trait implementations
- Action availability and execution
- Configuration and initialization

## Dependencies

- `feed-rs` - RSS/Atom feed parsing (supports both formats)
- `reqwest` - HTTP client with rustls-tls feature
- `url` - URL parsing and resolution
- `fusabi-streams-core` - Core traits and types
- `tokio`, `async-trait`, `serde`, `chrono`, `thiserror`, `tracing`

## License

MIT OR Apache-2.0

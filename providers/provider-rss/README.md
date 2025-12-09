# provider-rss

RSS/Atom feed provider for Scryforge.

## Features

- Fetches and parses RSS 2.0 and Atom feeds
- Converts feed entries to Scryforge items with `ItemContent::Article`
- Supports OPML import for bulk feed subscription
- HTTP client with rustls-tls for secure connections
- Comprehensive error handling

## Usage

### Basic Configuration

```rust
use provider_rss::{RssProvider, RssProviderConfig};

let config = RssProviderConfig::new(vec![
    "https://example.com/feed.xml".to_string(),
    "https://blog.example.com/atom.xml".to_string(),
]);
let provider = RssProvider::new(config);
```

### OPML Import

Import feeds from an OPML file (commonly exported from feed readers):

```rust
use provider_rss::RssProviderConfig;

let config = RssProviderConfig::from_opml("/path/to/subscriptions.opml").await?;
let provider = RssProvider::new(config);
```

### Using the Provider

The provider implements both `Provider` and `HasFeeds` traits:

```rust
use scryforge_provider_core::prelude::*;

// List all feeds
let feeds = provider.list_feeds().await?;

// Get items from a specific feed
let feed_id = FeedId("rss:0".to_string());
let options = FeedOptions {
    limit: Some(20),
    include_read: false,
    ..Default::default()
};
let items = provider.get_feed_items(&feed_id, options).await?;
```

## Feed Format Support

This provider uses the [feed-rs](https://github.com/feed-rs/feed-rs) library which supports:

- **RSS 2.0** - The most common RSS format
- **Atom 1.0** - Modern syndication format
- **RSS 1.0** - RDF-based RSS format
- **RSS 0.x** - Legacy RSS formats

## Item Mapping

RSS/Atom feed entries are mapped to Scryforge items as follows:

| Feed Field | Scryforge Item |
|------------|----------------|
| Entry ID | `ItemId` |
| Title | `title` |
| Summary/Description | `ItemContent::Article.summary` |
| Content | `ItemContent::Article.full_content` |
| Author | `author` |
| Published Date | `published` |
| Updated Date | `updated` |
| Link | `url` |
| Categories | `tags` |
| Media Thumbnail | `thumbnail_url` |

## Available Actions

The RSS provider supports these actions on items:

- **Open in Browser** - Opens the article URL in a web browser
- **Preview** - Shows article preview within the TUI
- **Copy Link** - Copies the article URL to clipboard
- **Mark as Read** - Marks the article as read
- **Save Article** - Saves the article for later reading

## OPML Format

OPML (Outline Processor Markup Language) is an XML format for exchanging lists of RSS/Atom feeds. Most feed readers can export subscriptions to OPML.

Example OPML structure:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>My Subscriptions</title>
  </head>
  <body>
    <outline text="Technology" title="Technology">
      <outline text="Tech Blog" xmlUrl="https://example.com/tech/rss"/>
      <outline text="Dev Blog" xmlUrl="https://example.com/dev/feed"/>
    </outline>
    <outline text="News" xmlUrl="https://example.com/news/atom"/>
  </body>
</opml>
```

The provider recursively extracts all `xmlUrl` attributes from the outline structure.

## Error Handling

The provider includes comprehensive error handling:

- `RssError::Http` - HTTP request failures
- `RssError::Parse` - Feed parsing errors
- `RssError::Opml` - OPML parsing errors
- `RssError::Io` - File I/O errors
- `RssError::InvalidUrl` - Invalid feed URLs

All errors are automatically converted to `StreamError` for consistency with the Scryforge provider API.

## Testing

Run the test suite:

```bash
cargo test -p provider-rss
```

The test suite includes:
- RSS 2.0 feed parsing
- Atom 1.0 feed parsing
- OPML import
- Entry to Item conversion
- Action execution
- Provider capabilities

## Dependencies

- **feed-rs** (2.0) - RSS/Atom feed parsing
- **reqwest** (0.12) - HTTP client with rustls-tls
- **opml** (1.1) - OPML parsing
- **async-trait** - Async trait support
- **chrono** - Date/time handling
- **serde** - Serialization support

## License

MIT OR Apache-2.0

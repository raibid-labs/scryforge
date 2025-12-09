# provider-youtube

YouTube provider for Scryforge using the YouTube Data API v3.

## Features

This provider implements three key capabilities:

- **HasFeeds**: Access subscribed channels and their recent videos
- **HasCollections**: Access user playlists and their videos
- **HasSavedItems**: Access Watch Later playlist videos

## Authentication

This provider requires OAuth 2.0 authentication with the YouTube Data API. You need:

- An OAuth 2.0 access token with the scope: `https://www.googleapis.com/auth/youtube.readonly`

## Usage

```rust
use provider_youtube::{YouTubeProvider, YouTubeConfig};
use fusabi_streams_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create provider with OAuth token
    let config = YouTubeConfig {
        access_token: "your_oauth_token".to_string(),
        api_key: None,
    };

    let provider = YouTubeProvider::new(config);

    // List subscribed channels
    let feeds = provider.list_feeds().await?;
    for feed in feeds {
        println!("Channel: {}", feed.name);
    }

    // Get recent videos from a channel
    let feed_id = FeedId("UCxxx".to_string()); // Channel ID
    let options = FeedOptions::default();
    let videos = provider.get_feed_items(&feed_id, options).await?;

    // List playlists
    let playlists = provider.list_collections().await?;
    for playlist in playlists {
        println!("Playlist: {} ({} videos)", playlist.name, playlist.item_count);
    }

    // Get Watch Later videos
    let watch_later = provider.get_saved_items(SavedItemsOptions::default()).await?;

    Ok(())
}
```

## API Coverage

This provider uses the following YouTube Data API v3 endpoints:

- **GET /subscriptions**: List subscribed channels
- **GET /channels**: Get channel details and uploads playlist
- **GET /playlists**: List user's playlists
- **GET /playlistItems**: Get videos in a playlist (including Watch Later)
- **GET /videos**: Get detailed video information

## Data Mapping

YouTube videos are mapped to `Item` structs with the following fields:

- **id**: `youtube:{video_id}`
- **title**: Video title
- **content**: `ItemContent::Video` with:
  - `description`: Video description (truncated to 500 chars)
  - `duration_seconds`: Parsed from ISO 8601 duration
  - `view_count`: Video view count
- **author**: Channel name
- **url**: `https://www.youtube.com/watch?v={video_id}`
- **thumbnail_url**: Best available thumbnail (maxres > standard > high > medium > default)
- **published**: Video publish date
- **metadata**:
  - `duration_seconds`: Video duration in seconds
  - `view_count`: View count as string

## Rate Limits

The YouTube Data API has quota limits. This provider does not implement rate limiting internally. When quota is exhausted, API calls will return a `QuotaExceeded` error, which is mapped to `StreamError::RateLimited(3600)` (suggesting retry after 1 hour).

## Testing

Run tests with:

```bash
cargo test -p provider-youtube
```

The test suite includes:
- ISO 8601 duration parsing
- Provider creation and configuration
- Available actions for items
- Action execution

Note: Tests do not make real API calls. Integration tests with real API access would require valid OAuth credentials.

## License

Licensed under MIT OR Apache-2.0

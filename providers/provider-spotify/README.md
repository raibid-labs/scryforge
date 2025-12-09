# provider-spotify

Spotify provider for Scryforge - access your playlists and library.

## Features

- List user playlists (via `HasCollections` trait)
- Fetch tracks from playlists
- Access liked/saved songs (via `HasSavedItems` trait)
- Rich track metadata including:
  - Track name, artists, album
  - Duration
  - Album artwork URLs
  - Spotify URIs and web URLs

## Authentication

This provider requires a valid Spotify access token. Tokens should be obtained via Sigilforge (OAuth flow) and passed in the configuration.

## API Coverage

The provider uses the following Spotify Web API endpoints:

- `GET /me/playlists` - List user's playlists
- `GET /playlists/{id}/tracks` - Get tracks in a playlist
- `GET /me/tracks` - Get user's liked/saved tracks

## Usage

```rust
use provider_spotify::{SpotifyProvider, SpotifyConfig};
use fusabi_streams_core::prelude::*;

#[tokio::main]
async fn main() {
    let config = SpotifyConfig {
        access_token: "your_spotify_access_token".to_string(),
    };

    let provider = SpotifyProvider::new(config);

    // List playlists
    let playlists = provider.list_collections().await.unwrap();
    for playlist in playlists {
        println!("Playlist: {}", playlist.name);
    }

    // Get liked songs
    let options = SavedItemsOptions::default();
    let liked_songs = provider.get_saved_items(options).await.unwrap();
    for song in liked_songs {
        println!("Liked: {}", song.title);
    }
}
```

## Data Mapping

Spotify tracks are mapped to `Item` structs as follows:

- `id`: `ItemId` with format `spotify:{track_id}`
- `title`: Track name
- `content`: `ItemContent::Track` with album, duration, and artists
- `author`: Primary artist(s)
- `url`: Spotify web URL (`https://open.spotify.com/track/...`)
- `thumbnail_url`: Album artwork URL
- `published`: When the track was added (to playlist or liked songs)
- `metadata`: Additional fields including:
  - `duration_ms`: Track duration in milliseconds
  - `album`: Album name
  - `uri`: Spotify URI (`spotify:track:...`)
  - `album_art_url`: Album artwork URL

## Error Handling

The provider handles common Spotify API errors:

- 401 Unauthorized → `StreamError::AuthRequired`
- 429 Rate Limited → `StreamError::RateLimited`
- Network errors → `StreamError::Network`
- Other API errors → `StreamError::Provider`

## Testing

Run the test suite:

```bash
cargo test -p provider-spotify
```

## Future Enhancements

Phase 4 (write operations) will add:
- Create/modify playlists
- Add/remove tracks from playlists
- Like/unlike tracks
- Reorder playlist tracks

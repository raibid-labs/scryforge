//! # provider-spotify
//!
//! Spotify provider for Scryforge.
//!
//! This provider integrates with the Spotify Web API to fetch user playlists,
//! playlist tracks, and liked songs. It uses OAuth authentication via the
//! Sigilforge daemon.
//!
//! ## Features
//!
//! - Fetch user playlists (collections)
//! - Get tracks in a playlist
//! - Get user's liked/saved songs
//! - Map Spotify tracks to Scryforge items with full metadata
//!
//! ## Authentication
//!
//! This provider requires a Spotify OAuth token, which is fetched from the
//! Sigilforge daemon using the service name "spotify" and a user-provided
//! account identifier.

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use scryforge_provider_core::auth::TokenFetcher;
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Spotify API Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyUser {
    id: String,
    display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyImage {
    url: String,
    height: Option<u32>,
    width: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyPlaylist {
    id: String,
    name: String,
    description: Option<String>,
    images: Vec<SpotifyImage>,
    tracks: SpotifyPlaylistTracks,
    owner: SpotifyUser,
    public: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyPlaylistTracks {
    total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyPagingObject<T> {
    items: Vec<T>,
    total: u32,
    limit: u32,
    offset: u32,
    next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyPlaylistItem {
    track: Option<SpotifyTrack>,
    added_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifySavedTrack {
    track: SpotifyTrack,
    added_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyTrack {
    id: Option<String>,
    name: String,
    artists: Vec<SpotifyArtist>,
    album: SpotifyAlbum,
    duration_ms: u32,
    external_urls: SpotifyExternalUrls,
    uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyArtist {
    name: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyAlbum {
    name: String,
    id: Option<String>,
    images: Vec<SpotifyImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotifyExternalUrls {
    spotify: String,
}

// ============================================================================
// Spotify Provider
// ============================================================================

/// Spotify provider implementation.
///
/// Fetches playlists and tracks from the Spotify Web API using OAuth
/// authentication via the Sigilforge daemon.
pub struct SpotifyProvider {
    token_fetcher: Arc<dyn TokenFetcher>,
    account: String,
    http_client: Client,
    api_base_url: String,
}

impl SpotifyProvider {
    /// Create a new Spotify provider.
    ///
    /// # Arguments
    ///
    /// * `token_fetcher` - Token fetcher for OAuth authentication
    /// * `account` - Account identifier for Sigilforge (e.g., "personal", "work")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use provider_spotify::SpotifyProvider;
    /// use scryforge_provider_core::auth::SigilforgeClient;
    /// use std::sync::Arc;
    ///
    /// let client = SigilforgeClient::with_default_path();
    /// let provider = SpotifyProvider::new(Arc::new(client), "personal".to_string());
    /// ```
    pub fn new(token_fetcher: Arc<dyn TokenFetcher>, account: String) -> Self {
        Self {
            token_fetcher,
            account,
            http_client: Client::new(),
            api_base_url: "https://api.spotify.com/v1".to_string(),
        }
    }

    /// Fetch a fresh OAuth token for Spotify API calls.
    async fn get_token(&self) -> Result<String> {
        self.token_fetcher
            .fetch_token("spotify", &self.account)
            .await
            .map_err(|e| StreamError::AuthRequired(format!("Failed to fetch Spotify token: {}", e)))
    }

    /// Make an authenticated GET request to the Spotify API.
    async fn api_get<T>(&self, endpoint: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let token = self.get_token().await?;
        let url = format!("{}{}", self.api_base_url, endpoint);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| StreamError::Network(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if status == 429 {
                return Err(StreamError::RateLimited(60));
            }

            return Err(StreamError::Provider(format!(
                "Spotify API error ({}): {}",
                status, error_body
            )));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| StreamError::Internal(format!("Failed to parse response: {}", e)))
    }

    /// Convert a Spotify track to a Scryforge Item.
    fn track_to_item(
        &self,
        track: &SpotifyTrack,
        stream_id: StreamId,
        added_at: Option<String>,
    ) -> Item {
        let item_id = track
            .id
            .as_ref()
            .map(|id| format!("track:{}", id))
            .unwrap_or_else(|| format!("track:{}", track.uri));

        let artists: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
        let artist_names = artists.join(", ");

        let published = added_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let thumbnail_url = track.album.images.first().map(|img| img.url.clone());

        Item {
            id: ItemId::new("spotify", &item_id),
            stream_id,
            title: track.name.clone(),
            content: ItemContent::Track {
                album: Some(track.album.name.clone()),
                duration_ms: Some(track.duration_ms),
                artists,
            },
            author: Some(Author {
                name: artist_names,
                email: None,
                url: None,
                avatar_url: None,
            }),
            published,
            updated: None,
            url: Some(track.external_urls.spotify.clone()),
            thumbnail_url,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        }
    }

    /// Fetch all pages of a paginated endpoint.
    async fn fetch_all_pages<T>(&self, initial_url: &str) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de> + Clone,
    {
        let mut all_items = Vec::new();
        let mut next_url = Some(initial_url.to_string());

        while let Some(url) = next_url {
            // Extract endpoint from full URL if needed
            let endpoint = if url.starts_with(&self.api_base_url) {
                url.trim_start_matches(&self.api_base_url).to_string()
            } else {
                url
            };

            let page: SpotifyPagingObject<T> = self.api_get(&endpoint).await?;
            all_items.extend(page.items);
            next_url = page.next;
        }

        Ok(all_items)
    }
}

#[async_trait]
impl Provider for SpotifyProvider {
    fn id(&self) -> &'static str {
        "spotify"
    }

    fn name(&self) -> &'static str {
        "Spotify"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        match self.get_token().await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("Successfully authenticated with Spotify".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Authentication failed: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        // Attempt to fetch playlists as a basic sync operation
        match self.list_collections().await {
            Ok(collections) => Ok(SyncResult {
                success: true,
                items_added: collections.len() as u32,
                items_updated: 0,
                items_removed: 0,
                errors: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Ok(SyncResult {
                success: false,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![format!("Sync failed: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: false,
            has_collections: true,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
            Action {
                id: "open".to_string(),
                name: "Open in Spotify".to_string(),
                description: "Open track in Spotify app or web player".to_string(),
                kind: ActionKind::Open,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "open_browser".to_string(),
                name: "Open in Browser".to_string(),
                description: "Open track in web browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("b".to_string()),
            },
            Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy Spotify track link to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            },
        ])
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::Open | ActionKind::OpenInBrowser | ActionKind::CopyLink => {
                Ok(ActionResult {
                    success: true,
                    message: Some(format!(
                        "Action '{}' executed for item: {}",
                        action.name, item.title
                    )),
                    data: item
                        .url
                        .as_ref()
                        .map(|url| serde_json::json!({ "url": url })),
                })
            }
            _ => Ok(ActionResult {
                success: false,
                message: Some(format!("Action '{}' not supported", action.name)),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl HasCollections for SpotifyProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        // Fetch current user's playlists
        let playlists: SpotifyPagingObject<SpotifyPlaylist> =
            self.api_get("/me/playlists?limit=50").await?;

        let mut all_playlists = playlists.items;

        // Fetch remaining pages if there are more
        let mut next = playlists.next;
        while let Some(next_url) = next {
            let endpoint = next_url.trim_start_matches(&self.api_base_url);
            let page: SpotifyPagingObject<SpotifyPlaylist> = self.api_get(endpoint).await?;
            all_playlists.extend(page.items);
            next = page.next;
        }

        Ok(all_playlists
            .into_iter()
            .map(|playlist| {
                let icon = playlist.images.first().map(|img| img.url.clone());

                Collection {
                    id: CollectionId(playlist.id.clone()),
                    name: playlist.name,
                    description: playlist.description,
                    icon,
                    item_count: playlist.tracks.total,
                    is_editable: playlist.public.unwrap_or(false),
                    owner: playlist.owner.display_name.or(Some(playlist.owner.id)),
                }
            })
            .collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let stream_id = StreamId::new("spotify", "playlist", &collection_id.0);

        // Fetch playlist tracks
        let endpoint = format!("/playlists/{}/tracks?limit=50", collection_id.0);
        let playlist_items: Vec<SpotifyPlaylistItem> = self.fetch_all_pages(&endpoint).await?;

        Ok(playlist_items
            .into_iter()
            .filter_map(|item| {
                item.track
                    .map(|track| self.track_to_item(&track, stream_id.clone(), item.added_at))
            })
            .collect())
    }

    async fn add_to_collection(
        &self,
        _collection_id: &CollectionId,
        _item_id: &ItemId,
    ) -> Result<()> {
        // TODO: Implement adding tracks to playlists (Phase 4 - write operations)
        Err(StreamError::Provider(
            "Adding to collections is not yet implemented".to_string(),
        ))
    }

    async fn remove_from_collection(
        &self,
        _collection_id: &CollectionId,
        _item_id: &ItemId,
    ) -> Result<()> {
        // TODO: Implement removing tracks from playlists (Phase 4 - write operations)
        Err(StreamError::Provider(
            "Removing from collections is not yet implemented".to_string(),
        ))
    }

    async fn create_collection(&self, _name: &str) -> Result<Collection> {
        // TODO: Implement creating playlists (Phase 4 - write operations)
        Err(StreamError::Provider(
            "Creating collections is not yet implemented".to_string(),
        ))
    }
}

#[async_trait]
impl HasSavedItems for SpotifyProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let stream_id = StreamId::new("spotify", "saved", "liked-songs");

        // Build query parameters
        let limit = options.limit.unwrap_or(50).min(50);
        let offset = options.offset.unwrap_or(0);
        let endpoint = format!("/me/tracks?limit={}&offset={}", limit, offset);

        let saved_tracks: SpotifyPagingObject<SpotifySavedTrack> = self.api_get(&endpoint).await?;

        Ok(saved_tracks
            .items
            .into_iter()
            .map(|saved_track| {
                self.track_to_item(&saved_track.track, stream_id.clone(), saved_track.added_at)
            })
            .collect())
    }

    async fn is_saved(&self, item_id: &ItemId) -> Result<bool> {
        // Extract track ID from ItemId
        let track_id = item_id
            .as_str()
            .strip_prefix("spotify:track:")
            .ok_or_else(|| StreamError::ItemNotFound(item_id.as_str().to_string()))?;

        let endpoint = format!("/me/tracks/contains?ids={}", track_id);
        let is_saved: Vec<bool> = self.api_get(&endpoint).await?;

        Ok(is_saved.first().copied().unwrap_or(false))
    }

    async fn save_item(&self, _item_id: &ItemId) -> Result<()> {
        // TODO: Implement saving/liking tracks (Phase 4 - write operations)
        Err(StreamError::Provider(
            "Saving items is not yet implemented".to_string(),
        ))
    }

    async fn unsave_item(&self, _item_id: &ItemId) -> Result<()> {
        // TODO: Implement unsaving/unliking tracks (Phase 4 - write operations)
        Err(StreamError::Provider(
            "Unsaving items is not yet implemented".to_string(),
        ))
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

    fn create_mock_provider() -> SpotifyProvider {
        let mut tokens = HashMap::new();
        tokens.insert(
            ("spotify".to_string(), "test_account".to_string()),
            "test_token_123".to_string(),
        );

        let token_fetcher = Arc::new(MockTokenFetcher::new(tokens));
        SpotifyProvider::new(token_fetcher, "test_account".to_string())
    }

    #[tokio::test]
    async fn test_provider_id_and_name() {
        let provider = create_mock_provider();
        assert_eq!(provider.id(), "spotify");
        assert_eq!(provider.name(), "Spotify");
    }

    #[tokio::test]
    async fn test_capabilities() {
        let provider = create_mock_provider();
        let caps = provider.capabilities();

        assert!(!caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_get_token_success() {
        let provider = create_mock_provider();
        let token = provider.get_token().await.unwrap();
        assert_eq!(token, "test_token_123");
    }

    #[tokio::test]
    async fn test_get_token_failure() {
        let token_fetcher = Arc::new(MockTokenFetcher::empty());
        let provider = SpotifyProvider::new(token_fetcher, "missing_account".to_string());

        let result = provider.get_token().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(StreamError::AuthRequired(_))));
    }

    #[tokio::test]
    async fn test_track_to_item() {
        let provider = create_mock_provider();
        let stream_id = StreamId::new("spotify", "playlist", "test-playlist");

        let track = SpotifyTrack {
            id: Some("track123".to_string()),
            name: "Test Song".to_string(),
            artists: vec![
                SpotifyArtist {
                    name: "Artist One".to_string(),
                    id: Some("artist1".to_string()),
                },
                SpotifyArtist {
                    name: "Artist Two".to_string(),
                    id: Some("artist2".to_string()),
                },
            ],
            album: SpotifyAlbum {
                name: "Test Album".to_string(),
                id: Some("album123".to_string()),
                images: vec![SpotifyImage {
                    url: "https://example.com/album.jpg".to_string(),
                    height: Some(640),
                    width: Some(640),
                }],
            },
            duration_ms: 180000,
            external_urls: SpotifyExternalUrls {
                spotify: "https://open.spotify.com/track/track123".to_string(),
            },
            uri: "spotify:track:track123".to_string(),
        };

        let item = provider.track_to_item(&track, stream_id.clone(), None);

        assert_eq!(item.id.as_str(), "spotify:track:track123");
        assert_eq!(item.stream_id, stream_id);
        assert_eq!(item.title, "Test Song");
        assert_eq!(
            item.url,
            Some("https://open.spotify.com/track/track123".to_string())
        );
        assert_eq!(
            item.thumbnail_url,
            Some("https://example.com/album.jpg".to_string())
        );

        match item.content {
            ItemContent::Track {
                album,
                duration_ms,
                artists,
            } => {
                assert_eq!(album, Some("Test Album".to_string()));
                assert_eq!(duration_ms, Some(180000));
                assert_eq!(
                    artists,
                    vec!["Artist One".to_string(), "Artist Two".to_string()]
                );
            }
            _ => panic!("Expected Track content"),
        }

        assert!(item.author.is_some());
        let author = item.author.unwrap();
        assert_eq!(author.name, "Artist One, Artist Two");
    }

    #[tokio::test]
    async fn test_available_actions() {
        let provider = create_mock_provider();
        let item = Item {
            id: ItemId::new("spotify", "track:test"),
            stream_id: StreamId::new("spotify", "playlist", "test"),
            title: "Test Track".to_string(),
            content: ItemContent::Track {
                album: Some("Test Album".to_string()),
                duration_ms: Some(180000),
                artists: vec!["Test Artist".to_string()],
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://open.spotify.com/track/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].kind, ActionKind::Open);
        assert_eq!(actions[1].kind, ActionKind::OpenInBrowser);
        assert_eq!(actions[2].kind, ActionKind::CopyLink);
    }

    #[tokio::test]
    async fn test_execute_action() {
        let provider = create_mock_provider();
        let item = Item {
            id: ItemId::new("spotify", "track:test"),
            stream_id: StreamId::new("spotify", "playlist", "test"),
            title: "Test Track".to_string(),
            content: ItemContent::Track {
                album: Some("Test Album".to_string()),
                duration_ms: Some(180000),
                artists: vec!["Test Artist".to_string()],
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://open.spotify.com/track/test".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let action = Action {
            id: "open".to_string(),
            name: "Open in Spotify".to_string(),
            description: "Open track in Spotify".to_string(),
            kind: ActionKind::Open,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_spotify_track_deserialization() {
        let json = r#"{
            "id": "track123",
            "name": "Test Song",
            "artists": [
                {"name": "Artist One", "id": "artist1"}
            ],
            "album": {
                "name": "Test Album",
                "id": "album123",
                "images": [
                    {"url": "https://example.com/album.jpg", "height": 640, "width": 640}
                ]
            },
            "duration_ms": 180000,
            "external_urls": {
                "spotify": "https://open.spotify.com/track/track123"
            },
            "uri": "spotify:track:track123"
        }"#;

        let track: SpotifyTrack = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, Some("track123".to_string()));
        assert_eq!(track.name, "Test Song");
        assert_eq!(track.artists.len(), 1);
        assert_eq!(track.duration_ms, 180000);
    }

    #[test]
    fn test_spotify_playlist_deserialization() {
        let json = r#"{
            "id": "playlist123",
            "name": "My Playlist",
            "description": "Test playlist",
            "images": [
                {"url": "https://example.com/playlist.jpg", "height": 640, "width": 640}
            ],
            "tracks": {
                "total": 42
            },
            "owner": {
                "id": "user123",
                "display_name": "Test User"
            },
            "public": true
        }"#;

        let playlist: SpotifyPlaylist = serde_json::from_str(json).unwrap();
        assert_eq!(playlist.id, "playlist123");
        assert_eq!(playlist.name, "My Playlist");
        assert_eq!(playlist.tracks.total, 42);
        assert_eq!(playlist.public, Some(true));
    }

    #[test]
    fn test_spotify_paging_object_deserialization() {
        let json = r#"{
            "items": [],
            "total": 100,
            "limit": 50,
            "offset": 0,
            "next": "https://api.spotify.com/v1/me/playlists?offset=50&limit=50"
        }"#;

        let page: SpotifyPagingObject<SpotifyPlaylist> = serde_json::from_str(json).unwrap();
        assert_eq!(page.total, 100);
        assert_eq!(page.limit, 50);
        assert_eq!(page.offset, 0);
        assert!(page.next.is_some());
    }
}

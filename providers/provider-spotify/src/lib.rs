//! # provider-spotify
//!
//! Spotify provider for Scryforge.
//!
//! This provider accesses Spotify data via the Web API, supporting:
//! - User playlists (via HasCollections trait)
//! - Liked songs / saved tracks (via HasSavedItems trait)
//!
//! ## Authentication
//!
//! This provider requires a valid Spotify access token. The token should be
//! obtained via Sigilforge and passed in the configuration. The provider
//! includes the token in API requests and handles 401 errors gracefully.
//!
//! ## API Coverage
//!
//! - GET /me/playlists - List user's playlists
//! - GET /playlists/{id}/tracks - Get tracks in a playlist
//! - GET /me/tracks - Get user's liked/saved tracks

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusabi_streams_core::prelude::*;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, warn};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum SpotifyError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Playlist not found: {0}")]
    PlaylistNotFound(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

impl From<SpotifyError> for StreamError {
    fn from(err: SpotifyError) -> Self {
        match err {
            SpotifyError::Http(e) => StreamError::Network(e.to_string()),
            SpotifyError::AuthFailed(e) => StreamError::AuthRequired(e),
            SpotifyError::InvalidToken(e) => StreamError::AuthRequired(e),
            SpotifyError::ApiError { status, message } => {
                if status == 401 {
                    StreamError::AuthRequired(message)
                } else if status == 429 {
                    StreamError::RateLimited(60) // Default retry after 60 seconds
                } else {
                    StreamError::Provider(format!("API error {status}: {message}"))
                }
            }
            SpotifyError::PlaylistNotFound(e) => StreamError::StreamNotFound(e),
            SpotifyError::Parse(e) => StreamError::Provider(format!("Parse error: {e}")),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the Spotify provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConfig {
    /// Spotify API access token (obtained via Sigilforge)
    pub access_token: String,
}

// ============================================================================
// Spotify API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct SpotifyPagingResponse<T> {
    items: Vec<T>,
    next: Option<String>,
    #[allow(dead_code)]
    total: u32,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistSimple {
    id: String,
    name: String,
    description: Option<String>,
    owner: SpotifyUser,
    tracks: SpotifyPlaylistTracksRef,
    #[allow(dead_code)]
    images: Vec<SpotifyImage>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistTracksRef {
    total: u32,
}

#[derive(Debug, Deserialize)]
struct SpotifyUser {
    display_name: Option<String>,
    id: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyImage {
    url: String,
    #[allow(dead_code)]
    height: Option<u32>,
    #[allow(dead_code)]
    width: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistTrack {
    track: Option<SpotifyTrack>,
    added_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifySavedTrack {
    track: SpotifyTrack,
    added_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrack {
    id: Option<String>,
    name: String,
    artists: Vec<SpotifyArtist>,
    album: SpotifyAlbum,
    duration_ms: u32,
    uri: String,
    external_urls: SpotifyExternalUrls,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbum {
    name: String,
    images: Vec<SpotifyImage>,
}

#[derive(Debug, Deserialize)]
struct SpotifyExternalUrls {
    spotify: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyErrorResponse {
    error: SpotifyErrorDetail,
}

#[derive(Debug, Deserialize)]
struct SpotifyErrorDetail {
    status: u16,
    message: String,
}

// ============================================================================
// Spotify Provider
// ============================================================================

const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";

/// Spotify provider for accessing playlists and library.
pub struct SpotifyProvider {
    config: Arc<SpotifyConfig>,
    client: Client,
}

impl SpotifyProvider {
    /// Create a new Spotify provider with the given configuration.
    pub fn new(config: SpotifyConfig) -> Self {
        let client = Client::builder()
            .user_agent("Scryforge/0.1.0 (Spotify Provider)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    /// Make an authenticated request to the Spotify API.
    async fn api_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
    ) -> std::result::Result<T, SpotifyError> {
        let url = format!("{}{}", SPOTIFY_API_BASE, endpoint);
        debug!("Spotify API request: {}", url);

        let response = self
            .client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.access_token),
            )
            .send()
            .await?;

        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(SpotifyError::AuthFailed(
                "Invalid or expired access token".to_string(),
            ));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse structured error response
            if let Ok(error_response) = serde_json::from_str::<SpotifyErrorResponse>(&error_text) {
                return Err(SpotifyError::ApiError {
                    status: error_response.error.status,
                    message: error_response.error.message,
                });
            }

            return Err(SpotifyError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let data = response.json::<T>().await.map_err(|e| {
            SpotifyError::Parse(format!("Failed to parse response: {}", e))
        })?;

        Ok(data)
    }

    /// Convert a Spotify track to an Item.
    fn track_to_item(
        &self,
        track: &SpotifyTrack,
        stream_id: StreamId,
        added_at: Option<&str>,
    ) -> Item {
        let track_id = track.id.as_deref().unwrap_or("unknown");
        let item_id = ItemId::new("spotify", track_id);

        // Build artist names
        let artists: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
        let artist_str = artists.join(", ");

        // Get album art URL (prefer medium size)
        let thumbnail_url = track.album.images.first().map(|img| img.url.clone());

        // Parse added_at timestamp
        let published = added_at.and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("duration_ms".to_string(), track.duration_ms.to_string());
        metadata.insert("album".to_string(), track.album.name.clone());
        metadata.insert("uri".to_string(), track.uri.clone());
        if let Some(url) = &thumbnail_url {
            metadata.insert("album_art_url".to_string(), url.clone());
        }

        Item {
            id: item_id,
            stream_id,
            title: track.name.clone(),
            content: ItemContent::Track {
                album: Some(track.album.name.clone()),
                duration_ms: Some(track.duration_ms),
                artists: artists.clone(),
            },
            author: if !artists.is_empty() {
                Some(Author {
                    name: artist_str,
                    email: None,
                    url: None,
                    avatar_url: None,
                })
            } else {
                None
            },
            published,
            updated: None,
            url: Some(track.external_urls.spotify.clone()),
            thumbnail_url,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata,
        }
    }

    /// Fetch all user playlists.
    async fn fetch_playlists(&self) -> std::result::Result<Vec<SpotifyPlaylistSimple>, SpotifyError> {
        let mut all_playlists = Vec::new();
        let mut next_url: Option<String> = Some("/me/playlists?limit=50".to_string());

        while let Some(endpoint) = next_url {
            let response: SpotifyPagingResponse<SpotifyPlaylistSimple> =
                self.api_request(&endpoint).await?;

            all_playlists.extend(response.items);
            next_url = response.next.map(|url| {
                // Extract the path from the full URL
                url.strip_prefix(SPOTIFY_API_BASE)
                    .unwrap_or(&url)
                    .to_string()
            });
        }

        Ok(all_playlists)
    }

    /// Fetch tracks from a playlist.
    async fn fetch_playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> std::result::Result<Vec<SpotifyPlaylistTrack>, SpotifyError> {
        let mut all_tracks = Vec::new();
        let mut next_url: Option<String> = Some(format!("/playlists/{}/tracks?limit=50", playlist_id));

        while let Some(endpoint) = next_url {
            let response: SpotifyPagingResponse<SpotifyPlaylistTrack> =
                self.api_request(&endpoint).await?;

            all_tracks.extend(response.items);
            next_url = response.next.map(|url| {
                url.strip_prefix(SPOTIFY_API_BASE)
                    .unwrap_or(&url)
                    .to_string()
            });
        }

        Ok(all_tracks)
    }

    /// Fetch user's saved/liked tracks.
    async fn fetch_saved_tracks(&self) -> std::result::Result<Vec<SpotifySavedTrack>, SpotifyError> {
        let mut all_tracks = Vec::new();
        let mut next_url: Option<String> = Some("/me/tracks?limit=50".to_string());

        while let Some(endpoint) = next_url {
            let response: SpotifyPagingResponse<SpotifySavedTrack> =
                self.api_request(&endpoint).await?;

            all_tracks.extend(response.items);
            next_url = response.next.map(|url| {
                url.strip_prefix(SPOTIFY_API_BASE)
                    .unwrap_or(&url)
                    .to_string()
            });
        }

        Ok(all_tracks)
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
        // Try to fetch user's playlists to verify authentication
        match self.api_request::<SpotifyPagingResponse<SpotifyPlaylistSimple>>("/me/playlists?limit=1").await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("Successfully authenticated with Spotify API".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => {
                warn!("Spotify health check failed: {}", e);
                Ok(ProviderHealth {
                    is_healthy: false,
                    message: Some(format!("Failed to connect to Spotify: {}", e)),
                    last_sync: None,
                    error_count: 1,
                })
            }
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();
        let mut items_added = 0;
        let mut errors = Vec::new();

        debug!("Syncing Spotify data");

        // Fetch playlists
        match self.fetch_playlists().await {
            Ok(playlists) => {
                debug!("Fetched {} playlists", playlists.len());
                items_added += playlists.len() as u32;
            }
            Err(e) => {
                error!("Failed to fetch playlists: {}", e);
                errors.push(format!("Playlists: {}", e));
            }
        }

        // Fetch saved tracks
        match self.fetch_saved_tracks().await {
            Ok(tracks) => {
                debug!("Fetched {} saved tracks", tracks.len());
                items_added += tracks.len() as u32;
            }
            Err(e) => {
                error!("Failed to fetch saved tracks: {}", e);
                errors.push(format!("Saved tracks: {}", e));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SyncResult {
            success: errors.is_empty(),
            items_added,
            items_updated: 0,
            items_removed: 0,
            errors,
            duration_ms,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: false,
            has_collections: true,
            has_saved_items: true,
            has_communities: false,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "open_spotify".to_string(),
                name: "Open in Spotify".to_string(),
                description: "Open track in Spotify app".to_string(),
                kind: ActionKind::Open,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show track details".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
        ];

        // Add "Open in Browser" if URL is available
        if item.url.is_some() {
            actions.push(Action {
                id: "open_browser".to_string(),
                name: "Open in Browser".to_string(),
                description: "Open track in web browser".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("b".to_string()),
            });

            actions.push(Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy Spotify URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("c".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::Open | ActionKind::OpenInBrowser => {
                if let Some(url) = &item.url {
                    debug!("Opening Spotify URL: {}", url);
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
            ActionKind::CopyLink => {
                if let Some(url) = &item.url {
                    Ok(ActionResult {
                        success: true,
                        message: Some("Link copied to clipboard".to_string()),
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
            _ => Ok(ActionResult {
                success: true,
                message: Some(format!("Executed action: {}", action.name)),
                data: None,
            }),
        }
    }
}

#[async_trait]
impl HasCollections for SpotifyProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let playlists = self
            .fetch_playlists()
            .await
            .map_err(StreamError::from)?;

        Ok(playlists
            .iter()
            .map(|p| Collection {
                id: CollectionId(p.id.clone()),
                name: p.name.clone(),
                description: p.description.clone(),
                icon: Some("🎵".to_string()),
                item_count: p.tracks.total,
                is_editable: false, // Read-only for now (Phase 4 will add write support)
                owner: p.owner.display_name.clone().or(Some(p.owner.id.clone())),
            })
            .collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let stream_id = StreamId::new("spotify", "playlist", &collection_id.0);

        let playlist_tracks = self
            .fetch_playlist_tracks(&collection_id.0)
            .await
            .map_err(StreamError::from)?;

        let items: Vec<Item> = playlist_tracks
            .iter()
            .filter_map(|pt| {
                pt.track.as_ref().map(|track| {
                    self.track_to_item(track, stream_id.clone(), pt.added_at.as_deref())
                })
            })
            .collect();

        Ok(items)
    }
}

#[async_trait]
impl HasSavedItems for SpotifyProvider {
    async fn get_saved_items(&self, options: SavedItemsOptions) -> Result<Vec<Item>> {
        let stream_id = StreamId::new("spotify", "saved", "liked-songs");

        let saved_tracks = self
            .fetch_saved_tracks()
            .await
            .map_err(StreamError::from)?;

        let items: Vec<Item> = saved_tracks
            .iter()
            .map(|st| self.track_to_item(&st.track, stream_id.clone(), st.added_at.as_deref()))
            .collect();

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        let items = items.into_iter().skip(offset);
        let items = if let Some(limit) = limit {
            items.take(limit).collect()
        } else {
            items.collect()
        };

        Ok(items)
    }

    async fn is_saved(&self, _item_id: &ItemId) -> Result<bool> {
        // This would require calling the Spotify API to check if a track is saved
        // For now, we'll return false as a placeholder
        // TODO: Implement proper saved track checking
        Ok(false)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> SpotifyConfig {
        SpotifyConfig {
            access_token: "test_token_12345".to_string(),
        }
    }

    #[test]
    fn test_spotify_provider_creation() {
        let config = create_test_config();
        let provider = SpotifyProvider::new(config);

        assert_eq!(provider.id(), "spotify");
        assert_eq!(provider.name(), "Spotify");

        let caps = provider.capabilities();
        assert!(!caps.has_feeds);
        assert!(caps.has_collections);
        assert!(caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[test]
    fn test_track_to_item_conversion() {
        let config = create_test_config();
        let provider = SpotifyProvider::new(config);

        let track = SpotifyTrack {
            id: Some("track123".to_string()),
            name: "Test Song".to_string(),
            artists: vec![
                SpotifyArtist {
                    name: "Artist One".to_string(),
                },
                SpotifyArtist {
                    name: "Artist Two".to_string(),
                },
            ],
            album: SpotifyAlbum {
                name: "Test Album".to_string(),
                images: vec![SpotifyImage {
                    url: "https://example.com/album.jpg".to_string(),
                    height: Some(640),
                    width: Some(640),
                }],
            },
            duration_ms: 180000, // 3 minutes
            uri: "spotify:track:track123".to_string(),
            external_urls: SpotifyExternalUrls {
                spotify: "https://open.spotify.com/track/track123".to_string(),
            },
        };

        let stream_id = StreamId::new("spotify", "playlist", "test-playlist");
        let item = provider.track_to_item(&track, stream_id.clone(), Some("2024-01-01T12:00:00Z"));

        assert_eq!(item.id.as_str(), "spotify:track123");
        assert_eq!(item.stream_id, stream_id);
        assert_eq!(item.title, "Test Song");

        // Check content is Track type
        match &item.content {
            ItemContent::Track { album, duration_ms, artists } => {
                assert_eq!(album.as_ref().unwrap(), "Test Album");
                assert_eq!(duration_ms.unwrap(), 180000);
                assert_eq!(artists.len(), 2);
                assert_eq!(artists[0], "Artist One");
                assert_eq!(artists[1], "Artist Two");
            }
            _ => panic!("Expected Track content type"),
        }

        // Check author
        assert!(item.author.is_some());
        let author = item.author.as_ref().unwrap();
        assert_eq!(author.name, "Artist One, Artist Two");

        // Check URL and thumbnail
        assert_eq!(
            item.url.as_ref().unwrap(),
            "https://open.spotify.com/track/track123"
        );
        assert_eq!(
            item.thumbnail_url.as_ref().unwrap(),
            "https://example.com/album.jpg"
        );

        // Check metadata
        assert_eq!(item.metadata.get("duration_ms").unwrap(), "180000");
        assert_eq!(item.metadata.get("album").unwrap(), "Test Album");
        assert_eq!(item.metadata.get("uri").unwrap(), "spotify:track:track123");
        assert_eq!(
            item.metadata.get("album_art_url").unwrap(),
            "https://example.com/album.jpg"
        );
    }

    #[tokio::test]
    async fn test_available_actions() {
        let config = create_test_config();
        let provider = SpotifyProvider::new(config);

        let item = Item {
            id: ItemId::new("spotify", "test-track"),
            stream_id: StreamId::new("spotify", "playlist", "test"),
            title: "Test Track".to_string(),
            content: ItemContent::Track {
                album: Some("Test Album".to_string()),
                duration_ms: Some(180000),
                artists: vec!["Test Artist".to_string()],
            },
            author: Some(Author {
                name: "Test Artist".to_string(),
                email: None,
                url: None,
                avatar_url: None,
            }),
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

        // Should have: Open, Preview, Open in Browser, Copy Link
        assert_eq!(actions.len(), 4);
        assert!(actions.iter().any(|a| a.kind == ActionKind::Open));
        assert!(actions.iter().any(|a| a.kind == ActionKind::Preview));
        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_execute_action_open() {
        let config = create_test_config();
        let provider = SpotifyProvider::new(config);

        let item = Item {
            id: ItemId::new("spotify", "test-track"),
            stream_id: StreamId::new("spotify", "playlist", "test"),
            title: "Test Track".to_string(),
            content: ItemContent::Track {
                album: None,
                duration_ms: None,
                artists: vec![],
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
            id: "open_spotify".to_string(),
            name: "Open in Spotify".to_string(),
            description: "Open in Spotify".to_string(),
            kind: ActionKind::Open,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_spotify_error_conversion() {
        let auth_error = SpotifyError::AuthFailed("test".to_string());
        let stream_error: StreamError = auth_error.into();
        assert!(matches!(stream_error, StreamError::AuthRequired(_)));

        let invalid_token_error = SpotifyError::InvalidToken("test".to_string());
        let stream_error: StreamError = invalid_token_error.into();
        assert!(matches!(stream_error, StreamError::AuthRequired(_)));

        let api_error = SpotifyError::ApiError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        let stream_error: StreamError = api_error.into();
        assert!(matches!(stream_error, StreamError::AuthRequired(_)));

        let rate_limit_error = SpotifyError::ApiError {
            status: 429,
            message: "Rate limited".to_string(),
        };
        let stream_error: StreamError = rate_limit_error.into();
        assert!(matches!(stream_error, StreamError::RateLimited(_)));

        let playlist_not_found = SpotifyError::PlaylistNotFound("test".to_string());
        let stream_error: StreamError = playlist_not_found.into();
        assert!(matches!(stream_error, StreamError::StreamNotFound(_)));

        let parse_error = SpotifyError::Parse("test".to_string());
        let stream_error: StreamError = parse_error.into();
        assert!(matches!(stream_error, StreamError::Provider(_)));
    }
}

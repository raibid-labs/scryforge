//! Saved items unified view endpoint.
//!
//! This module provides the implementation for the saved.all RPC method
//! which aggregates saved items from all providers.

use chrono::Utc;
use jsonrpsee::core::RpcResult;

use super::handlers::{JsonValue, SavedItemResponse};

/// Generate dummy saved items for testing.
///
/// This is a placeholder that will be replaced with actual provider aggregation
/// once the registry is wired to the API.
pub fn generate_dummy_saved_items(
    sort: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    filters: Option<JsonValue>,
) -> RpcResult<Vec<SavedItemResponse>> {
    // For now, return some dummy saved items
    let mut items = vec![
        SavedItemResponse {
            item: fusabi_streams_core::Item {
                id: fusabi_streams_core::ItemId::new("reddit", "saved-001"),
                stream_id: fusabi_streams_core::StreamId::new("reddit", "saved", "all"),
                title: "Interesting Reddit Post".to_string(),
                content: fusabi_streams_core::ItemContent::Article {
                    summary: Some("This is a saved Reddit post about Rust...".to_string()),
                    full_content: None,
                },
                author: Some(fusabi_streams_core::Author {
                    name: "rustacean".to_string(),
                    email: None,
                    url: None,
                    avatar_url: None,
                }),
                published: Some(Utc::now()),
                updated: None,
                url: Some("https://reddit.com/r/rust/comments/example".to_string()),
                thumbnail_url: None,
                is_read: false,
                is_saved: true,
                tags: vec![],
                metadata: std::collections::HashMap::new(),
            },
            provider_ids: vec!["reddit".to_string()],
            saved_at: Utc::now().to_rfc3339(),
        },
        SavedItemResponse {
            item: fusabi_streams_core::Item {
                id: fusabi_streams_core::ItemId::new("spotify", "track-saved-001"),
                stream_id: fusabi_streams_core::StreamId::new("spotify", "saved", "tracks"),
                title: "Favorite Song".to_string(),
                content: fusabi_streams_core::ItemContent::Track {
                    album: Some("Great Album".to_string()),
                    duration_ms: Some(240000),
                    artists: vec!["Amazing Artist".to_string()],
                },
                author: Some(fusabi_streams_core::Author {
                    name: "Amazing Artist".to_string(),
                    email: None,
                    url: None,
                    avatar_url: None,
                }),
                published: None,
                updated: None,
                url: Some("https://open.spotify.com/track/saved-example".to_string()),
                thumbnail_url: None,
                is_read: false,
                is_saved: true,
                tags: vec![],
                metadata: std::collections::HashMap::new(),
            },
            provider_ids: vec!["spotify".to_string()],
            saved_at: Utc::now().to_rfc3339(),
        },
    ];

    // Apply filters if provided
    if let Some(ref filter_obj) = filters {
        if let Some(provider) = filter_obj.get("provider").and_then(|v| v.as_str()) {
            items.retain(|item| item.provider_ids.contains(&provider.to_string()));
        }
        if let Some(content_type) = filter_obj.get("content_type").and_then(|v| v.as_str()) {
            items.retain(|item| {
                let item_type = match &item.item.content {
                    fusabi_streams_core::ItemContent::Article { .. } => "article",
                    fusabi_streams_core::ItemContent::Track { .. } => "track",
                    fusabi_streams_core::ItemContent::Video { .. } => "video",
                    _ => "other",
                };
                item_type.eq_ignore_ascii_case(content_type)
            });
        }
    }

    // Apply sorting
    let sort_order = sort.as_deref().unwrap_or("saved_desc");
    match sort_order {
        "saved_asc" => items.sort_by(|a, b| a.saved_at.cmp(&b.saved_at)),
        "saved_desc" => items.sort_by(|a, b| b.saved_at.cmp(&a.saved_at)),
        _ => {} // default: saved_desc
    }

    // Apply pagination
    let offset_val = offset.unwrap_or(0) as usize;
    let limit_val = limit.map(|l| l as usize);

    let result: Vec<SavedItemResponse> = items
        .into_iter()
        .skip(offset_val)
        .take(limit_val.unwrap_or(usize::MAX))
        .collect();

    Ok(result)
}

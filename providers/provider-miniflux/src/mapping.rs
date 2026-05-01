//! Conversion between Miniflux API types and Scryforge domain types.

use scryforge_provider_core::prelude::*;
use std::collections::HashMap;

use crate::api;

/// Provider id used as the namespace prefix for `ItemId` / `StreamId`.
pub(crate) const PROVIDER_ID: &str = "miniflux";

/// Build a stream id of the form `miniflux:feed:<feed-id>`.
pub(crate) fn feed_stream_id(feed_id: i64) -> StreamId {
    StreamId::new(PROVIDER_ID, "feed", &feed_id.to_string())
}

/// Build the saved-items stream id used for starred entries.
pub(crate) fn saved_stream_id() -> StreamId {
    StreamId::new(PROVIDER_ID, "saved", "starred")
}

/// Build the public `FeedId` for a Miniflux feed.
pub(crate) fn feed_id(feed_id: i64) -> FeedId {
    FeedId(format!("miniflux:{}", feed_id))
}

/// Decode a `FeedId` produced by [`feed_id`] back into the numeric Miniflux id.
pub(crate) fn parse_feed_id(id: &FeedId) -> Option<i64> {
    id.0.strip_prefix("miniflux:")
        .and_then(|s| s.parse::<i64>().ok())
}

/// Decode a `miniflux:<n>` `ItemId` back into the numeric Miniflux entry id.
pub(crate) fn parse_item_id(id: &ItemId) -> Option<i64> {
    id.0.strip_prefix("miniflux:")
        .and_then(|s| s.parse::<i64>().ok())
}

/// Convert a Miniflux [`api::Feed`] into a Scryforge [`Feed`].
pub fn feed_to_feed(feed: &api::Feed) -> Feed {
    let description = feed
        .category
        .as_ref()
        .map(|c| format!("Category: {}", c.title));
    Feed {
        id: feed_id(feed.id),
        name: if feed.title.is_empty() {
            feed.feed_url.clone()
        } else {
            feed.title.clone()
        },
        description,
        icon: Some("📰".to_string()),
        unread_count: None,
        total_count: None,
    }
}

/// Convert a Miniflux [`api::Entry`] into a Scryforge [`Item`].
///
/// `stream_id` controls which stream the resulting item is filed under (a feed
/// stream when listing feeds, the starred-items stream when listing saved
/// items, etc.).
pub fn entry_to_item(entry: &api::Entry, stream_id: StreamId) -> Item {
    let id = ItemId::new(PROVIDER_ID, &entry.id.to_string());

    let title = if entry.title.is_empty() {
        "(untitled)".to_string()
    } else {
        entry.title.clone()
    };

    let author = if entry.author.is_empty() {
        None
    } else {
        Some(Author {
            name: entry.author.clone(),
            email: None,
            url: None,
            avatar_url: None,
        })
    };

    let content = ItemContent::Article {
        summary: None,
        full_content: if entry.content.is_empty() {
            None
        } else {
            Some(entry.content.clone())
        },
    };

    // Best-effort thumbnail: first image-typed enclosure.
    let thumbnail_url = entry
        .enclosures
        .iter()
        .find(|enc| enc.mime_type.starts_with("image/"))
        .or_else(|| entry.enclosures.first())
        .map(|enc| enc.url.clone());

    let url = if entry.url.is_empty() {
        None
    } else {
        Some(entry.url.clone())
    };

    let mut metadata = HashMap::new();
    metadata.insert("miniflux_feed_id".to_string(), entry.feed_id.to_string());
    metadata.insert("miniflux_status".to_string(), entry.status.clone());
    if !entry.hash.is_empty() {
        metadata.insert("miniflux_hash".to_string(), entry.hash.clone());
    }
    if let Some(feed) = &entry.feed {
        if !feed.title.is_empty() {
            metadata.insert("feed_title".to_string(), feed.title.clone());
        }
        if let Some(category) = &feed.category {
            metadata.insert("feed_category".to_string(), category.title.clone());
        }
    }

    let is_read = entry.status == "read";
    let tags = entry.tags.clone();

    Item {
        id,
        stream_id,
        title,
        content,
        author,
        published: entry.published_at,
        // The issue spec maps `created_at` → `updated` as a best-effort field;
        // fall back to `changed_at` when `created_at` is missing.
        updated: entry.created_at.or(entry.changed_at),
        url,
        thumbnail_url,
        is_read,
        is_saved: entry.starred,
        tags,
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry() -> api::Entry {
        api::Entry {
            id: 42,
            user_id: 1,
            feed_id: 7,
            status: "unread".to_string(),
            hash: "abc123".to_string(),
            title: "Hello, world".to_string(),
            url: "https://example.com/post".to_string(),
            comments_url: String::new(),
            published_at: None,
            created_at: None,
            changed_at: None,
            author: "Jane".to_string(),
            content: "<p>Body</p>".to_string(),
            share_code: String::new(),
            starred: true,
            reading_time: 0,
            enclosures: vec![api::Enclosure {
                id: 1,
                url: "https://example.com/img.png".to_string(),
                mime_type: "image/png".to_string(),
                size: 0,
            }],
            tags: vec!["rust".to_string()],
            feed: Some(api::EntryFeed {
                id: 7,
                title: "Sample Feed".to_string(),
                site_url: Some("https://example.com".to_string()),
                feed_url: Some("https://example.com/feed".to_string()),
                category: Some(api::Category {
                    id: 3,
                    title: "Tech".to_string(),
                    user_id: 1,
                }),
            }),
        }
    }

    #[test]
    fn entry_maps_to_item_with_expected_fields() {
        let entry = sample_entry();
        let item = entry_to_item(&entry, feed_stream_id(entry.feed_id));

        assert_eq!(item.id.as_str(), "miniflux:42");
        assert_eq!(item.stream_id.as_str(), "miniflux:feed:7");
        assert_eq!(item.title, "Hello, world");
        assert_eq!(item.url.as_deref(), Some("https://example.com/post"));
        assert!(matches!(item.content, ItemContent::Article { .. }));
        assert_eq!(
            item.thumbnail_url.as_deref(),
            Some("https://example.com/img.png")
        );
        assert!(!item.is_read);
        assert!(item.is_saved);
        assert_eq!(item.tags, vec!["rust".to_string()]);
        assert_eq!(item.author.as_ref().unwrap().name, "Jane");
        assert_eq!(
            item.metadata.get("miniflux_feed_id").map(|s| s.as_str()),
            Some("7")
        );
        assert_eq!(
            item.metadata.get("feed_title").map(|s| s.as_str()),
            Some("Sample Feed")
        );
        assert_eq!(
            item.metadata.get("feed_category").map(|s| s.as_str()),
            Some("Tech")
        );
    }

    #[test]
    fn read_status_propagates_to_item_flag() {
        let mut entry = sample_entry();
        entry.status = "read".to_string();
        let item = entry_to_item(&entry, feed_stream_id(entry.feed_id));
        assert!(item.is_read);
    }

    #[test]
    fn feed_id_round_trips() {
        let id = feed_id(123);
        assert_eq!(parse_feed_id(&id), Some(123));
    }

    #[test]
    fn item_id_round_trips() {
        let id = ItemId::new(PROVIDER_ID, "987");
        assert_eq!(parse_item_id(&id), Some(987));
    }
}

//! Test fixtures for integration tests.
//!
//! This module provides reusable mock data and helper functions for testing.

use chrono::Utc;
use scryforge_provider_core::{
    Author, Collection, CollectionId, Item, ItemContent, ItemId, Stream, StreamId, StreamType,
};
use std::collections::HashMap;

/// Creates a test stream with the given ID and provider.
pub fn create_test_stream(id: &str, provider_id: &str, name: &str) -> Stream {
    Stream {
        id: StreamId(id.to_string()),
        name: name.to_string(),
        provider_id: provider_id.to_string(),
        stream_type: StreamType::Feed,
        icon: Some("📥".to_string()),
        unread_count: Some(5),
        total_count: Some(10),
        last_updated: Some(Utc::now()),
        metadata: HashMap::new(),
    }
}

/// Creates a test item with the given ID in a stream.
pub fn create_test_item(item_id: &str, stream_id: &str, title: &str) -> Item {
    Item {
        id: ItemId(item_id.to_string()),
        stream_id: StreamId(stream_id.to_string()),
        title: title.to_string(),
        content: ItemContent::Text(format!("Test content for {}", title)),
        author: Some(Author {
            name: "Test Author".to_string(),
            email: Some("test@example.com".to_string()),
            url: None,
            avatar_url: None,
        }),
        published: Some(Utc::now()),
        updated: None,
        url: Some(format!("https://example.com/{}", item_id)),
        thumbnail_url: None,
        is_read: false,
        is_saved: false,
        tags: vec!["test".to_string()],
        metadata: HashMap::new(),
    }
}

/// Creates a test item with specific read/saved states.
pub fn create_test_item_with_state(
    item_id: &str,
    stream_id: &str,
    title: &str,
    is_read: bool,
    is_saved: bool,
) -> Item {
    Item {
        id: ItemId(item_id.to_string()),
        stream_id: StreamId(stream_id.to_string()),
        title: title.to_string(),
        content: ItemContent::Text(format!("Test content for {}", title)),
        author: Some(Author {
            name: "Test Author".to_string(),
            email: Some("test@example.com".to_string()),
            url: None,
            avatar_url: None,
        }),
        published: Some(Utc::now()),
        updated: None,
        url: Some(format!("https://example.com/{}", item_id)),
        thumbnail_url: None,
        is_read,
        is_saved,
        tags: vec!["test".to_string()],
        metadata: HashMap::new(),
    }
}

/// Creates a test collection.
pub fn create_test_collection(id: &str, name: &str) -> Collection {
    Collection {
        id: CollectionId(id.to_string()),
        name: name.to_string(),
        description: Some(format!("Test collection: {}", name)),
        icon: Some("📁".to_string()),
        item_count: 0,
        is_editable: true,
        owner: Some("test_user".to_string()),
    }
}

/// Creates a set of test streams for a provider.
pub fn create_test_streams(provider_id: &str) -> Vec<Stream> {
    vec![
        create_test_stream(
            &format!("{}:stream:1", provider_id),
            provider_id,
            "Test Stream 1",
        ),
        create_test_stream(
            &format!("{}:stream:2", provider_id),
            provider_id,
            "Test Stream 2",
        ),
        create_test_stream(
            &format!("{}:stream:3", provider_id),
            provider_id,
            "Test Stream 3",
        ),
    ]
}

/// Creates a set of test items for a stream.
pub fn create_test_items(stream_id: &str, count: usize) -> Vec<Item> {
    (0..count)
        .map(|i| {
            create_test_item(
                &format!("test:item:{}", i),
                stream_id,
                &format!("Test Item {}", i),
            )
        })
        .collect()
}

/// Creates test items with various states for filtering tests.
pub fn create_mixed_state_items(stream_id: &str) -> Vec<Item> {
    vec![
        create_test_item_with_state("test:item:1", stream_id, "Unread Item", false, false),
        create_test_item_with_state("test:item:2", stream_id, "Read Item", true, false),
        create_test_item_with_state("test:item:3", stream_id, "Saved Item", false, true),
        create_test_item_with_state("test:item:4", stream_id, "Read and Saved Item", true, true),
    ]
}

/// Creates test items with different content types.
pub fn create_varied_content_items(stream_id: &str) -> Vec<Item> {
    vec![
        Item {
            id: ItemId("test:email:1".to_string()),
            stream_id: StreamId(stream_id.to_string()),
            title: "Test Email".to_string(),
            content: ItemContent::Email {
                subject: "Test Email Subject".to_string(),
                body_text: Some("This is the email body".to_string()),
                body_html: None,
                snippet: "This is the email...".to_string(),
            },
            author: Some(Author {
                name: "Email Sender".to_string(),
                email: Some("sender@example.com".to_string()),
                url: None,
                avatar_url: None,
            }),
            published: Some(Utc::now()),
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        },
        Item {
            id: ItemId("test:article:1".to_string()),
            stream_id: StreamId(stream_id.to_string()),
            title: "Test Article".to_string(),
            content: ItemContent::Article {
                summary: Some("Article summary".to_string()),
                full_content: Some("Full article content".to_string()),
            },
            author: Some(Author {
                name: "Article Author".to_string(),
                email: None,
                url: Some("https://example.com/author".to_string()),
                avatar_url: None,
            }),
            published: Some(Utc::now()),
            updated: None,
            url: Some("https://example.com/article".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec!["article".to_string()],
            metadata: HashMap::new(),
        },
        Item {
            id: ItemId("test:video:1".to_string()),
            stream_id: StreamId(stream_id.to_string()),
            title: "Test Video".to_string(),
            content: ItemContent::Video {
                description: "Video description".to_string(),
                duration_seconds: Some(300),
                view_count: Some(1000),
            },
            author: Some(Author {
                name: "Video Creator".to_string(),
                email: None,
                url: None,
                avatar_url: None,
            }),
            published: Some(Utc::now()),
            updated: None,
            url: Some("https://example.com/video".to_string()),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            is_read: false,
            is_saved: false,
            tags: vec!["video".to_string()],
            metadata: HashMap::new(),
        },
    ]
}

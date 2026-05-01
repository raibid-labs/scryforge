//! Wiremock-driven integration tests for `provider-miniflux`.
//!
//! These tests stand up a `wiremock::MockServer` to impersonate a Miniflux
//! API and exercise the trait surface end-to-end against canned responses.

use provider_miniflux::{MinifluxProvider, MinifluxProviderConfig};
use scryforge_provider_core::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider_for(server: &MockServer) -> MinifluxProvider {
    MinifluxProvider::new(MinifluxProviderConfig::new(server.uri(), "test-token"))
}

#[tokio::test]
async fn list_feeds_round_trips() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/feeds"))
        .and(header("X-Auth-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": 1,
                "user_id": 1,
                "feed_url": "https://example.com/feed.xml",
                "site_url": "https://example.com",
                "title": "Example Feed",
                "checked_at": "2024-01-01T00:00:00Z",
                "category": {"id": 1, "title": "News", "user_id": 1}
            },
            {
                "id": 2,
                "user_id": 1,
                "feed_url": "https://blog.example.com/atom.xml",
                "site_url": "https://blog.example.com",
                "title": "Blog Feed",
                "checked_at": null,
                "category": null
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let feeds = provider.list_feeds().await.unwrap();
    assert_eq!(feeds.len(), 2);
    assert_eq!(feeds[0].id.0, "miniflux:1");
    assert_eq!(feeds[0].name, "Example Feed");
    assert_eq!(feeds[0].description.as_deref(), Some("Category: News"));
    assert_eq!(feeds[1].id.0, "miniflux:2");
}

#[tokio::test]
async fn get_feed_items_filters_unread_by_default() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/entries"))
        .and(query_param("feed_id", "7"))
        .and(query_param("status", "unread"))
        .and(header("X-Auth-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 1,
            "entries": [
                {
                    "id": 100,
                    "user_id": 1,
                    "feed_id": 7,
                    "status": "unread",
                    "hash": "deadbeef",
                    "title": "Hello",
                    "url": "https://example.com/hello",
                    "comments_url": "",
                    "published_at": "2024-01-01T12:00:00Z",
                    "created_at": "2024-01-01T12:00:01Z",
                    "changed_at": "2024-01-01T12:00:02Z",
                    "author": "Author Name",
                    "content": "<p>Body</p>",
                    "share_code": "",
                    "starred": false,
                    "reading_time": 2,
                    "enclosures": [],
                    "tags": ["rust"],
                    "feed": {
                        "id": 7,
                        "title": "Sample Feed",
                        "site_url": "https://example.com",
                        "feed_url": "https://example.com/feed",
                        "category": {"id": 1, "title": "Tech", "user_id": 1}
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let items = provider
        .get_feed_items(&FeedId("miniflux:7".to_string()), FeedOptions::default())
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.id.as_str(), "miniflux:100");
    assert_eq!(item.stream_id.as_str(), "miniflux:feed:7");
    assert_eq!(item.title, "Hello");
    assert_eq!(item.url.as_deref(), Some("https://example.com/hello"));
    assert!(!item.is_read);
    assert_eq!(item.tags, vec!["rust".to_string()]);
    assert!(matches!(item.content, ItemContent::Article { .. }));
}

#[tokio::test]
async fn get_feed_items_unknown_feed_id_is_stream_not_found() {
    let server = MockServer::start().await;
    let provider = provider_for(&server);
    let err = provider
        .get_feed_items(
            &FeedId("not-miniflux:7".to_string()),
            FeedOptions::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, StreamError::StreamNotFound(_)));
}

#[tokio::test]
async fn get_saved_items_lists_starred_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/entries"))
        .and(query_param("starred", "true"))
        .and(header("X-Auth-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 1,
            "entries": [
                {
                    "id": 200,
                    "user_id": 1,
                    "feed_id": 9,
                    "status": "read",
                    "hash": "hashy",
                    "title": "Saved One",
                    "url": "https://example.com/saved",
                    "comments_url": "",
                    "published_at": null,
                    "created_at": null,
                    "changed_at": null,
                    "author": "",
                    "content": "",
                    "share_code": "",
                    "starred": true,
                    "reading_time": 0,
                    "enclosures": [],
                    "tags": [],
                    "feed": null
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let items = provider
        .get_saved_items(SavedItemsOptions::default())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].is_saved);
    assert_eq!(items[0].stream_id.as_str(), "miniflux:saved:starred");
}

#[tokio::test]
async fn mark_read_action_round_trips_to_server() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/v1/entries"))
        .and(header("X-Auth-Token", "test-token"))
        .and(body_partial_json(json!({
            "entry_ids": [42],
            "status": "read"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let item = Item {
        id: ItemId::new("miniflux", "42"),
        stream_id: StreamId::new("miniflux", "feed", "1"),
        title: "x".to_string(),
        content: ItemContent::Article {
            summary: None,
            full_content: None,
        },
        author: None,
        published: None,
        updated: None,
        url: None,
        thumbnail_url: None,
        is_read: false,
        is_saved: false,
        tags: vec![],
        metadata: Default::default(),
    };
    let action = Action {
        id: "mark_read".to_string(),
        name: "Mark as Read".to_string(),
        description: String::new(),
        kind: ActionKind::MarkRead,
        keyboard_shortcut: None,
    };
    let result = provider.execute_action(&item, &action).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn star_action_round_trips_via_bookmark_endpoint() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/v1/entries/55/bookmark"))
        .and(header("X-Auth-Token", "test-token"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let item = Item {
        id: ItemId::new("miniflux", "55"),
        stream_id: StreamId::new("miniflux", "feed", "1"),
        title: "x".to_string(),
        content: ItemContent::Article {
            summary: None,
            full_content: None,
        },
        author: None,
        published: None,
        updated: None,
        url: None,
        thumbnail_url: None,
        is_read: false,
        is_saved: false,
        tags: vec![],
        metadata: Default::default(),
    };
    let action = Action {
        id: "save".to_string(),
        name: "Star Article".to_string(),
        description: String::new(),
        kind: ActionKind::Save,
        keyboard_shortcut: None,
    };
    let result = provider.execute_action(&item, &action).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn save_item_is_idempotent_when_already_starred() {
    let server = MockServer::start().await;

    // is_saved sees the entry already in the starred list, so toggle_bookmark
    // must NOT be called.
    Mock::given(method("GET"))
        .and(path("/v1/entries"))
        .and(query_param("starred", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 1,
            "entries": [{
                "id": 77,
                "user_id": 1,
                "feed_id": 1,
                "status": "read",
                "hash": "h",
                "title": "Already starred",
                "url": "https://example.com/x",
                "comments_url": "",
                "published_at": null,
                "created_at": null,
                "changed_at": null,
                "author": "",
                "content": "",
                "share_code": "",
                "starred": true,
                "reading_time": 0,
                "enclosures": [],
                "tags": [],
                "feed": null
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    provider
        .save_item(&ItemId::new("miniflux", "77"))
        .await
        .unwrap();
    // Successful return + the lone GET expectation is enough — wiremock would
    // otherwise complain about an unexpected PUT on drop.
}

#[tokio::test]
async fn unsave_item_toggles_when_currently_starred() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/entries"))
        .and(query_param("starred", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 1,
            "entries": [{
                "id": 88,
                "user_id": 1,
                "feed_id": 1,
                "status": "read",
                "hash": "h",
                "title": "Will be unsaved",
                "url": "https://example.com/x",
                "comments_url": "",
                "published_at": null,
                "created_at": null,
                "changed_at": null,
                "author": "",
                "content": "",
                "share_code": "",
                "starred": true,
                "reading_time": 0,
                "enclosures": [],
                "tags": [],
                "feed": null
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/v1/entries/88/bookmark"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    provider
        .unsave_item(&ItemId::new("miniflux", "88"))
        .await
        .unwrap();
}

#[tokio::test]
async fn unauthorized_response_maps_to_auth_required() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/feeds"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let err = provider.list_feeds().await.unwrap_err();
    assert!(
        matches!(err, StreamError::AuthRequired(_)),
        "expected AuthRequired, got {err:?}"
    );
}

#[tokio::test]
async fn health_check_uses_me_endpoint() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .and(header("X-Auth-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1,
            "username": "alice",
            "is_admin": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let health = provider.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert!(health.message.unwrap().contains("alice"));
}

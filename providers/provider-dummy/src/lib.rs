//! # provider-dummy
//!
//! A dummy provider implementation for testing and development.
//!
//! This provider returns static fixture data and does not connect to any real services.
//! It implements the `Provider` and `HasFeeds` traits to demonstrate the provider pattern
//! and to facilitate testing of the daemon and TUI components.

use async_trait::async_trait;
use chrono::Utc;
use scryforge_provider_core::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Internal state for collections.
#[derive(Debug, Clone)]
struct CollectionState {
    collections: HashMap<String, Collection>,
    collection_items: HashMap<String, Vec<ItemId>>,
    next_collection_id: u64,
}

impl Default for CollectionState {
    fn default() -> Self {
        let mut collections = HashMap::new();
        let mut collection_items = HashMap::new();

        // Initialize with some dummy collections
        let playlist_id = "dummy:playlist-1".to_string();
        collections.insert(
            playlist_id.clone(),
            Collection {
                id: CollectionId(playlist_id.clone()),
                name: "My Favorites".to_string(),
                description: Some("A dummy collection of favorite items".to_string()),
                icon: Some("⭐".to_string()),
                item_count: 2,
                is_editable: true,
                owner: Some("dummy_user".to_string()),
            },
        );
        collection_items.insert(
            playlist_id.clone(),
            vec![
                ItemId::new("dummy", "vid-1"),
                ItemId::new("dummy", "vid-2"),
            ],
        );

        let reading_list_id = "dummy:reading-list".to_string();
        collections.insert(
            reading_list_id.clone(),
            Collection {
                id: CollectionId(reading_list_id.clone()),
                name: "Reading List".to_string(),
                description: Some("Articles to read later".to_string()),
                icon: Some("📚".to_string()),
                item_count: 0,
                is_editable: true,
                owner: Some("dummy_user".to_string()),
            },
        );
        collection_items.insert(reading_list_id.clone(), vec![]);

        Self {
            collections,
            collection_items,
            next_collection_id: 3,
        }
    }
}

/// A dummy provider that returns static test data.
pub struct DummyProvider {
    state: Arc<Mutex<CollectionState>>,
}

impl DummyProvider {
    /// Create a new dummy provider instance.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CollectionState::default())),
        }
    }

    /// Generate static dummy feeds.
    fn dummy_feeds() -> Vec<Feed> {
        vec![
            Feed {
                id: FeedId("dummy:subscriptions".to_string()),
                name: "Subscriptions".to_string(),
                description: Some("Latest videos from your subscriptions".to_string()),
                icon: Some("📺".to_string()),
                unread_count: Some(7),
                total_count: Some(10),
            },
            Feed {
                id: FeedId("dummy:watch-later".to_string()),
                name: "Watch Later".to_string(),
                description: Some("Videos saved to watch later".to_string()),
                icon: Some("⏰".to_string()),
                unread_count: Some(3),
                total_count: Some(5),
            },
            Feed {
                id: FeedId("dummy:liked-videos".to_string()),
                name: "Liked Videos".to_string(),
                description: Some("Videos you've liked".to_string()),
                icon: Some("👍".to_string()),
                unread_count: Some(0),
                total_count: Some(8),
            },
        ]
    }

    /// Generate static dummy items for a given feed.
    fn dummy_items(feed_id: &FeedId) -> Vec<Item> {
        let stream_id = StreamId::new("dummy", "feed", feed_id.0.as_str());

        match feed_id.0.as_str() {
            "dummy:subscriptions" => vec![
                Item {
                    id: ItemId::new("dummy", "vid-1"),
                    stream_id: stream_id.clone(),
                    title: "Building a Rust CLI from Scratch".to_string(),
                    content: ItemContent::Video {
                        description: "In this video, we build a complete CLI application using Rust. We'll cover argument parsing with clap, error handling with anyhow, and creating a polished user experience with colored terminal output.".to_string(),
                        duration_seconds: Some(1832), // 30:32
                        view_count: Some(45_678),
                    },
                    author: Some(Author {
                        name: "RustConf".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/RustConf".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(3)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo1".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo1/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "cli".to_string(), "tutorial".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-2"),
                    stream_id: stream_id.clone(),
                    title: "Async Rust: Tokio Deep Dive".to_string(),
                    content: ItemContent::Video {
                        description: "A comprehensive guide to async programming in Rust using Tokio. Learn about futures, async/await, spawning tasks, and building high-performance concurrent applications.".to_string(),
                        duration_seconds: Some(2547), // 42:27
                        view_count: Some(89_234),
                    },
                    author: Some(Author {
                        name: "Rust Programming Channel".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/RustProgramming".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::hours(18)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo2".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo2/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: true,
                    tags: vec!["rust".to_string(), "async".to_string(), "tokio".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-3"),
                    stream_id: stream_id.clone(),
                    title: "What's New in Rust 1.76".to_string(),
                    content: ItemContent::Video {
                        description: "Quick overview of the latest Rust release. New features, stabilizations, and improvements to the language and tooling.".to_string(),
                        duration_seconds: Some(485), // 8:05
                        view_count: Some(156_789),
                    },
                    author: Some(Author {
                        name: "Rust Foundation".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/RustFoundation".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(1)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo3".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo3/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "news".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-4"),
                    stream_id: stream_id.clone(),
                    title: "Building TUI Apps in Rust with Ratatui".to_string(),
                    content: ItemContent::Video {
                        description: "Learn how to build beautiful terminal user interfaces in Rust. We'll use the ratatui library to create an interactive dashboard with widgets, layouts, and event handling.".to_string(),
                        duration_seconds: Some(3621), // 1:00:21
                        view_count: Some(67_432),
                    },
                    author: Some(Author {
                        name: "Terminal Wizardry".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/TerminalWizardry".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::hours(48)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo4".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo4/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "tui".to_string(), "ratatui".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-5"),
                    stream_id: stream_id.clone(),
                    title: "Rust Web Frameworks Compared: Actix vs Axum vs Rocket".to_string(),
                    content: ItemContent::Video {
                        description: "An in-depth comparison of the most popular Rust web frameworks. We'll look at performance, ergonomics, ecosystem, and use cases for each framework.".to_string(),
                        duration_seconds: Some(1923), // 32:03
                        view_count: Some(234_567),
                    },
                    author: Some(Author {
                        name: "Code Comparison".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/CodeComparison".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(5)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo5".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo5/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "web".to_string(), "frameworks".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-6"),
                    stream_id: stream_id.clone(),
                    title: "Error Handling in Rust: From Beginner to Expert".to_string(),
                    content: ItemContent::Video {
                        description: "Master error handling in Rust! We cover Result, Option, the ? operator, custom error types with thiserror, and advanced patterns for production applications.".to_string(),
                        duration_seconds: Some(2785), // 46:25
                        view_count: Some(178_923),
                    },
                    author: Some(Author {
                        name: "Rust Mastery".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/RustMastery".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(7)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo6".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo6/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "errors".to_string(), "tutorial".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "vid-7"),
                    stream_id: stream_id.clone(),
                    title: "Live Coding: Building a JSON-RPC Server in Rust".to_string(),
                    content: ItemContent::Video {
                        description: "Join me as we build a JSON-RPC server from scratch in Rust. We'll implement the spec, add async support with Tokio, and create a client library.".to_string(),
                        duration_seconds: Some(7234), // 2:00:34
                        view_count: Some(23_456),
                    },
                    author: Some(Author {
                        name: "Live Rust Coding".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/LiveRustCoding".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::hours(6)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=demo7".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/demo7/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "jsonrpc".to_string(), "live".to_string()],
                    metadata: Default::default(),
                },
            ],
            "dummy:watch-later" => vec![
                Item {
                    id: ItemId::new("dummy", "wl-1"),
                    stream_id: stream_id.clone(),
                    title: "Understanding Rust Lifetimes Once and For All".to_string(),
                    content: ItemContent::Video {
                        description: "Lifetimes explained with practical examples. No more confusion about 'a, 'static, and lifetime elision rules.".to_string(),
                        duration_seconds: Some(1456), // 24:16
                        view_count: Some(456_789),
                    },
                    author: Some(Author {
                        name: "Rust Simplified".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/RustSimplified".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(14)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=wl1".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/wl1/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: true,
                    tags: vec!["rust".to_string(), "lifetimes".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "wl-2"),
                    stream_id: stream_id.clone(),
                    title: "Rust Macros: The Complete Guide".to_string(),
                    content: ItemContent::Video {
                        description: "Everything you need to know about macros in Rust. From macro_rules! to procedural macros, derive macros, and attribute macros.".to_string(),
                        duration_seconds: Some(3142), // 52:22
                        view_count: Some(123_456),
                    },
                    author: Some(Author {
                        name: "Advanced Rust".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/AdvancedRust".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(10)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=wl2".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/wl2/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: true,
                    tags: vec!["rust".to_string(), "macros".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "wl-3"),
                    stream_id: stream_id.clone(),
                    title: "Building a Database in Rust: Part 1".to_string(),
                    content: ItemContent::Video {
                        description: "First part of our series on building a relational database from scratch in Rust. We'll implement a B-tree, query parser, and basic SQL support.".to_string(),
                        duration_seconds: Some(4567), // 1:16:07
                        view_count: Some(89_123),
                    },
                    author: Some(Author {
                        name: "Database Internals".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/DatabaseInternals".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(21)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=wl3".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/wl3/hqdefault.jpg".to_string()),
                    is_read: false,
                    is_saved: true,
                    tags: vec!["rust".to_string(), "database".to_string(), "series".to_string()],
                    metadata: Default::default(),
                },
            ],
            "dummy:liked-videos" => vec![
                Item {
                    id: ItemId::new("dummy", "like-1"),
                    stream_id: stream_id.clone(),
                    title: "Rust in Production: Lessons from Discord".to_string(),
                    content: ItemContent::Video {
                        description: "Discord engineering team shares their experience running Rust in production at scale. Performance wins, challenges, and best practices.".to_string(),
                        duration_seconds: Some(2134), // 35:34
                        view_count: Some(567_890),
                    },
                    author: Some(Author {
                        name: "Discord Engineering".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/DiscordEng".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(45)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=like1".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/like1/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "production".to_string(), "discord".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "like-2"),
                    stream_id: stream_id.clone(),
                    title: "The Rust Borrow Checker Explained Visually".to_string(),
                    content: ItemContent::Video {
                        description: "A visual guide to understanding how the Rust borrow checker works. See ownership, borrowing, and lifetimes in action with animations.".to_string(),
                        duration_seconds: Some(892), // 14:52
                        view_count: Some(892_345),
                    },
                    author: Some(Author {
                        name: "Visual Rust".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/VisualRust".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(60)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=like2".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/like2/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "borrow-checker".to_string(), "visual".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "like-3"),
                    stream_id: stream_id.clone(),
                    title: "Writing Fast Rust Code".to_string(),
                    content: ItemContent::Video {
                        description: "Performance optimization techniques for Rust. Profiling, benchmarking, avoiding allocations, and leveraging zero-cost abstractions.".to_string(),
                        duration_seconds: Some(2678), // 44:38
                        view_count: Some(345_678),
                    },
                    author: Some(Author {
                        name: "Performance Matters".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/PerfMatters".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(30)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=like3".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/like3/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "performance".to_string(), "optimization".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "like-4"),
                    stream_id: stream_id.clone(),
                    title: "Rust for Linux Kernel Development".to_string(),
                    content: ItemContent::Video {
                        description: "Overview of the Rust for Linux project. How Rust is being integrated into the Linux kernel and what it means for systems programming.".to_string(),
                        duration_seconds: Some(1834), // 30:34
                        view_count: Some(678_901),
                    },
                    author: Some(Author {
                        name: "Linux Foundation".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/LinuxFoundation".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(90)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=like4".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/like4/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "linux".to_string(), "kernel".to_string()],
                    metadata: Default::default(),
                },
                Item {
                    id: ItemId::new("dummy", "like-5"),
                    stream_id: stream_id.clone(),
                    title: "Embedded Rust: Getting Started with ESP32".to_string(),
                    content: ItemContent::Video {
                        description: "Introduction to embedded Rust development on ESP32 microcontrollers. Set up your environment, write firmware, and deploy to hardware.".to_string(),
                        duration_seconds: Some(2345), // 39:05
                        view_count: Some(234_567),
                    },
                    author: Some(Author {
                        name: "Embedded Rust".to_string(),
                        email: None,
                        url: Some("https://youtube.com/c/EmbeddedRust".to_string()),
                        avatar_url: None,
                    }),
                    published: Some(Utc::now() - chrono::Duration::days(15)),
                    updated: None,
                    url: Some("https://youtube.com/watch?v=like5".to_string()),
                    thumbnail_url: Some("https://i.ytimg.com/vi/like5/hqdefault.jpg".to_string()),
                    is_read: true,
                    is_saved: false,
                    tags: vec!["rust".to_string(), "embedded".to_string(), "esp32".to_string()],
                    metadata: Default::default(),
                },
            ],
            _ => vec![],
        }
    }
}

impl Default for DummyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for DummyProvider {
    fn id(&self) -> &'static str {
        "dummy"
    }

    fn name(&self) -> &'static str {
        "Dummy Provider"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            is_healthy: true,
            message: Some("Dummy provider is always healthy".to_string()),
            last_sync: Some(Utc::now()),
            error_count: 0,
        })
    }

    async fn sync(&self) -> Result<SyncResult> {
        // Simulate a successful sync
        Ok(SyncResult {
            success: true,
            items_added: 0,
            items_updated: 0,
            items_removed: 0,
            errors: vec![],
            duration_ms: 10,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: true,
            has_saved_items: false,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
            Action {
                id: "open".to_string(),
                name: "Open".to_string(),
                description: "Open item".to_string(),
                kind: ActionKind::Open,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Read".to_string(),
                description: "Mark item as read".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
        ])
    }

    async fn execute_action(&self, _item: &Item, action: &Action) -> Result<ActionResult> {
        Ok(ActionResult {
            success: true,
            message: Some(format!("Executed action: {}", action.name)),
            data: None,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl HasFeeds for DummyProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        Ok(Self::dummy_feeds())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let mut items = Self::dummy_items(feed_id);

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Apply since filter
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

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
}

#[async_trait]
impl HasCollections for DummyProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let state = self.state.lock().unwrap();
        let collections: Vec<Collection> = state.collections.values().cloned().collect();
        Ok(collections)
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let state = self.state.lock().unwrap();

        let item_ids = state
            .collection_items
            .get(&collection_id.0)
            .ok_or_else(|| {
                StreamError::StreamNotFound(format!("Collection not found: {}", collection_id.0))
            })?;

        // Generate items for the collection items
        // In a real provider, you would fetch the actual items
        let stream_id = StreamId::new("dummy", "collection", &collection_id.0);
        let items: Vec<Item> = item_ids
            .iter()
            .enumerate()
            .map(|(idx, item_id)| Item {
                id: item_id.clone(),
                stream_id: stream_id.clone(),
                title: format!("Collection Item {}", idx + 1),
                content: ItemContent::Text(format!(
                    "This is item {} in the collection",
                    item_id.as_str()
                )),
                author: None,
                published: Some(Utc::now() - chrono::Duration::hours(idx as i64)),
                updated: None,
                url: Some(format!("https://example.com/{}", item_id.as_str())),
                thumbnail_url: None,
                is_read: false,
                is_saved: true,
                tags: vec!["collection".to_string()],
                metadata: Default::default(),
            })
            .collect();

        Ok(items)
    }

    async fn add_to_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        // Check if collection exists and is editable
        {
            let collection = state.collections.get(&collection_id.0).ok_or_else(|| {
                StreamError::StreamNotFound(format!("Collection not found: {}", collection_id.0))
            })?;

            if !collection.is_editable {
                return Err(StreamError::Provider(format!(
                    "Collection {} is not editable",
                    collection_id.0
                )));
            }
        }

        // Get the items list for this collection
        let items = state
            .collection_items
            .get_mut(&collection_id.0)
            .ok_or_else(|| StreamError::Internal("Collection items list not found".to_string()))?;

        // Check if item is already in collection
        if items.contains(item_id) {
            return Err(StreamError::Provider(format!(
                "Item {} already exists in collection {}",
                item_id.as_str(),
                collection_id.0
            )));
        }

        // Add item to collection
        items.push(item_id.clone());

        // Update item count
        let new_count = items.len() as u32;
        if let Some(collection) = state.collections.get_mut(&collection_id.0) {
            collection.item_count = new_count;
        }

        Ok(())
    }

    async fn remove_from_collection(
        &self,
        collection_id: &CollectionId,
        item_id: &ItemId,
    ) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        // Check if collection exists and is editable
        {
            let collection = state.collections.get(&collection_id.0).ok_or_else(|| {
                StreamError::StreamNotFound(format!("Collection not found: {}", collection_id.0))
            })?;

            if !collection.is_editable {
                return Err(StreamError::Provider(format!(
                    "Collection {} is not editable",
                    collection_id.0
                )));
            }
        }

        // Get the items list for this collection
        let items = state
            .collection_items
            .get_mut(&collection_id.0)
            .ok_or_else(|| StreamError::Internal("Collection items list not found".to_string()))?;

        // Find and remove the item
        let original_len = items.len();
        items.retain(|id| id != item_id);

        if items.len() == original_len {
            return Err(StreamError::ItemNotFound(format!(
                "Item {} not found in collection {}",
                item_id.as_str(),
                collection_id.0
            )));
        }

        // Update item count
        let new_count = items.len() as u32;
        if let Some(collection) = state.collections.get_mut(&collection_id.0) {
            collection.item_count = new_count;
        }

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> Result<Collection> {
        let mut state = self.state.lock().unwrap();

        let collection_id = format!("dummy:collection-{}", state.next_collection_id);
        state.next_collection_id += 1;

        let collection = Collection {
            id: CollectionId(collection_id.clone()),
            name: name.to_string(),
            description: Some(format!("User-created collection: {}", name)),
            icon: Some("📁".to_string()),
            item_count: 0,
            is_editable: true,
            owner: Some("dummy_user".to_string()),
        };

        state
            .collections
            .insert(collection_id.clone(), collection.clone());
        state.collection_items.insert(collection_id.clone(), vec![]);

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dummy_provider_basics() {
        let provider = DummyProvider::new();

        assert_eq!(provider.id(), "dummy");
        assert_eq!(provider.name(), "Dummy Provider");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_collections);
    }

    #[tokio::test]
    async fn test_health_check() {
        let provider = DummyProvider::new();
        let health = provider.health_check().await.unwrap();

        assert!(health.is_healthy);
        assert_eq!(health.error_count, 0);
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let provider = DummyProvider::new();
        let feeds = provider.list_feeds().await.unwrap();

        assert_eq!(feeds.len(), 3);
        assert_eq!(feeds[0].id.0, "dummy:subscriptions");
        assert_eq!(feeds[1].id.0, "dummy:watch-later");
        assert_eq!(feeds[2].id.0, "dummy:liked-videos");
    }

    #[tokio::test]
    async fn test_get_feed_items() {
        let provider = DummyProvider::new();
        let feed_id = FeedId("dummy:subscriptions".to_string());
        let options = FeedOptions {
            include_read: true, // Include all items
            ..Default::default()
        };

        let items = provider.get_feed_items(&feed_id, options).await.unwrap();
        assert_eq!(items.len(), 7);
        assert_eq!(items[0].title, "Building a Rust CLI from Scratch");
    }

    #[tokio::test]
    async fn test_get_feed_items_exclude_read() {
        let provider = DummyProvider::new();
        let feed_id = FeedId("dummy:subscriptions".to_string());
        let options = FeedOptions {
            include_read: false,
            ..Default::default()
        };

        let items = provider.get_feed_items(&feed_id, options).await.unwrap();
        assert_eq!(items.len(), 5); // Two items are marked as read
        assert!(!items.iter().any(|item| item.is_read));
    }

    #[tokio::test]
    async fn test_get_feed_items_with_limit() {
        let provider = DummyProvider::new();
        let feed_id = FeedId("dummy:subscriptions".to_string());
        let options = FeedOptions {
            limit: Some(2),
            ..Default::default()
        };

        let items = provider.get_feed_items(&feed_id, options).await.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_available_actions() {
        let provider = DummyProvider::new();
        let item = Item {
            id: ItemId::new("dummy", "test"),
            stream_id: StreamId::new("dummy", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Text("Test".to_string()),
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

        let actions = provider.available_actions(&item).await.unwrap();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].kind, ActionKind::Open);
        assert_eq!(actions[1].kind, ActionKind::Preview);
        assert_eq!(actions[2].kind, ActionKind::MarkRead);
    }

    #[tokio::test]
    async fn test_execute_action() {
        let provider = DummyProvider::new();
        let item = Item {
            id: ItemId::new("dummy", "test"),
            stream_id: StreamId::new("dummy", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Text("Test".to_string()),
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
            id: "open".to_string(),
            name: "Open".to_string(),
            description: "Open item".to_string(),
            kind: ActionKind::Open,
            keyboard_shortcut: Some("o".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_list_collections() {
        let provider = DummyProvider::new();
        let collections = provider.list_collections().await.unwrap();

        assert_eq!(collections.len(), 2);
        let names: Vec<_> = collections.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"My Favorites"));
        assert!(names.contains(&"Reading List"));
    }

    #[tokio::test]
    async fn test_get_collection_items() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:playlist-1".to_string());

        let items = provider.get_collection_items(&collection_id).await.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_add_to_collection() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:playlist-1".to_string());
        let item_id = ItemId::new("dummy", "item-3");

        // Add item to collection
        provider
            .add_to_collection(&collection_id, &item_id)
            .await
            .unwrap();

        // Verify item was added
        let items = provider.get_collection_items(&collection_id).await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn test_add_duplicate_to_collection() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:playlist-1".to_string());
        let item_id = ItemId::new("dummy", "vid-1");

        // Try to add item that's already in collection
        let result = provider.add_to_collection(&collection_id, &item_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_from_collection() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:playlist-1".to_string());
        let item_id = ItemId::new("dummy", "vid-1");

        // Remove item from collection
        provider
            .remove_from_collection(&collection_id, &item_id)
            .await
            .unwrap();

        // Verify item was removed
        let items = provider.get_collection_items(&collection_id).await.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_from_collection() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:playlist-1".to_string());
        let item_id = ItemId::new("dummy", "nonexistent");

        // Try to remove item that's not in collection
        let result = provider
            .remove_from_collection(&collection_id, &item_id)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_collection() {
        let provider = DummyProvider::new();

        // Create new collection
        let collection = provider.create_collection("Test Collection").await.unwrap();
        assert_eq!(collection.name, "Test Collection");
        assert_eq!(collection.item_count, 0);
        assert!(collection.is_editable);

        // Verify it appears in collections list
        let collections = provider.list_collections().await.unwrap();
        assert_eq!(collections.len(), 3);
    }

    #[tokio::test]
    async fn test_collection_item_count_updates() {
        let provider = DummyProvider::new();
        let collection_id = CollectionId("dummy:reading-list".to_string());

        // Initially empty
        let collections = provider.list_collections().await.unwrap();
        let reading_list = collections
            .iter()
            .find(|c| c.id.0 == "dummy:reading-list")
            .unwrap();
        assert_eq!(reading_list.item_count, 0);

        // Add an item
        let item_id = ItemId::new("dummy", "vid-1");
        provider
            .add_to_collection(&collection_id, &item_id)
            .await
            .unwrap();

        // Check count updated
        let collections = provider.list_collections().await.unwrap();
        let reading_list = collections
            .iter()
            .find(|c| c.id.0 == "dummy:reading-list")
            .unwrap();
        assert_eq!(reading_list.item_count, 1);

        // Remove the item
        provider
            .remove_from_collection(&collection_id, &item_id)
            .await
            .unwrap();

        // Check count updated
        let collections = provider.list_collections().await.unwrap();
        let reading_list = collections
            .iter()
            .find(|c| c.id.0 == "dummy:reading-list")
            .unwrap();
        assert_eq!(reading_list.item_count, 0);
    }
}

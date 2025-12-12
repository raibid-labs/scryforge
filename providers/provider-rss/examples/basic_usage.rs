//! Basic usage example for the RSS provider
//!
//! Run with: cargo run --package provider-rss --example basic_usage

use provider_rss::{RssProvider, RssProviderConfig};
use scryforge_provider_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a configuration with some popular RSS feeds
    let config = RssProviderConfig::new(vec![
        // Rust Blog
        "https://blog.rust-lang.org/feed.xml".to_string(),
        // GitHub Blog
        "https://github.blog/feed/".to_string(),
    ]);

    // Create the provider
    let provider = RssProvider::new(config);

    println!("RSS Provider: {}", provider.name());
    println!("Provider ID: {}", provider.id());
    println!();

    // Check provider health
    println!("Checking provider health...");
    match provider.health_check().await {
        Ok(health) => {
            println!("Health: {}", if health.is_healthy { "OK" } else { "FAIL" });
            if let Some(msg) = health.message {
                println!("Message: {}", msg);
            }
        }
        Err(e) => {
            println!("Health check failed: {}", e);
        }
    }
    println!();

    // List all feeds
    println!("Listing feeds...");
    match provider.list_feeds().await {
        Ok(feeds) => {
            println!("Found {} feeds:", feeds.len());
            for feed in &feeds {
                println!("  - {} (ID: {})", feed.name, feed.id.0);
                if let Some(desc) = &feed.description {
                    println!("    Description: {}", desc);
                }
                if let Some(count) = feed.unread_count {
                    println!("    Unread: {}", count);
                }
            }
            println!();

            // Get items from the first feed
            if let Some(first_feed) = feeds.first() {
                println!("Fetching items from: {}", first_feed.name);
                let options = FeedOptions {
                    limit: Some(5),
                    include_read: true,
                    ..Default::default()
                };

                match provider.get_feed_items(&first_feed.id, options).await {
                    Ok(items) => {
                        println!("Found {} items:", items.len());
                        for (idx, item) in items.iter().enumerate() {
                            println!("  {}. {}", idx + 1, item.title);
                            if let Some(author) = &item.author {
                                println!("     Author: {}", author.name);
                            }
                            if let Some(published) = item.published {
                                println!("     Published: {}", published.format("%Y-%m-%d %H:%M"));
                            }
                            if let Some(url) = &item.url {
                                println!("     URL: {}", url);
                            }
                            if let ItemContent::Article {
                                summary: Some(summary_text),
                                ..
                            } = &item.content
                            {
                                let truncated = if summary_text.len() > 100 {
                                    format!("{}...", &summary_text[..100])
                                } else {
                                    summary_text.clone()
                                };
                                println!("     Summary: {}", truncated);
                            }
                            println!();
                        }

                        // Show available actions for the first item
                        if let Some(first_item) = items.first() {
                            println!("Available actions for '{}':", first_item.title);
                            match provider.available_actions(first_item).await {
                                Ok(actions) => {
                                    for action in actions {
                                        print!("  - {} ({})", action.name, action.description);
                                        if let Some(shortcut) = action.keyboard_shortcut {
                                            print!(" [{}]", shortcut);
                                        }
                                        println!();
                                    }
                                }
                                Err(e) => println!("Failed to get actions: {}", e),
                            }
                        }
                    }
                    Err(e) => println!("Failed to fetch items: {}", e),
                }
            }
        }
        Err(e) => {
            println!("Failed to list feeds: {}", e);
        }
    }

    Ok(())
}

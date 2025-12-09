//! OPML import example for the RSS provider
//!
//! Run with: cargo run --package provider-rss --example opml_import

use provider_rss::{RssProvider, RssProviderConfig};
use scryforge_provider_core::prelude::*;

const SAMPLE_OPML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>Sample Subscriptions</title>
  </head>
  <body>
    <outline text="Technology">
      <outline text="Rust Blog" xmlUrl="https://blog.rust-lang.org/feed.xml"/>
      <outline text="GitHub Blog" xmlUrl="https://github.blog/feed/"/>
    </outline>
  </body>
</opml>"#;

#[tokio::main]
async fn main() -> Result<()> {
    println!("OPML Import Example");
    println!("===================");
    println!();

    // Parse OPML from a string
    println!("Parsing OPML...");
    match RssProviderConfig::from_opml_string(SAMPLE_OPML) {
        Ok(config) => {
            println!("Successfully parsed OPML!");
            println!("Found {} feeds:", config.feeds.len());
            for (idx, feed_url) in config.feeds.iter().enumerate() {
                println!("  {}. {}", idx + 1, feed_url);
            }
            println!();

            // Create provider with the imported feeds
            let provider = RssProvider::new(config);

            // List the feeds
            println!("Fetching feed metadata...");
            match provider.list_feeds().await {
                Ok(feeds) => {
                    println!("Feeds with metadata:");
                    for feed in feeds {
                        println!();
                        println!("  Name: {}", feed.name);
                        println!("  ID: {}", feed.id.0);
                        if let Some(desc) = feed.description {
                            println!("  Description: {}", desc);
                        }
                        if let Some(count) = feed.total_count {
                            println!("  Total items: {}", count);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to fetch feed metadata: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Failed to parse OPML: {}", e);
        }
    }

    println!();
    println!("To import from a file, use:");
    println!(
        "  let config = RssProviderConfig::from_opml(\"/path/to/subscriptions.opml\").await?;"
    );

    Ok(())
}

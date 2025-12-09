# provider-bookmarks

A provider for accessing local bookmarks stored in JSON format.

## Overview

The bookmarks provider manages bookmarks stored locally in a JSON file and implements both the `HasCollections` trait (for bookmark folders) and the `HasSavedItems` trait (for accessing all bookmarks). This allows you to organize bookmarks into folders and access them through the Scryforge TUI.

## Features

- **Local JSON Storage**: Bookmarks are stored in a simple, readable JSON format
- **Folder Organization**: Organize bookmarks into hierarchical folders (e.g., `Development/Rust/Libraries`)
- **Tags Support**: Add tags to bookmarks for easy categorization and filtering
- **Metadata**: Store descriptions and additional metadata with each bookmark
- **Auto-creation**: The bookmarks file is automatically created if it doesn't exist

## Storage Location

By default, bookmarks are stored at:
```
$XDG_DATA_HOME/scryforge/bookmarks.json
```

If `XDG_DATA_HOME` is not set, it defaults to:
```
~/.local/share/scryforge/bookmarks.json
```

## JSON Schema

Bookmarks are stored in the following format:

```json
{
  "bookmarks": [
    {
      "id": "uuid-v4",
      "url": "https://example.com",
      "title": "Example Site",
      "description": "Optional description",
      "tags": ["tag1", "tag2"],
      "folder": "Category/Subcategory",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

### Fields

- **id** (string, required): Unique identifier (UUID v4)
- **url** (string, required): The bookmark URL
- **title** (string, required): Display title for the bookmark
- **description** (string, optional): Detailed description or notes
- **tags** (array, optional): List of tags for categorization
- **folder** (string, optional): Folder path using `/` as separator (e.g., `Development/Rust`)
- **created_at** (datetime, required): ISO 8601 timestamp of creation
- **updated_at** (datetime, required): ISO 8601 timestamp of last update

## Usage

### Creating a Provider Instance

```rust
use provider_bookmarks::{BookmarksProvider, BookmarksConfig};

// Use default configuration
let provider = BookmarksProvider::with_default_config().await?;

// Or use custom path
let config = BookmarksConfig::new("/path/to/bookmarks.json".into());
let provider = BookmarksProvider::new(config).await?;
```

### Accessing Bookmarks

```rust
use fusabi_streams_core::prelude::*;

// List all bookmark folders
let collections = provider.list_collections().await?;
for collection in collections {
    println!("Folder: {} ({} items)", collection.name, collection.item_count);
}

// Get bookmarks in a specific folder
let folder_id = CollectionId("Development/Rust".to_string());
let items = provider.get_collection_items(&folder_id).await?;

// Get all saved bookmarks
let options = SavedItemsOptions::default();
let all_bookmarks = provider.get_saved_items(options).await?;

// Get bookmarks with pagination
let options = SavedItemsOptions {
    limit: Some(10),
    offset: Some(0),
    category: None,
};
let bookmarks = provider.get_saved_items(options).await?;

// Filter by folder
let options = SavedItemsOptions {
    category: Some("Development/Rust".to_string()),
    ..Default::default()
};
let rust_bookmarks = provider.get_saved_items(options).await?;
```

### Provider Capabilities

The bookmarks provider supports:

```rust
let caps = provider.capabilities();
assert!(caps.has_collections);  // Supports folder organization
assert!(caps.has_saved_items);  // Supports saved items list
assert!(!caps.has_feeds);       // No feed support
assert!(!caps.has_communities); // No community support
```

## Example Bookmarks File

See `example-bookmarks.json` for a sample bookmarks file with various folder structures and tags.

## Future Enhancements

Planned features for future releases:

- **Browser Import**: Import bookmarks from Chrome, Firefox, Safari, and Edge
- **Write Operations**: Add, edit, and delete bookmarks programmatically
- **Search**: Full-text search across titles, descriptions, and tags
- **Sync**: Optional sync with browser bookmarks
- **Export**: Export bookmarks to HTML, JSON, or Markdown formats
- **Deduplication**: Detect and merge duplicate bookmarks

## Testing

Run the test suite:

```bash
cargo test -p provider-bookmarks
```

All tests use temporary files and do not affect your actual bookmarks.

## Implementation Details

### Traits Implemented

- **`Provider`**: Base provider trait with health checks and sync operations
- **`HasCollections`**: Supports listing folders and retrieving bookmarks by folder
- **`HasSavedItems`**: Supports retrieving all bookmarks with pagination and filtering

### Thread Safety

The provider uses `tokio::sync::RwLock` for thread-safe access to the bookmark store, allowing concurrent reads while ensuring exclusive access for writes.

### Error Handling

All operations return proper error types that integrate with the `fusabi_streams_core::StreamError` type system.

## License

MIT OR Apache-2.0

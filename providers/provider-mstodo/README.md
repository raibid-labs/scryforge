# provider-mstodo

Microsoft To Do provider for Scryforge.

## Overview

This provider integrates Microsoft To Do tasks with Scryforge, allowing you to view and manage your tasks through the TUI. It uses the Microsoft Graph API to access task lists and individual tasks.

## Features

- **Collections**: Access your task lists as collections
- **Virtual Feeds**: View tasks through smart feeds:
  - "Due Today" - Tasks due today
  - "Important" - High-priority tasks
  - "Planned" - Tasks with due dates
  - "All Tasks" - All tasks across all lists

## Authentication

This provider requires an OAuth 2.0 access token with the following Microsoft Graph API permissions:

- `Tasks.Read` - Read user tasks
- `Tasks.ReadWrite` - Read and write user tasks (for future write operations)

The access token is provided via the `MsTodoConfig` configuration.

## Usage

```rust
use provider_mstodo::{MsTodoProvider, MsTodoConfig};
use fusabi_streams_core::prelude::*;

// Create configuration with access token
let config = MsTodoConfig::new("your_access_token".to_string());

// Create provider
let provider = MsTodoProvider::new(config);

// List task lists (collections)
let collections = provider.list_collections().await?;

// Get tasks from a specific list
let tasks = provider.get_collection_items(&collection_id).await?;

// List virtual feeds
let feeds = provider.list_feeds().await?;

// Get tasks from a feed (e.g., due today)
let feed_id = FeedId("mstodo:due-today".to_string());
let options = FeedOptions::default();
let items = provider.get_feed_items(&feed_id, options).await?;
```

## Task Item Mapping

Microsoft To Do tasks are mapped to Scryforge `Item` structs with the following properties:

- **title**: Task title
- **content**: `ItemContent::Task` with:
  - `body`: Task notes/description
  - `due_date`: Task due date (if set)
  - `is_completed`: Task completion status
- **url**: Link to task in Microsoft To Do web app
- **is_read**: Set to `true` for completed tasks
- **tags**: Task categories
- **metadata**: Additional task properties:
  - `status`: Task status (notStarted, inProgress, completed, etc.)
  - `importance`: Task importance (low, normal, high)
  - `is_reminder_on`: Whether reminder is enabled
  - `due_date_time`: Full due date/time with timezone
  - `completed_date_time`: Completion timestamp
  - `reminder_date_time`: Reminder date/time

## API Endpoints

The provider uses the following Microsoft Graph API endpoints:

- `GET /me/todo/lists` - List all task lists
- `GET /me/todo/lists/{id}/tasks` - Get tasks in a specific list

## Testing

Run the test suite:

```bash
cargo test -p provider-mstodo
```

The tests include:
- Provider creation and configuration
- Task-to-Item conversion
- Feed listing
- Collection operations
- Action availability based on task state
- Graph API response deserialization

## Future Enhancements

Planned features for future releases:

- Task creation, update, and deletion
- Calendar event integration
- Task recurrence support
- Subtask/checklist support
- Attachment handling
- Real-time sync via webhooks

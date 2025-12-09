# provider-email-imap

IMAP email provider for Scryforge.

## Features

- TLS/SSL connection support (enabled by default)
- Password authentication
- Mailbox synchronization as feeds
- Folder listing as collections
- Email parsing with proper encoding handling
- Multipart message support (plain text preferred over HTML)
- Attachment metadata extraction

## Configuration

```rust
use provider_email_imap::{ImapConfig, ImapMailboxConfig, ImapProvider};

let config = ImapConfig {
    server: "imap.gmail.com".to_string(),
    port: 993,
    username: "user@gmail.com".to_string(),
    password: "app-password".to_string(),
    use_tls: true,
    mailboxes: vec![
        ImapMailboxConfig {
            id: "inbox".to_string(),
            name: "Inbox".to_string(),
            mailbox_name: "INBOX".to_string(),
            description: Some("Main inbox".to_string()),
            icon: Some("📥".to_string()),
        },
    ],
};

let provider = ImapProvider::new(config);
```

## Usage

The IMAP provider implements both `HasFeeds` and `HasCollections` traits:

- **Feeds**: Configured mailboxes (INBOX, Sent, etc.)
- **Collections**: All IMAP folders discovered via LIST command

### Fetching Emails as Feed Items

```rust
use fusabi_streams_core::prelude::*;

// List available feeds
let feeds = provider.list_feeds().await?;

// Fetch items from a feed
let feed_id = FeedId("inbox".to_string());
let options = FeedOptions {
    limit: Some(50),
    include_read: false,
    ..Default::default()
};
let items = provider.get_feed_items(&feed_id, options).await?;
```

### Browsing Folders as Collections

```rust
// List all folders
let collections = provider.list_collections().await?;

// Get items in a specific folder
let collection_id = CollectionId("Archive".to_string());
let items = provider.get_collection_items(&collection_id).await?;
```

## Item Mapping

Emails are converted to `Item` structs with the following mappings:

- **title**: Email subject
- **content**: `ItemContent::Email` with plain text and/or HTML body
- **author**: From header (name and email)
- **published**: Date header or internal date
- **metadata**: To, Cc, attachments list, message ID

## Edge Cases Handled

- Multipart messages (prefers plain text over HTML)
- Encoded headers and subjects
- Missing dates (falls back to internal date or current time)
- Attachments (extracted as metadata)
- Various IMAP folder types and attributes

## Future Enhancements

- OAuth2 authentication support
- Write operations (mark as read/unread, delete, archive)
- Better attachment handling
- Search functionality
- Push notifications via IDLE

## Dependencies

- `async-imap`: IMAP client library
- `async-native-tls`: TLS support
- `async-std`: Async runtime (required by async-imap)
- `mail-parser`: Email message parsing
- `futures`: Stream utilities

## Testing

Run tests with:

```bash
cargo test -p provider-email-imap
```

Note: Tests do not require a live IMAP server. They verify the provider structure, configuration, and email parsing logic.

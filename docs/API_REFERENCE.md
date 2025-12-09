# Scryforge JSON-RPC API Reference

This document provides complete documentation for the Scryforge daemon's JSON-RPC 2.0 API.

## Table of Contents

1. [Connection](#connection)
2. [Protocol](#protocol)
3. [Error Codes](#error-codes)
4. [Stream Methods](#stream-methods)
5. [Item Methods](#item-methods)
6. [Search Methods](#search-methods)
7. [Collection Methods](#collection-methods)
8. [Sync Methods](#sync-methods)
9. [Type Definitions](#type-definitions)

## Connection

The daemon exposes its API over a Unix socket (default) or TCP connection.

### Unix Socket

**Default Location**: `$XDG_RUNTIME_DIR/scryforge.sock`

**Fallback**: `/tmp/scryforge.sock`

**Example Connection** (using `nc`):
```bash
nc -U /run/user/1000/scryforge.sock
```

### TCP Socket (Optional)

**Default**: `localhost:7470`

**Example Connection**:
```bash
nc localhost 7470
```

### Client Libraries

For Rust applications, use the `daemon_client.rs` module in `scryforge-tui`.

## Protocol

Scryforge uses JSON-RPC 2.0 for all API communication.

### Request Format

```json
{
  "jsonrpc": "2.0",
  "method": "method.name",
  "params": [...],
  "id": 1
}
```

### Response Format (Success)

```json
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}
```

### Response Format (Error)

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": 1
}
```

### Batch Requests

Multiple requests can be sent in a single batch:

```json
[
  {"jsonrpc": "2.0", "method": "streams.list", "id": 1},
  {"jsonrpc": "2.0", "method": "sync.status", "id": 2}
]
```

Responses will be returned in the same order.

## Error Codes

Standard JSON-RPC error codes plus application-specific codes:

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid request | Invalid JSON-RPC format |
| -32601 | Method not found | Method does not exist |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Internal server error |
| -32000 | Server error | Generic application error |
| -32001 | Resource not available | Cache or sync manager unavailable |
| -32002 | Invalid ID format | ID does not follow expected format |
| -32003 | Not found | Provider, stream, or item not found |
| -32004 | Not supported | Operation not supported by provider |
| -32005 | Not implemented | Capability not implemented |

## Stream Methods

### `streams.list`

List all available streams across all providers.

**Method**: `streams.list`

**Parameters**: None

**Returns**: `Stream[]`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "streams.list",
  "params": [],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "id": "email:inbox:gmail",
      "name": "Gmail Inbox",
      "provider_id": "email-imap",
      "stream_type": "Feed",
      "icon": "📧",
      "unread_count": 5,
      "total_count": 150,
      "last_updated": "2025-01-15T10:30:00Z",
      "metadata": {}
    },
    {
      "id": "rss:feed:hackernews",
      "name": "Hacker News",
      "provider_id": "rss",
      "stream_type": "Feed",
      "icon": "📰",
      "unread_count": 42,
      "total_count": 100,
      "last_updated": "2025-01-15T10:25:00Z",
      "metadata": {}
    }
  ],
  "id": 1
}
```

## Item Methods

### `items.list`

List items for a specific stream.

**Method**: `items.list`

**Parameters**:
- `stream_id` (string, required): Stream identifier

**Returns**: `Item[]`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "items.list",
  "params": ["email:inbox:gmail"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "id": "email:msg-001",
      "stream_id": "email:inbox:gmail",
      "title": "Meeting tomorrow at 10am",
      "content": {
        "Email": {
          "subject": "Meeting tomorrow at 10am",
          "body_text": "Hi,\n\nJust a reminder...",
          "body_html": null,
          "snippet": "Just a reminder about our meeting..."
        }
      },
      "author": {
        "name": "John Doe",
        "email": "john@example.com",
        "url": null,
        "avatar_url": null
      },
      "published": "2025-01-15T09:00:00Z",
      "updated": null,
      "url": null,
      "thumbnail_url": null,
      "is_read": false,
      "is_saved": false,
      "tags": [],
      "metadata": {}
    }
  ],
  "id": 1
}
```

### `items.mark_read`

Mark an item as read.

**Method**: `items.mark_read`

**Parameters**:
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "items.mark_read",
  "params": ["email:msg-001"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": null,
  "id": 1
}
```

### `items.mark_unread`

Mark an item as unread.

**Method**: `items.mark_unread`

**Parameters**:
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example**: Same as `items.mark_read` but with `items.mark_unread` method.

### `items.save`

Save/bookmark an item.

**Method**: `items.save`

**Parameters**:
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "items.save",
  "params": ["rss:article-001"],
  "id": 1
}
```

### `items.unsave`

Remove bookmark from an item.

**Method**: `items.unsave`

**Parameters**:
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

### `items.archive`

Archive an item.

**Method**: `items.archive`

**Parameters**:
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "items.archive",
  "params": ["email:msg-001"],
  "id": 1
}
```

## Search Methods

### `search.query`

Search items across all streams or within a specific stream.

**Method**: `search.query`

**Parameters**:
- `query` (string, required): Search query text
- `filters` (object, optional): Filter criteria

**Filter Object**:
```typescript
{
  stream_id?: string,      // Filter by specific stream
  content_type?: string,   // Filter by content type (e.g., "Email", "Article")
  is_read?: boolean,       // Filter by read status
  is_saved?: boolean       // Filter by saved status
}
```

**Returns**: `Item[]` (up to 100 results)

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "search.query",
  "params": [
    "rust programming",
    {
      "is_read": false,
      "content_type": "Article"
    }
  ],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "id": "rss:article-001",
      "stream_id": "rss:feed:hackernews",
      "title": "Show HN: A new Rust TUI framework",
      "content": {
        "Article": {
          "summary": "I've been working on a new TUI framework in Rust...",
          "full_content": null
        }
      },
      "author": {
        "name": "rustdev",
        "email": null,
        "url": null,
        "avatar_url": null
      },
      "published": "2025-01-15T08:00:00Z",
      "updated": null,
      "url": "https://news.ycombinator.com/item?id=123",
      "thumbnail_url": null,
      "is_read": false,
      "is_saved": false,
      "tags": [],
      "metadata": {}
    }
  ],
  "id": 1
}
```

## Collection Methods

### `collections.list`

List all collections across all providers.

**Method**: `collections.list`

**Parameters**: None

**Returns**: `Collection[]`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "collections.list",
  "params": [],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "id": "dummy:playlist-1",
      "name": "My Favorites",
      "description": "A dummy collection of favorite items",
      "icon": "⭐",
      "item_count": 2,
      "is_editable": true,
      "owner": "dummy_user"
    },
    {
      "id": "spotify:playlist:abcd1234",
      "name": "Chill Vibes",
      "description": null,
      "icon": "🎵",
      "item_count": 45,
      "is_editable": true,
      "owner": "spotify_user"
    }
  ],
  "id": 1
}
```

### `collections.items`

Get items in a specific collection.

**Method**: `collections.items`

**Parameters**:
- `collection_id` (string, required): Collection identifier

**Returns**: `Item[]`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "collections.items",
  "params": ["dummy:playlist-1"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "id": "dummy:item-1",
      "stream_id": "dummy:collection:dummy:playlist-1",
      "title": "Collection Item 1",
      "content": {
        "Text": "This is item dummy:item-1 in the collection"
      },
      "author": null,
      "published": "2025-01-15T10:30:00Z",
      "updated": null,
      "url": "https://example.com/dummy:item-1",
      "thumbnail_url": null,
      "is_read": false,
      "is_saved": true,
      "tags": ["collection"],
      "metadata": {}
    }
  ],
  "id": 1
}
```

### `collections.add_item`

Add an item to a collection.

**Method**: `collections.add_item`

**Parameters**:
- `collection_id` (string, required): Collection identifier
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "collections.add_item",
  "params": ["dummy:playlist-1", "dummy:item-3"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": null,
  "id": 1
}
```

### `collections.remove_item`

Remove an item from a collection.

**Method**: `collections.remove_item`

**Parameters**:
- `collection_id` (string, required): Collection identifier
- `item_id` (string, required): Item identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "collections.remove_item",
  "params": ["dummy:playlist-1", "dummy:item-1"],
  "id": 1
}
```

### `collections.create`

Create a new collection.

**Method**: `collections.create`

**Parameters**:
- `name` (string, required): Collection name

**Returns**: `Collection`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "collections.create",
  "params": ["My Reading List"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "id": "dummy:collection-3",
    "name": "My Reading List",
    "description": "User-created collection: My Reading List",
    "icon": "📁",
    "item_count": 0,
    "is_editable": true,
    "owner": "dummy_user"
  },
  "id": 1
}
```

## Sync Methods

### `sync.status`

Get sync status for all providers.

**Method**: `sync.status`

**Parameters**: None

**Returns**: `Map<String, ProviderSyncState>`

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "sync.status",
  "params": [],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "dummy": {
      "provider_id": "dummy",
      "is_syncing": false,
      "last_sync": "2025-01-15T10:00:00Z",
      "last_success": "2025-01-15T10:00:00Z",
      "last_error": null,
      "items_synced": 10,
      "next_sync": "2025-01-15T10:05:00Z"
    },
    "rss": {
      "provider_id": "rss",
      "is_syncing": true,
      "last_sync": "2025-01-15T10:02:00Z",
      "last_success": "2025-01-15T10:02:00Z",
      "last_error": null,
      "items_synced": 42,
      "next_sync": "2025-01-15T10:07:00Z"
    }
  },
  "id": 1
}
```

### `sync.trigger`

Manually trigger a sync for a specific provider.

**Method**: `sync.trigger`

**Parameters**:
- `provider_id` (string, required): Provider identifier

**Returns**: `null` (success) or error

**Example Request**:
```json
{
  "jsonrpc": "2.0",
  "method": "sync.trigger",
  "params": ["rss"],
  "id": 1
}
```

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "result": null,
  "id": 1
}
```

## Type Definitions

### Stream

```typescript
{
  id: string,                    // Format: "provider:type:local_id"
  name: string,                  // Display name
  provider_id: string,           // Provider that owns this stream
  stream_type: StreamType,       // "Feed" | "Collection" | "SavedItems" | "Community" | {"Custom": string}
  icon: string | null,           // Icon emoji or path
  unread_count: number | null,   // Number of unread items
  total_count: number | null,    // Total number of items
  last_updated: string | null,   // ISO 8601 timestamp
  metadata: object               // Provider-specific metadata
}
```

### Item

```typescript
{
  id: string,                    // Format: "provider:item_id"
  stream_id: string,             // Parent stream
  title: string,                 // Item title
  content: ItemContent,          // Content variant (see below)
  author: Author | null,         // Author information
  published: string | null,      // ISO 8601 timestamp
  updated: string | null,        // ISO 8601 timestamp
  url: string | null,            // External URL
  thumbnail_url: string | null,  // Thumbnail image URL
  is_read: boolean,              // Read status
  is_saved: boolean,             // Saved/bookmarked status
  tags: string[],                // Tags
  metadata: object               // Provider-specific metadata
}
```

### ItemContent

One of the following variants:

#### Text
```typescript
{"Text": string}
```

#### Markdown
```typescript
{"Markdown": string}
```

#### Html
```typescript
{"Html": string}
```

#### Email
```typescript
{
  "Email": {
    subject: string,
    body_text: string | null,
    body_html: string | null,
    snippet: string
  }
}
```

#### Article
```typescript
{
  "Article": {
    summary: string | null,
    full_content: string | null
  }
}
```

#### Video
```typescript
{
  "Video": {
    description: string,
    duration_seconds: number | null,
    view_count: number | null
  }
}
```

#### Track
```typescript
{
  "Track": {
    album: string | null,
    duration_ms: number | null,
    artists: string[]
  }
}
```

#### Task
```typescript
{
  "Task": {
    body: string | null,
    due_date: string | null,    // YYYY-MM-DD
    is_completed: boolean
  }
}
```

#### Event
```typescript
{
  "Event": {
    description: string | null,
    start: string,              // ISO 8601 timestamp
    end: string,                // ISO 8601 timestamp
    location: string | null,
    is_all_day: boolean
  }
}
```

#### Bookmark
```typescript
{
  "Bookmark": {
    description: string | null
  }
}
```

#### Generic
```typescript
{
  "Generic": {
    body: string | null
  }
}
```

### Author

```typescript
{
  name: string,
  email: string | null,
  url: string | null,
  avatar_url: string | null
}
```

### Collection

```typescript
{
  id: string,                    // Format: "provider:collection_id"
  name: string,                  // Collection name
  description: string | null,    // Description
  icon: string | null,           // Icon emoji or path
  item_count: number,            // Number of items
  is_editable: boolean,          // Can add/remove items
  owner: string | null           // Owner identifier
}
```

### ProviderSyncState

```typescript
{
  provider_id: string,           // Provider identifier
  is_syncing: boolean,           // Currently syncing
  last_sync: string | null,      // ISO 8601 timestamp of last sync
  last_success: string | null,   // ISO 8601 timestamp of last successful sync
  last_error: string | null,     // Last error message
  items_synced: number,          // Total items synced
  next_sync: string | null       // ISO 8601 timestamp of next scheduled sync
}
```

## Client Implementation Examples

### JavaScript/TypeScript

```typescript
import net from 'net';

class ScryforgeClient {
  private socket: net.Socket;
  private requestId = 0;

  constructor(socketPath: string) {
    this.socket = net.createConnection(socketPath);
  }

  async request(method: string, params: any[] = []): Promise<any> {
    return new Promise((resolve, reject) => {
      const id = ++this.requestId;
      const request = JSON.stringify({
        jsonrpc: '2.0',
        method,
        params,
        id
      });

      this.socket.write(request + '\n');

      this.socket.once('data', (data) => {
        const response = JSON.parse(data.toString());
        if (response.error) {
          reject(new Error(response.error.message));
        } else {
          resolve(response.result);
        }
      });
    });
  }

  async listStreams(): Promise<Stream[]> {
    return this.request('streams.list');
  }

  async listItems(streamId: string): Promise<Item[]> {
    return this.request('items.list', [streamId]);
  }

  async search(query: string, filters: object = {}): Promise<Item[]> {
    return this.request('search.query', [query, filters]);
  }
}
```

### Python

```python
import socket
import json

class ScryforgeClient:
    def __init__(self, socket_path):
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(socket_path)
        self.request_id = 0

    def request(self, method, params=None):
        self.request_id += 1
        request = {
            'jsonrpc': '2.0',
            'method': method,
            'params': params or [],
            'id': self.request_id
        }

        self.socket.send((json.dumps(request) + '\n').encode())

        response = json.loads(self.socket.recv(4096).decode())

        if 'error' in response:
            raise Exception(response['error']['message'])

        return response['result']

    def list_streams(self):
        return self.request('streams.list')

    def list_items(self, stream_id):
        return self.request('items.list', [stream_id])

    def search(self, query, filters=None):
        return self.request('search.query', [query, filters or {}])
```

### Rust

```rust
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Serialize)]
struct Request {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

#[derive(Deserialize)]
struct Response<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<ErrorObject>,
    id: u64,
}

#[derive(Deserialize)]
struct ErrorObject {
    code: i32,
    message: String,
}

pub struct ScryforgeClient {
    stream: UnixStream,
    next_id: u64,
}

impl ScryforgeClient {
    pub async fn connect(socket_path: &str) -> Result<Self, std::io::Error> {
        let stream = UnixStream::connect(socket_path).await?;
        Ok(Self { stream, next_id: 1 })
    }

    pub async fn request<T: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, String> {
        let request = Request {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: self.next_id,
        };
        self.next_id += 1;

        let request_json = serde_json::to_string(&request).unwrap();
        self.stream.write_all(request_json.as_bytes()).await.unwrap();
        self.stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(&mut self.stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: Response<T> = serde_json::from_str(&response_line)
            .map_err(|e| format!("Parse error: {}", e))?;

        if let Some(error) = response.error {
            return Err(error.message);
        }

        response.result.ok_or_else(|| "No result".to_string())
    }

    pub async fn list_streams(&mut self) -> Result<Vec<Stream>, String> {
        self.request("streams.list", json!([])).await
    }

    pub async fn list_items(&mut self, stream_id: &str) -> Result<Vec<Item>, String> {
        self.request("items.list", json!([stream_id])).await
    }
}
```

## Rate Limiting

The API does not currently impose rate limits, but clients should:

1. Avoid polling methods excessively (use reasonable intervals)
2. Batch requests when possible
3. Handle backpressure gracefully

Future versions may add rate limiting headers in responses.

## Versioning

The API version is included in responses (planned):

```json
{
  "jsonrpc": "2.0",
  "result": ...,
  "id": 1,
  "api_version": "1.0"
}
```

Breaking changes will increment the major version.

## Resources

- [Architecture Documentation](./ARCHITECTURE.md)
- [Provider Development Guide](./PROVIDER_DEVELOPMENT.md)
- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [API Handler Source](../scryforge-daemon/src/api/handlers.rs)

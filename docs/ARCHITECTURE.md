# Scryforge Architecture

This document describes the technical architecture of Scryforge, including workspace layout, crate responsibilities, the daemon API, plugin system, and data flow.

## Workspace Layout

```
scryforge/
├── Cargo.toml                          # Workspace root with shared dependencies
│
├── crates/                             # Core library crates
│   ├── scryforge-provider-core/        # Provider traits and types
│   ├── fusabi-runtime/                 # Plugin runtime and bytecode loader
│   ├── fusabi-plugin-api/              # Plugin API types
│   ├── fusabi-tui-core/                # TUI framework primitives
│   └── fusabi-tui-widgets/             # Reusable TUI widgets
│
├── scryforge-daemon/                   # The hub daemon binary
├── scryforge-tui/                      # The TUI client binary
├── scryforge-sigilforge-client/        # Auth client library
│
├── providers/                          # Provider implementations
│   ├── provider-dummy/                 # Test/demo provider
│   ├── provider-bookmarks/             # Local bookmarks provider
│   ├── provider-email-imap/            # IMAP email provider
│   ├── provider-mstodo/                # Microsoft To Do provider
│   ├── provider-reddit/                # Reddit provider
│   ├── provider-rss/                   # RSS/Atom feed provider
│   ├── provider-spotify/               # Spotify provider
│   └── provider-youtube/               # YouTube provider
│
└── docs/                               # Documentation
```

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        External Services                         │
│  (Gmail, RSS, Spotify, Reddit, YouTube, MS To Do, etc.)         │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     │ OAuth tokens via
                     ▼
        ┌────────────────────────┐
        │   Sigilforge Daemon    │ (Separate auth service)
        │   (Unix socket)        │
        └────────────┬───────────┘
                     │
                     │ Token fetch requests
                     ▼
        ┌────────────────────────────────────────────────┐
        │          scryforge-daemon                       │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │         Provider Registry                 │  │
        │  │  ┌────────────┐  ┌────────────┐         │  │
        │  │  │ Provider 1 │  │ Provider 2 │  ...    │  │
        │  │  └────────────┘  └────────────┘         │  │
        │  └──────────────────────────────────────────┘  │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │            Plugin Manager                 │  │
        │  │  (Fusabi Runtime for .fzb plugins)       │  │
        │  └──────────────────────────────────────────┘  │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │          Cache Layer                      │  │
        │  │  (SQLite: streams, items, sync state)    │  │
        │  └──────────────────────────────────────────┘  │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │         JSON-RPC API Server               │  │
        │  │  (Unix socket or TCP)                     │  │
        │  └──────────────────────────────────────────┘  │
        └─────────────────┬──────────────────────────────┘
                          │
                          │ JSON-RPC calls
                          ▼
        ┌────────────────────────────────────────────────┐
        │          scryforge-tui                          │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │      Daemon Client (RPC)                  │  │
        │  └──────────────────────────────────────────┘  │
        │                                                 │
        │  ┌──────────────────────────────────────────┐  │
        │  │      TUI Components                       │  │
        │  │  - Stream List                            │  │
        │  │  - Item List                              │  │
        │  │  - Preview Pane                           │  │
        │  │  - Status Bar                             │  │
        │  └──────────────────────────────────────────┘  │
        └────────────────────────────────────────────────┘
```

## Crate Responsibilities

### `scryforge-provider-core`

The foundational crate defining the data model and provider capabilities:

**Core Types**:
- `StreamId`, `ItemId`, `FeedId`, `CollectionId` - Unique identifiers
- `Stream` - A logical feed or collection (inbox, playlist, subreddit)
- `Item` - An entry within a stream (email, article, video, track)
- `ItemContent` - Content variants (Email, Article, Video, Track, Task, etc.)
- `Action` - Operations that can be performed on items
- `Author` - Creator/sender information

**Base Trait**:
- `Provider` - Base trait all providers must implement
  - `id()`, `name()` - Identification
  - `health_check()` - Connectivity status
  - `sync()` - Trigger data synchronization
  - `capabilities()` - Declare supported capabilities
  - `available_actions()`, `execute_action()` - Action system

**Capability Traits**:
- `HasFeeds` - Providers with time-based streams
  - `list_feeds()` - Get available feeds
  - `get_feed_items()` - Fetch items from a feed
- `HasCollections` - Providers with named, ordered sets
  - `list_collections()` - Get collections
  - `get_collection_items()` - Fetch collection contents
  - `add_to_collection()`, `remove_from_collection()` - Manage items
  - `create_collection()` - Create new collections
- `HasSavedItems` - Providers with bookmarked/liked items
  - `get_saved_items()` - Fetch all saved items
  - `is_saved()`, `save_item()`, `unsave_item()` - Manage saved state
- `HasCommunities` - Providers with subscriptions
  - `list_communities()` - Get subscribed communities
  - `get_community()` - Fetch community details
- `HasTasks` - Providers with task management
  - `complete_task()`, `uncomplete_task()` - Task completion

**Authentication Support** (optional `sigilforge` feature):
- `auth` module re-exports Sigilforge client types
- `TokenFetcher` trait for OAuth token retrieval

### `scryforge-daemon`

The hub daemon managing all data flow:

**Provider Registry** (`registry.rs`):
- `ProviderRegistry` - Central registry for all providers
- Stores providers as `Arc<dyn Provider>` trait objects
- Operations: register, get, list, remove, clear

**Cache Layer** (`cache/mod.rs`):
- `Cache` trait - Abstract cache interface
- `SqliteCache` - SQLite-based implementation
- Schema: streams, items, sync_state
- Operations: upsert_streams, upsert_items, get_items, search_items
- State tracking: mark_read, mark_starred, mark_archived

**Sync Manager** (`sync.rs`):
- Periodic synchronization of providers
- Tracks sync state per provider
- Manages sync intervals and error handling

**Plugin Manager** (`plugin/manager.rs`):
- `PluginManager` - Manages Fusabi plugins
- Plugin discovery from well-known paths
- Integration with ProviderRegistry

**JSON-RPC API Server** (`api/`):
- `ScryforgeApi` trait - RPC method definitions
- `ApiImpl` - Implementation with cache and sync manager
- Transport: Unix socket (default) or TCP

**Configuration** (`config.rs`):
- Daemon settings (socket path, sync intervals)
- Provider configuration loading
- XDG directory integration

### `scryforge-sigilforge-client`

Client library for OAuth token management:

**Core Types**:
- `SigilforgeClient` - Unix socket client for Sigilforge daemon
- `TokenFetcher` trait - Abstraction for token retrieval
- `MockTokenFetcher` - Testing implementation

**Operations**:
- `get_token(service, account)` - Fetch OAuth token
- `resolve(reference)` - Resolve credential reference
- Connection management and error handling

**Integration**:
- Providers use `TokenFetcher` to request credentials
- No direct credential storage in Scryforge
- All auth delegated to Sigilforge daemon

### `fusabi-runtime`

Plugin runtime for loading and executing Fusabi plugins:

**Plugin Discovery** (`discovery.rs`):
- Scan well-known paths for plugins
- Plugin directory structure validation

**Manifest Parsing** (`manifest.rs`):
- `PluginManifest` - Plugin metadata and configuration
- `PluginMetadata` - ID, name, version, authors, license
- `ProviderConfig` - Provider-specific settings
- `RateLimitConfig` - API rate limiting
- Capability declarations

**Bytecode Loader** (`bytecode.rs`):
- `Bytecode` - Bytecode representation
- `BytecodeLoader` - Load and validate .fzb files
- Format: Magic bytes (FZB\x01) + JSON or binary
- Instruction set: LoadConst, Call, Jump, Add, etc.

**Capability System** (`capability.rs`):
- `Capability` enum - Permission types
- Capabilities: Network, FileRead, FileWrite, Credentials, CacheRead, etc.
- `CapabilitySet` - Collection of required capabilities
- Runtime enforcement (future)

### `fusabi-tui-core` & `fusabi-tui-widgets`

TUI framework components (to be implemented):

- Event loop abstraction
- State management primitives
- Reusable widgets (StreamList, ItemList, PreviewPane, etc.)

### `scryforge-tui`

The terminal user interface:

**Daemon Client** (`daemon_client.rs`):
- JSON-RPC client connecting to daemon
- Async request/response handling
- Error recovery

**UI Components**:
- Stream browsing and navigation
- Item list with filtering
- Item preview and actions
- Status display and notifications

## Data Flow

### Synchronization Flow

```
1. Daemon Periodic Sync
   ┌──────────────┐
   │ Sync Manager │
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │  Provider    │──────► Fetch from external API
   │   .sync()    │◄────── (using OAuth token)
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │    Cache     │──────► Upsert streams
   │  .upsert_*() │        Upsert items
   └──────────────┘        Update sync state

2. TUI Request Flow
   ┌──────────────┐
   │     TUI      │──────► streams.list (JSON-RPC)
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │ API Handler  │──────► Query cache
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │    Cache     │──────► Return cached data
   │  .get_*()    │
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │     TUI      │◄────── Response (JSON-RPC)
   └──────────────┘

3. Action Execution Flow
   ┌──────────────┐
   │     TUI      │──────► items.mark_read (JSON-RPC)
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │ API Handler  │──────► Update cache
   └──────┬───────┘        mark_read(item_id, true)
          │
          ▼
   ┌──────────────┐
   │  Provider    │──────► Sync to external service
   │ (future)     │        (Phase 4+)
   └──────────────┘
```

### Provider Registration Flow

```
Daemon Startup
   │
   ▼
┌──────────────────┐
│ Create Registry  │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Load Config      │──────► Read provider configurations
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Register Native  │──────► provider_dummy::DummyProvider
│ Providers        │        provider_rss::RssProvider
└────────┬─────────┘        etc.
         │
         ▼
┌──────────────────┐
│ Discover Plugins │──────► Scan ~/.local/share/scryforge/plugins/
│ (Fusabi)         │        Load manifests
└────────┬─────────┘        Load bytecode
         │
         ▼
┌──────────────────┐
│ Register Plugin  │──────► Create PluginProvider wrappers
│ Providers        │        Add to registry
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Health Check All │──────► Verify connectivity
└────────┬─────────┘        Log status
         │
         ▼
┌──────────────────┐
│ Start API Server │
└──────────────────┘
```

## Daemon JSON-RPC API

The daemon exposes a JSON-RPC 2.0 API over Unix socket (default) or TCP.

**Transport**:
- Unix socket: `$XDG_RUNTIME_DIR/scryforge.sock`
- TCP: `localhost:7470` (optional)

**Available Methods**:
```
streams.list() -> Stream[]
items.list(stream_id: String) -> Item[]
items.mark_read(item_id: String) -> ()
items.mark_unread(item_id: String) -> ()
items.save(item_id: String) -> ()
items.unsave(item_id: String) -> ()
items.archive(item_id: String) -> ()
search.query(query: String, filters: Object) -> Item[]
collections.list() -> Collection[]
collections.items(collection_id: String) -> Item[]
collections.add_item(collection_id: String, item_id: String) -> ()
collections.remove_item(collection_id: String, item_id: String) -> ()
collections.create(name: String) -> Collection
sync.status() -> Map<String, ProviderSyncState>
sync.trigger(provider_id: String) -> ()
```

See [API_REFERENCE.md](./API_REFERENCE.md) for complete documentation.

## Authentication via Sigilforge

Scryforge does NOT manage OAuth tokens, API keys, or credentials directly. All authentication is delegated to **Sigilforge**, a separate daemon.

### Auth Reference Format

Providers reference credentials using service/account pairs:

```
spotify/default
gmail/work
reddit/personal
microsoft/main
```

### Integration Pattern

```rust
use scryforge_provider_core::auth::{TokenFetcher, SigilforgeClient};

pub struct SpotifyProvider {
    token_fetcher: Arc<dyn TokenFetcher>,
}

impl SpotifyProvider {
    pub async fn make_api_call(&self) -> Result<Response> {
        // Request token from Sigilforge
        let token = self.token_fetcher
            .fetch_token("spotify", "personal")
            .await?;

        // Use token for API call
        let client = SpotifyClient::with_token(token);
        client.get_playlists().await
    }
}
```

### Sigilforge Responsibilities

- Store encrypted credentials
- Handle OAuth flows (redirect URI, PKCE)
- Automatic token refresh
- Multi-account support per service
- Credential resolution

## Plugin System (Fusabi)

### Plugin Structure

Plugins are directories containing:

```
~/.local/share/scryforge/plugins/my-plugin/
├── manifest.toml       # Plugin metadata and capabilities
└── plugin.fzb          # Compiled Fusabi bytecode
```

### Manifest Format

```toml
[plugin]
id = "my-provider"
name = "My Provider"
version = "1.0.0"
description = "A custom provider"
authors = ["Author Name"]
plugin_type = "provider"

[provider]
id = "my-provider"
display_name = "My Provider"
has_feeds = true
has_collections = false
oauth_provider = "myservice"

capabilities = ["network", "credentials"]

[rate_limit]
requests_per_second = 10.0
max_concurrent = 5
```

### Capability System

Plugins must declare required capabilities:

- `network` - HTTP/HTTPS requests
- `file_read`, `file_write` - Filesystem access
- `credentials` - Access to OAuth tokens
- `cache_read`, `cache_write` - Scryforge cache access
- `environment` - Environment variables
- `process` - Subprocess spawning
- `notifications` - User notifications
- `clipboard` - Clipboard access
- `open_url` - Open URLs in browser

The runtime enforces that plugins only use declared capabilities.

### Bytecode Format

Fusabi bytecode (.fzb) uses a simple format:

```
+----------------+
| Magic (4 bytes)|  "FZB\x01"
+----------------+
| Metadata       |  JSON: plugin_id, version, etc.
+----------------+
| Constants      |  Constant pool
+----------------+
| Functions      |  Function definitions
+----------------+
| Instructions   |  Bytecode instructions
+----------------+
```

Currently supports JSON encoding for development. Binary format planned for production.

## Crate Dependency Graph

```
scryforge-tui
    └─► scryforge-provider-core
    └─► fusabi-tui-core
        └─► fusabi-tui-widgets

scryforge-daemon
    └─► scryforge-provider-core
    └─► scryforge-sigilforge-client
    └─► fusabi-runtime
        └─► fusabi-plugin-api
    └─► provider-dummy
    └─► provider-rss
    └─► provider-* (other providers)

provider-*
    └─► scryforge-provider-core
    └─► scryforge-sigilforge-client (if auth required)

fusabi-runtime
    └─► fusabi-plugin-api
```

## Configuration

Configuration is stored in `$XDG_CONFIG_HOME/scryforge/`:

```
scryforge/
├── config.toml           # Main configuration
├── providers/            # Per-provider configuration
│   ├── dummy.toml
│   ├── rss.toml
│   └── spotify.toml
└── plugins/              # Plugin configuration
    └── my-plugin.toml
```

### Example `config.toml`

```toml
[daemon]
socket_path = "/run/user/1000/scryforge.sock"
sync_interval_secs = 300
data_dir = "~/.local/share/scryforge"

[sigilforge]
socket_path = "/run/user/1000/sigilforge.sock"

[providers]
enabled = ["dummy", "rss", "spotify", "youtube"]

[tui]
theme = "default"
```

## Database Schema

The cache uses SQLite with the following schema:

### `streams` table
```sql
CREATE TABLE streams (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    stream_type TEXT NOT NULL,
    icon TEXT,
    unread_count INTEGER,
    total_count INTEGER,
    last_updated TEXT,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### `items` table
```sql
CREATE TABLE items (
    id TEXT PRIMARY KEY,
    stream_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content_type TEXT NOT NULL,
    content_data TEXT NOT NULL,
    author_name TEXT,
    author_email TEXT,
    author_url TEXT,
    author_avatar_url TEXT,
    published TEXT,
    updated TEXT,
    url TEXT,
    thumbnail_url TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,
    is_saved INTEGER NOT NULL DEFAULT 0,
    is_archived INTEGER NOT NULL DEFAULT 0,
    tags TEXT NOT NULL,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES streams(id) ON DELETE CASCADE
);
```

### `sync_state` table
```sql
CREATE TABLE sync_state (
    provider_id TEXT PRIMARY KEY,
    last_sync TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### Indexes
```sql
CREATE INDEX idx_streams_provider ON streams(provider_id);
CREATE INDEX idx_items_stream ON items(stream_id);
CREATE INDEX idx_items_published ON items(published DESC);
CREATE INDEX idx_items_is_read ON items(is_read);
CREATE INDEX idx_items_is_saved ON items(is_saved);
CREATE INDEX idx_items_is_archived ON items(is_archived);
```

## Future Considerations

### Phase 4: Write Operations

Additional write capabilities planned:

- Collection management (add/remove/reorder items)
- Task completion (MS To Do)
- Email actions (archive, delete, move)
- Bookmark management

### Phase 5: Advanced Features

- Full-text search with relevance ranking
- AI-powered content analysis (Scarab integration)
- Cross-stream recommendations
- Content summarization
- Natural language queries

### Scarab Integration

Scarab (AI/agent layer) may interact with Scryforge via the daemon API:

- Natural language queries
- Automated content classification
- Smart notifications
- Cross-stream intelligence

## Performance Characteristics

### Cache Performance
- Local SQLite queries: <1ms typical
- Indexed lookups on stream_id, published date
- Full-text search via LIKE queries (upgradeable to FTS5)

### Sync Performance
- Background sync every 5 minutes (configurable)
- Per-provider rate limiting
- Incremental updates (fetch only new items)
- Parallel provider syncing

### API Latency
- Unix socket: <1ms overhead
- Cached data: 1-5ms end-to-end
- Search queries: 5-50ms depending on result size

## Testing Strategy

### Unit Tests
- Provider trait implementations
- Cache operations
- API handlers
- Plugin loading

### Integration Tests
- Provider registration and discovery
- End-to-end sync flow
- API request/response cycle
- Cache persistence

### Smoke Tests
- Daemon startup and shutdown
- Provider health checks
- Basic API operations

Run tests:
```bash
cargo test --workspace
```

## Development Workflow

1. **Add new provider**: Implement `Provider` trait + capability traits
2. **Register provider**: Add to daemon's provider registry
3. **Test provider**: Unit tests + integration tests
4. **Add API methods**: Extend `ScryforgeApi` if needed
5. **Update TUI**: Add UI for new provider features
6. **Document**: Update provider matrix in PROVIDERS.md

## References

- [Provider Development Guide](./PROVIDER_DEVELOPMENT.md)
- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [API Reference](./API_REFERENCE.md)
- [Provider Capability Matrix](./PROVIDERS.md)

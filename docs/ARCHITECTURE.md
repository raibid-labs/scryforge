# Scryforge Architecture

This document describes the technical architecture of Scryforge, including workspace layout, crate responsibilities, the daemon API, and the plugin model.

## Workspace Layout

```
scryforge/
├── Cargo.toml                      # Workspace root with shared dependencies
│
├── crates/                         # Fusabi ecosystem crates (potentially extractable)
│   ├── fusabi-streams-core/        # Core traits and types
│   ├── fusabi-tui-core/            # TUI framework primitives
│   └── fusabi-tui-widgets/         # Reusable TUI widgets
│
├── scryforge-daemon/               # The hub daemon binary
├── scryforge-tui/                  # The TUI client binary
│
├── providers/                      # Provider implementations
│   ├── provider-email-imap/        # (future) IMAP email provider
│   ├── provider-rss/               # (future) RSS/Atom feed provider
│   ├── provider-spotify/           # (future) Spotify API provider
│   ├── provider-youtube/           # (future) YouTube API provider
│   ├── provider-reddit/            # (future) Reddit API provider
│   ├── provider-mstodo/            # (future) Microsoft To Do/Calendar
│   └── provider-bookmarks/         # (future) Local bookmarks + buku
│
└── docs/                           # Documentation
```

## Crate Responsibilities

### `fusabi-streams-core`

The foundational crate defining the data model and provider capabilities:

- **Types**: `StreamId`, `ItemId`, `Stream`, `Item`, `ItemContent`, `Action`
- **Traits**: `HasFeeds`, `HasCollections`, `HasSavedItems`, `HasCommunities`
- **Provider trait**: `Provider` (the base trait all providers implement)

This crate is designed to be extracted into `fusabi-community` as a standalone package that other Fusabi-based apps can use.

### `fusabi-tui-core`

Basic TUI infrastructure for Ratatui-based Fusabi applications:

- Event loop abstraction
- State management primitives
- Input handling framework
- Async command dispatch

### `fusabi-tui-widgets`

Reusable TUI widgets:

- **StreamList**: Sidebar showing available streams
- **ItemList**: Scrollable list of items with filtering
- **PreviewPane**: Rich preview of selected item
- **StatusBar**: Connection status, sync state, notifications
- **Omnibar**: Command palette / quick search

### `scryforge-daemon`

The hub daemon responsible for:

- Loading and managing provider plugins
- Periodic sync and caching of stream data
- Token retrieval from Sigilforge
- Exposing the daemon API over Unix socket or TCP
- Managing local state (SQLite or similar)

### `scryforge-tui`

The terminal user interface:

- Connects to daemon via local API
- Renders streams and items using fusabi-tui-widgets
- Handles user input and navigation
- Supports theming and customization

## Daemon API

The daemon exposes a local API for clients. The recommended transport is:

- **Unix socket** at `$XDG_RUNTIME_DIR/scryforge/daemon.sock` (or similar)
- **JSON-RPC 2.0** protocol for request/response

### API Methods (Draft)

```
// Stream discovery
streams.list() -> Stream[]
streams.get(stream_id) -> Stream

// Item retrieval
items.list(stream_id, options?) -> Item[]
items.get(item_id) -> Item
items.search(query, options?) -> Item[]

// Actions
actions.available(item_id) -> Action[]
actions.execute(item_id, action_name, params?) -> ActionResult

// Provider management
providers.list() -> ProviderInfo[]
providers.status(provider_id) -> ProviderStatus
providers.sync(provider_id?) -> SyncResult

// Cross-stream views
views.feeds() -> Item[]       // Unified feed view
views.saved() -> Item[]       // All saved items
views.collections() -> Collection[]
```

### Request/Response Format

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "streams.list",
  "params": {},
  "id": 1
}

// Response
{
  "jsonrpc": "2.0",
  "result": [
    {"id": "email:gmail:inbox", "name": "Gmail Inbox", "provider": "email-imap", ...},
    {"id": "rss:hackernews", "name": "Hacker News", "provider": "rss", ...}
  ],
  "id": 1
}
```

## Authentication via Sigilforge

Scryforge does NOT manage OAuth tokens, API keys, or credentials directly. Instead, it delegates to **Sigilforge**, a separate daemon in the raibid-labs ecosystem.

### Auth Reference Format

Providers reference credentials using URIs:

```
auth://spotify/default
auth://gmail/work
auth://reddit/personal
auth://microsoft/main
```

### Integration Pattern

```rust
// In a provider implementation
async fn get_client(&self, auth_ref: &str) -> Result<ApiClient> {
    // Request token from Sigilforge
    let token = sigilforge_client::get_token(auth_ref).await?;

    // Use token to create authenticated client
    Ok(ApiClient::with_token(token))
}
```

### Sigilforge Responsibilities

- Store encrypted credentials
- Handle OAuth flows (redirect URI, PKCE)
- Automatic token refresh
- Multi-account support per service

Scryforge assumes Sigilforge is running and accessible. If Sigilforge is unavailable, providers requiring auth will report a degraded state.

## Plugin Model

### Phase 1: In-Process Providers

Initially, providers are Rust crates compiled into the daemon:

```rust
// In scryforge-daemon
use provider_email_imap::ImapProvider;
use provider_rss::RssProvider;

fn register_providers(registry: &mut ProviderRegistry) {
    registry.register(ImapProvider::new());
    registry.register(RssProvider::new());
}
```

### Phase 2: Fusabi Plugins (.fzb)

Future iterations will support dynamic loading via Fusabi:

- Providers compiled to `.fzb` format
- Hot-reload capability
- Plugin manifest with capability declarations
- Sandboxed execution

### Phase 3: Scripted Providers (.fsx)

For simpler providers or user customization:

- Fusabi scripting for lightweight providers
- User-defined transformations
- Custom aggregations

## Data Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   External   │     │   Provider   │     │    Cache     │
│   Service    │◄───►│   (plugin)   │◄───►│   (SQLite)   │
└──────────────┘     └──────────────┘     └──────────────┘
                            │
                            ▼
                     ┌──────────────┐
                     │    Daemon    │
                     │   (hub)      │
                     └──────────────┘
                            │
                            ▼ (JSON-RPC)
                     ┌──────────────┐
                     │     TUI      │
                     └──────────────┘
```

1. **Sync**: Daemon periodically triggers providers to fetch from external services
2. **Cache**: Data is cached locally for fast access and offline support
3. **Serve**: TUI requests data from daemon, which serves from cache
4. **Actions**: User actions flow back through daemon to providers

## Configuration

Configuration is stored in `$XDG_CONFIG_HOME/scryforge/`:

```
scryforge/
├── config.toml           # Main configuration
├── providers/            # Per-provider configuration
│   ├── email-imap.toml
│   ├── rss.toml
│   └── spotify.toml
└── themes/               # TUI themes
    └── default.toml
```

### Example `config.toml`

```toml
[daemon]
socket_path = "/run/user/1000/scryforge/daemon.sock"
sync_interval_secs = 300

[sigilforge]
socket_path = "/run/user/1000/sigilforge/daemon.sock"

[providers]
enabled = ["email-imap", "rss", "spotify", "youtube", "reddit"]

[tui]
theme = "default"
```

## Future Considerations

### Scarab Integration

Scarab (AI/agent layer) may interact with Scryforge for:

- Natural language queries ("show me unread emails about the project")
- Automated actions based on content analysis
- Cross-stream intelligence

Integration would be via the same daemon API, with Scarab as another client.

### Write Operations

Phase 4+ may add limited write capabilities:

- Playlist management (add/remove/reorder)
- Bookmark operations
- Email actions (archive, mark read)
- Task completion (MS To Do)

These will be exposed as additional `Action` types and require provider support.

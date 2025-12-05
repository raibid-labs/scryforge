# Scryforge Next Steps

This document outlines concrete next tasks for continuing development of Scryforge. It is intended for AI-assisted development sessions that follow the initial scaffolding.

## Current State (Phase 0 Complete)

The scaffolding is complete with:
- Workspace layout with all crate stubs
- Core traits and types in `fusabi-streams-core`
- TUI infrastructure in `fusabi-tui-core` and `fusabi-tui-widgets`
- Placeholder daemon and TUI binaries
- Documentation (README, ARCHITECTURE, ROADMAP, PROVIDERS)

**Build status**: ✅ Compiles cleanly with `cargo build` and `cargo clippy`

The code compiles but is non-functional: the TUI shows dummy data and the daemon just logs startup.

## Immediate Next Tasks

### Task 1: Implement Daemon API Types

**Location**: Create `scryforge-daemon/src/api.rs`

Define the JSON-RPC API types:

```rust
// Request/response types for the daemon API
pub mod api {
    use serde::{Deserialize, Serialize};
    use fusabi_streams_core::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ListStreamsRequest {}

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ListStreamsResponse {
        pub streams: Vec<Stream>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ListItemsRequest {
        pub stream_id: StreamId,
        pub limit: Option<u32>,
        pub offset: Option<u32>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ListItemsResponse {
        pub items: Vec<Item>,
        pub total: u32,
    }

    // ... more request/response types
}
```

Reference `docs/ARCHITECTURE.md` for the full API specification.

### Task 2: Create a Dummy Provider

**Location**: Create `providers/provider-dummy/`

Create a minimal provider that returns fake data, useful for testing the full pipeline:

```rust
// providers/provider-dummy/src/lib.rs
use async_trait::async_trait;
use fusabi_streams_core::prelude::*;

pub struct DummyProvider;

#[async_trait]
impl Provider for DummyProvider {
    fn id(&self) -> &'static str { "dummy" }
    fn name(&self) -> &'static str { "Dummy Provider" }
    // ... implement remaining methods with fake data
}

#[async_trait]
impl HasFeeds for DummyProvider {
    // ... return fake feeds and items
}
```

Add to workspace in root `Cargo.toml`:
```toml
members = [
    # ...
    "providers/provider-dummy",
]
```

### Task 3: Wire Daemon to Registry

**Location**: `scryforge-daemon/src/main.rs` and new modules

1. Create `src/registry.rs` - Provider registry
2. Create `src/cache.rs` - In-memory cache (SQLite later)
3. Update `src/main.rs` to:
   - Load the dummy provider
   - Store streams/items in cache
   - (Defer API server to Task 4)

### Task 4: Implement Basic API Server

**Location**: `scryforge-daemon/src/api/`

Implement a minimal JSON-RPC server over Unix socket:

```rust
// Using tokio and serde_json
// Listen on $XDG_RUNTIME_DIR/scryforge/daemon.sock
// Handle streams.list and items.list methods
```

Consider using `jsonrpsee` crate for JSON-RPC implementation.

### Task 5: Connect TUI to Daemon

**Location**: `scryforge-tui/src/daemon_client.rs`

Replace dummy data in TUI with real daemon communication:

```rust
pub struct DaemonClient {
    // Unix socket connection
}

impl DaemonClient {
    pub async fn connect(socket_path: &Path) -> Result<Self>;
    pub async fn list_streams(&self) -> Result<Vec<Stream>>;
    pub async fn list_items(&self, stream_id: &StreamId) -> Result<Vec<Item>>;
}
```

Update `scryforge-tui/src/main.rs` to use the client.

### Task 6: Implement RSS Provider (First Real Provider)

**Location**: `providers/provider-rss/`

RSS is a good first real provider because:
- No authentication required
- Simple HTTP fetch + XML parsing
- Well-defined feed format

Dependencies to add:
- `reqwest` for HTTP
- `feed-rs` or `rss` crate for parsing

Implement:
- `HasFeeds` - each RSS feed URL is a feed
- `HasCommunities` - the feed sources themselves

Configuration format:
```toml
# providers/rss.toml
[[feeds]]
name = "Hacker News"
url = "https://news.ycombinator.com/rss"

[[feeds]]
name = "Lobsters"
url = "https://lobste.rs/rss"
```

## Guidelines for Extending the Scaffolding

### Adding New Provider Crates

1. Create directory: `providers/provider-{name}/`
2. Create `Cargo.toml`:
   ```toml
   [package]
   name = "provider-{name}"
   version = "0.1.0"
   edition.workspace = true

   [dependencies]
   fusabi-streams-core.workspace = true
   async-trait.workspace = true
   # ... provider-specific deps
   ```
3. Create `src/lib.rs` implementing `Provider` and relevant capability traits
4. Add to workspace members in root `Cargo.toml`
5. Register in daemon's provider registry

### Adding New TUI Widgets

1. Add widget struct to `fusabi-tui-widgets/src/lib.rs`
2. Follow the pattern of existing widgets:
   - Take references to data and theme
   - Implement a `render()` method
   - Use builder pattern for options (`.focused()`, etc.)
3. Add to the `prelude` module

### Adding New API Methods

1. Define request/response types in `scryforge-daemon/src/api/types.rs`
2. Add method handler in `scryforge-daemon/src/api/server.rs`
3. Document in `docs/ARCHITECTURE.md`
4. Add client method in `scryforge-tui/src/daemon_client.rs`

### Configuration

Configuration files should go in `$XDG_CONFIG_HOME/scryforge/`:
- `config.toml` - main configuration
- `providers/{name}.toml` - per-provider configuration

Use `directories` crate for XDG paths.

## Dependencies to Consider Adding

For the next development phases, consider adding these workspace dependencies:

```toml
[workspace.dependencies]
# JSON-RPC
jsonrpsee = { version = "0.24", features = ["server", "client"] }

# HTTP client (for providers)
reqwest = { version = "0.12", features = ["json"] }

# RSS/Atom parsing
feed-rs = "2.0"

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# Configuration
config = "0.14"
directories = "5.0"

# IMAP
async-imap = "0.9"
```

## Testing Strategy

### Unit Tests

Each crate should have unit tests in `src/lib.rs` or `tests/` directory:
- `fusabi-streams-core`: Test type serialization, ID generation
- Provider crates: Test parsing, transformation logic (mock HTTP)
- TUI widgets: Snapshot tests for rendering

### Integration Tests

Create `tests/` directory at workspace root:
- `tests/daemon_api.rs` - Test API endpoints
- `tests/provider_sync.rs` - Test provider sync with mocked services

### Manual Testing

1. Start daemon: `cargo run --bin scryforge-daemon`
2. In another terminal: `cargo run --bin scryforge-tui`
3. Verify:
   - Streams appear in sidebar
   - Items load when selecting a stream
   - Preview updates when selecting an item
   - Keyboard navigation works

## Architecture Decisions to Preserve

When extending the codebase, maintain these architectural principles:

1. **Separation of concerns**: TUI never talks directly to external services
2. **Provider independence**: Each provider is a separate crate
3. **Async-first**: All I/O operations are async
4. **Trait-based capabilities**: Use `Has*` traits for provider features
5. **Local-first**: Cache aggressively, support offline viewing
6. **Auth delegation**: All auth goes through Sigilforge (when implemented)

## Questions for Future Sessions

If you're an AI assistant continuing this work, consider asking the human:

1. Should we implement Sigilforge first, or use placeholder auth?
2. What's the priority order for providers?
3. Are there specific TUI interactions that need attention?
4. Should we add any CI/CD configuration?
5. What's the preferred logging/tracing setup?

## File Checklist

After completing the next phase, these files should exist:

```
scryforge/
├── Cargo.toml                          ✓ (exists)
├── README.md                           ✓ (exists)
├── NEXT_STEPS.md                       ✓ (exists)
├── docs/
│   ├── ARCHITECTURE.md                 ✓ (exists)
│   ├── ROADMAP.md                      ✓ (exists)
│   └── PROVIDERS.md                    ✓ (exists)
├── crates/
│   ├── fusabi-streams-core/
│   │   ├── Cargo.toml                  ✓ (exists)
│   │   └── src/lib.rs                  ✓ (exists)
│   ├── fusabi-tui-core/
│   │   ├── Cargo.toml                  ✓ (exists)
│   │   └── src/lib.rs                  ✓ (exists)
│   └── fusabi-tui-widgets/
│       ├── Cargo.toml                  ✓ (exists)
│       └── src/lib.rs                  ✓ (exists)
├── scryforge-daemon/
│   ├── Cargo.toml                      ✓ (exists)
│   └── src/
│       ├── main.rs                     ✓ (exists)
│       ├── api/                        □ (todo)
│       │   ├── mod.rs
│       │   ├── types.rs
│       │   └── server.rs
│       ├── registry.rs                 □ (todo)
│       └── cache.rs                    □ (todo)
├── scryforge-tui/
│   ├── Cargo.toml                      ✓ (exists)
│   └── src/
│       ├── main.rs                     ✓ (exists)
│       └── daemon_client.rs            □ (todo)
└── providers/
    ├── .gitkeep                        ✓ (exists)
    ├── provider-dummy/                 □ (todo)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── provider-rss/                   □ (todo)
        ├── Cargo.toml
        └── src/lib.rs
```

Legend: ✓ = exists, □ = todo

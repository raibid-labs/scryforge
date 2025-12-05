# Scryforge

**A Fusabi-powered, pluggable TUI information rolodex for the terminal.**

> **Status**: Phase 0 complete — scaffolding and architecture in place. The TUI renders with dummy data; daemon starts but has no API yet. See [NEXT_STEPS.md](NEXT_STEPS.md) for immediate work items.

Scryforge unifies multiple read-mostly information streams (email, RSS, playlists, saved items, bookmarks, and more) into a single terminal interface backed by a local daemon. It is part of the **raibid-labs ecosystem** (Scarab, Hibana, Tolaria, Phage, Fusabi).

## Overview

Scryforge follows a **daemon + TUI** architecture:

```
┌─────────────────┐      local API       ┌─────────────────────┐
│  scryforge-tui  │ ◄──────────────────► │  scryforge-daemon   │
│   (Ratatui)     │    (Unix socket /    │      ("hub")        │
└─────────────────┘     JSON-RPC)        └─────────┬───────────┘
                                                   │
                           ┌───────────────────────┼───────────────────────┐
                           │                       │                       │
                           ▼                       ▼                       ▼
                    ┌─────────────┐         ┌─────────────┐         ┌─────────────┐
                    │  Provider   │         │  Provider   │         │  Provider   │
                    │  (IMAP)     │         │  (RSS)      │         │  (Spotify)  │
                    └─────────────┘         └─────────────┘         └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ Sigilforge  │  (separate auth manager daemon)
                    │  (tokens)   │
                    └─────────────┘
```

- **Daemon ("hub")**: Manages provider plugins, handles sync/caching, retrieves auth tokens from Sigilforge, and exposes a local API.
- **TUI**: A Ratatui-based terminal client with explorer-style navigation (sidebars, lists, preview pane, omnibar).
- **Providers**: Pluggable data sources implementing capability traits (feeds, collections, saved items, etc.).

## Core Abstractions

### Stream

A **Stream** is a logical feed or collection of items:
- Email inboxes
- RSS feeds
- Spotify/YouTube playlists
- Reddit subreddits
- Bookmark folders
- Saved/liked items

### Item

An **Item** is an entry within a stream:
- Email message
- RSS article
- Video (YouTube)
- Track (Spotify)
- Reddit post
- Bookmark
- Task or calendar event

### Action

An **Action** is an operation that can be performed on an item:
- **Read-only (MVP)**: Open, preview, copy link, open in browser, tag locally
- **Write (future)**: Add to playlist, save/unsave, archive, mark read

### Provider Capabilities

Providers implement one or more capability traits:

| Trait | Description | Examples |
|-------|-------------|----------|
| `HasFeeds` | Lists logical feeds, retrieves items | Email inboxes, RSS, Reddit home/subs, YouTube subscriptions |
| `HasCollections` | Named collections with ordered items | Playlists (Spotify, YouTube), bookmark folders |
| `HasSavedItems` | Saved/bookmarked/liked items | Reddit saved, Medium bookmarks, YouTube watch-later |
| `HasCommunities` | Membership/subscriptions | Subreddits, channels, RSS feed list |

## MVP Scope

### In Scope

- **Read-only viewing** of information streams
- **Providers**:
  - Email via IMAP (Gmail, Outlook)
  - RSS feeds (including Medium via RSS)
  - Microsoft To Do and Calendar (via MS Graph, read-only)
  - Spotify (playlists, liked tracks)
  - YouTube (subscriptions, playlists, watch-later)
  - Reddit (home/subs, saved posts)
  - Bookmarks (local store, optional `buku` integration)
- **TUI features**:
  - Explorer-style navigation (sidebar, list, preview)
  - Fast filtering and search
  - Omnibar / command palette
  - Cross-stream views (all saved items, all feeds)

### Out of Scope (MVP)

- Complex HTML email rendering
- Email composer / full write flows
- Heavy write operations (creating playlists from scratch, etc.)
- Full Gmail API (IMAP only for MVP)
- Mobile or GUI clients

## Project Structure

```
scryforge/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── fusabi-streams-core/   # Core traits and types (Stream, Item, Actions)
│   ├── fusabi-tui-core/       # TUI event loop and state wiring
│   └── fusabi-tui-widgets/    # Reusable Ratatui widgets
├── scryforge-daemon/          # The hub daemon
├── scryforge-tui/             # The TUI client
├── providers/                 # Provider crate implementations
└── docs/
    ├── ARCHITECTURE.md        # Detailed architecture
    ├── ROADMAP.md             # Phased development plan
    └── PROVIDERS.md           # Provider capability model
```

## Related Projects

- **Sigilforge**: Auth manager daemon for token storage and refresh (referenced but not implemented here)
- **Scarab**: AI/agent integration layer (future integration)
- **Fusabi**: Plugin system powering Scryforge's extensibility

## Current State

### What Works
- **Workspace compiles cleanly** with `cargo build` and `cargo clippy`
- **TUI renders** with dummy data (streams sidebar, item list, preview pane, omnibar, status bar)
- **Keyboard navigation** works (vim keys, Tab to switch panes, `/` for search, `q` to quit)
- **Core types defined** in `fusabi-streams-core` (Stream, Item, Actions, Provider traits)

### What's Stubbed
- Daemon starts and logs but has no API server
- TUI uses hardcoded dummy data (not connected to daemon)
- No providers implemented yet
- No configuration loading
- No caching/persistence

## Getting Started

```bash
# Build all crates
cargo build

# Run the daemon
cargo run --bin scryforge-daemon

# Run the TUI (in another terminal)
cargo run --bin scryforge-tui
```

## Development

See [NEXT_STEPS.md](NEXT_STEPS.md) for detailed next tasks. Summary:

1. **Task 1**: Implement daemon API types
2. **Task 2**: Create a dummy provider
3. **Task 3**: Wire daemon to provider registry
4. **Task 4**: Implement JSON-RPC API server
5. **Task 5**: Connect TUI to daemon
6. **Task 6**: Implement RSS provider (first real provider)

## License

MIT OR Apache-2.0

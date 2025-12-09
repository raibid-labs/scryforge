# Scryforge

[![CI](https://github.com/raibid-labs/scryforge/workflows/CI/badge.svg)](https://github.com/raibid-labs/scryforge/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

**A Fusabi-powered, pluggable TUI information rolodex for the terminal.**

> **Status**: Phase 0 complete — scaffolding and architecture in place. The TUI renders with dummy data; daemon starts but has no API yet. See [docs/ROADMAP.md](docs/ROADMAP.md) for development phases.

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

### Quick Start

```bash
# Build all crates
cargo build

# Run the daemon (in one terminal)
cargo run --bin scryforge-daemon

# Run the TUI (in another terminal)
cargo run --bin scryforge-tui
```

### Documentation

- **[Getting Started Guide](docs/GETTING_STARTED.md)** - Installation, first run, and basic usage
- **[Configuration Guide](docs/CONFIGURATION.md)** - Complete config.toml reference
- **[Keybindings Reference](docs/KEYBINDINGS.md)** - All keyboard shortcuts
- **[Commands Reference](docs/COMMANDS.md)** - Omnibar commands and search syntax
- **[Architecture](docs/ARCHITECTURE.md)** - System design and implementation details
- **[Roadmap](docs/ROADMAP.md)** - Development phases and planned features
- **[Providers](docs/PROVIDERS.md)** - Provider capability model and integration guide

## Usage

The TUI uses vim-style keybindings for efficient navigation:

| Key | Action |
|-----|--------|
| `h/l` or `Tab` | Switch between panes |
| `j/k` or `↑/↓` | Navigate lists |
| `/` | Search |
| `:` | Commands (`:quit`, `:sync`, `:refresh`, etc.) |
| `r` | Toggle read/unread |
| `s` | Save/unsave item |
| `?` | Show help |
| `q` | Quit |

See [docs/KEYBINDINGS.md](docs/KEYBINDINGS.md) and [docs/COMMANDS.md](docs/COMMANDS.md) for complete references.

## Development

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full development roadmap. Current priorities:

1. Implement daemon API types and JSON-RPC server
2. Create provider registry and caching layer
3. Implement first real provider (RSS)
4. Connect TUI to daemon
5. Add configuration loading

### Testing

The project includes comprehensive tests for TUI widgets and core state management:

```bash
# Run all tests with the recommended target directory override
CARGO_TARGET_DIR=./target cargo test --workspace

# Run tests for a specific crate
CARGO_TARGET_DIR=./target cargo test -p fusabi-tui-core
CARGO_TARGET_DIR=./target cargo test -p fusabi-tui-widgets

# Run tests with output
CARGO_TARGET_DIR=./target cargo test --workspace -- --nocapture

# Run a specific test
CARGO_TARGET_DIR=./target cargo test test_list_state_new
```

**Note:** Using `CARGO_TARGET_DIR=./target` ensures consistent build artifact location and avoids permission issues in CI environments.

### Linting and Formatting

```bash
# Check formatting
cargo fmt --all -- --check

# Format code
cargo fmt --all

# Run clippy (linter)
CARGO_TARGET_DIR=./target cargo clippy --workspace -- -D warnings

# Auto-fix clippy warnings (when safe)
CARGO_TARGET_DIR=./target cargo clippy --workspace --fix --allow-dirty
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `CARGO_TARGET_DIR=./target cargo test --workspace`
5. Run lints: `cargo clippy` and `cargo fmt`
6. Submit a pull request

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for design details and [docs/STRUCTURE.md](docs/STRUCTURE.md) for documentation guidelines.

## License

MIT OR Apache-2.0

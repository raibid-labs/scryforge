# Scryforge Architecture

## Goals

Scryforge should:

- Act as a *central read layer* for many information services.
- Provide a **fast, keyboard-driven TUI** for exploring information across accounts.
- Be **plugin-based**, with most integration logic living in providers and Fusabi scripts.
- Expose a **daemon API** that other tools (CLI, Scarab agents, future GUIs) can call.
- Grow the **Fusabi ecosystem** with reusable crates and packages.

## High-Level Components

Scryforge has three main runtime components:

1. **Daemon (Hub)**
2. **TUI client**
3. **Provider plugins (Rust + Fusabi)**

### 1. Daemon (Hub)

The daemon is responsible for:

- Loading provider plugins (as Rust crates and/or Fusabi `.fzb` bundles).
- Querying remote services (email, RSS, media APIs, etc.).
- Normalizing results into a common **Stream / Item** model.
- Caching results locally and managing sync.
- Calling **Sigilforge** to obtain and refresh credentials/tokens.
- Exposing a local API (Unix socket or TCP, JSON-RPC or similar).

It knows **nothing** about terminal UI details. It just serves streams and items.

### 2. TUI Client

The TUI is a separate process that:

- Uses **Ratatui** for layout and rendering.
- Provides explorer-like navigation inspired by tools like *yazi* and *broot*:
  - Sidebars for streams and collections
  - Main list for items
  - Preview pane
  - Omnibar / command palette for filtering and commands
- Communicates exclusively with the daemon API.
- May load Fusabi `.fsx` scripts on the client side for:
  - Custom keybindings
  - Layout tweaks
  - Command palette actions
  - Convenience transforms (e.g., custom views across streams)

### 3. Provider Plugins

Providers are responsible for communicating with specific services and implementing
**capability traits**. Each provider indicates which capabilities it supports.

Examples:

- `provider-email-imap`
- `provider-rss`
- `provider-spotify`
- `provider-youtube`
- `provider-reddit`
- `provider-bookmarks`
- `provider-msgraph` (To Do + Calendar)

Providers are implemented as Rust crates that can be compiled to `.fzb` Fusabi
plugins for the daemon.

They never handle raw secrets directly: they ask **Sigilforge** via a client
library for tokens and credentials.

## Core Domain Model

### Streams

A **Stream** is a logical grouping of items:

- Email folders (INBOX, Archive, etc.)
- RSS feeds
- Spotify playlists
- YouTube playlists or subscription feeds
- Reddit home timeline and individual subreddits
- Medium publications
- Saved items, watch-later lists, liked songs
- Bookmark folders
- Task lists (e.g., Microsoft To Do)
- Calendar time windows (Today, This Week, etc.)

`Stream` fields typically include:

- Stable ID
- Provider ID / type
- Human-readable name
- Capability type(s)
- Optional filters / query parameters

### Items

An **Item** is a single entry within a stream, e.g.:

- Email message
- RSS article
- Reddit post
- Spotify track
- YouTube video
- Medium article
- Bookmark
- Task
- Calendar event

Items have:

- Stable ID and provider ID
- Timestamps (published, updated)
- Title / subject
- Summary / snippet / short content
- Link(s) (canonical URL, provider UI link)
- Provider-specific metadata (e.g., labels for email, subreddit name)
- Local metadata (tags, pinned state, local notes) managed by Scryforge

### Provider Capabilities

Rather than hard-coding provider types, Scryforge relies on a set of capabilities
that providers can declare and implement.

Initial capability traits:

- `HasFeeds`
  - Exposes a list of logical feeds/streams that behave like timelines.
  - Examples: RSS feeds, email folders, YouTube subscriptions, Medium feeds.

- `HasCollections`
  - Exposes named collections with ordered items.
  - Examples: Spotify playlists, bookmark folders.

- `HasSavedItems`
  - Exposes items explicitly saved/bookmarked by the user.
  - Examples: Reddit saved, Medium bookmarks, YouTube watch-later, Spotify liked songs, browser bookmarks.

- `HasCommunities`
  - Exposes "membership" or subscription relationships.
  - Examples: subscribed subreddits, followed channels, Medium publications, RSS feed list.

- `HasTasks` and `HasCalendar` (for MS Graph or similar)
  - To expose task lists and calendar views.

The daemon queries providers and merges results into a unified `Stream`/`Item`
schema used by the TUI and external clients.

## Read-Only MVP

For MVP:

- Scryforge is **read-only** for content operations:
  - No sending email
  - No posting to social media
  - No editing content on remote services
- Limited local actions are allowed:
  - Local tagging
  - Pinning
  - Local notes
  - "Mark as read" for Scryforge-only local state (initially)
- External write operations (e.g., editing playlists, bookmarking in providers)
  may be added later, once the core abstractions and UX are stable.

This drastically simplifies:

- Implementation complexity
- Surface area for bugs
- OAuth scopes
- UI/UX design for destructive operations

## Daemon–TUI API

The internal API is intentionally narrow. Representative endpoints / RPC
methods might include:

- `ListStreams() -> [Stream]`
- `ListItems(stream_id, cursor) -> ItemPage`
- `GetItem(item_id) -> Item`
- `GetItemPreview(item_id) -> RenderableContent`
- `RunLocalAction(item_id, action_id, params) -> Item` (local tagging, etc.)

The details of the transport (Unix socket vs TCP, JSON-RPC vs custom protocol)
are left open for early experimentation, but the contract is "TUI and other
clients talk to daemon via a documented API; providers never talk directly
to the TUI."

## Authentication via Sigilforge

Authentication and secret management are delegated to **Sigilforge**.

Providers will request tokens via a client library:

- `get_token(service, account_alias) -> AccessToken`
- `ensure_access_token(service, account_alias) -> AccessToken`

`service` is a symbolic name like:

- `gmail`
- `outlook`
- `spotify`
- `youtube`
- `reddit`
- `msgraph`

`account_alias` distinguishes multiple accounts for the same service,
e.g. `personal`, `work`, `lab`.

Sigilforge handles storage, refresh, and flows; Scryforge only handles
HTTP/API calls using the provided tokens.

## Workspace Layout (Suggested)

A possible Rust workspace layout for Scryforge:

- `scryforge-daemon/`
  - Daemon binary crate
- `scryforge-tui/`
  - TUI binary crate
- `fusabi-streams-core/`
  - Reusable model + traits for streams/items/capabilities
- `fusabi-tui-core/`
  - Reusable TUI scaffolding for Fusabi-aware apps
- `fusabi-tui-widgets/`
  - Shared TUI widgets (stream list, item list, preview)
- `providers/`
  - `provider-email-imap/`
  - `provider-rss/`
  - `provider-spotify/`
  - `provider-youtube/`
  - `provider-reddit/`
  - `provider-bookmarks/`
  - `provider-msgraph/`

Additional documentation is kept in `docs/`.

## Integration with Fusabi

Scryforge should help grow the Fusabi ecosystem:

- **Daemon plugins**:
  - Implemented in Rust and/or Fusabi
  - Compiled to `.fzb` for performance where appropriate

- **Client plugins**:
  - Fusabi `.fsx` scripts loaded by the TUI to:
    - Define new views and derived streams
    - Add commands and keybindings
    - Implement custom filters/slices

- **Fusabi community packages**:
  - `fusabi-streams-core` (types & traits)
  - `fusabi-tui-core` and `fusabi-tui-widgets`
  - Future: provider-specific `fusabi-*` packages wrapping particular APIs

Scryforge serves as a flagship implementation of "Fusabi as an application
extension and plugin language."

# Scryforge Roadmap

This roadmap is intentionally high-level. It outlines phases for turning
Scryforge into a usable, extensible information console.

## Phase 0 — Workspace & Skeleton

- Create Rust workspace with:
  - `scryforge-daemon`
  - `scryforge-tui`
  - `fusabi-streams-core`
  - `fusabi-tui-core`
  - `fusabi-tui-widgets`
  - `providers/` directory
- Implement initial `Stream` and `Item` types in `fusabi-streams-core`:
  - Basic metadata fields
  - Capability marker traits (`HasFeeds`, `HasCollections`, etc.)
- Implement skeleton TUI that:
  - Displays a placeholder list of streams and items from a dummy provider
  - Handles basic keybindings and quitting
- Implement skeleton daemon that:
  - Serves a hard-coded set of streams/items over a simple local API

## Phase 1 — Core TUI + First Real Providers

Focus: email + RSS, solid core UX.

- Implement IMAP-based email provider:
  - Configurable accounts (e.g. Gmail, Outlook/IMAP)
  - `HasFeeds` for folders (INBOX, Archive, etc.)
  - Items for messages (subject, from, snippet, dates, flags as metadata)
  - Read-only: listing and viewing messages; no send/compose.
- Implement RSS provider:
  - Feeds configured via a simple config file
  - `HasFeeds` implementation (per-feed streams)
  - Items for articles (title, link, summary, published time)
- Integrate with Sigilforge for accounts that need OAuth (or configure
  IMAP passwords via Sigilforge if desired).
- Upgrade TUI:
  - Sidebar for streams
  - Main list for items
  - Preview pane for content (e.g., plain text/HTML-stripped email, RSS item)

Deliverable: Scryforge can act as a basic multi-account email + RSS reader.

## Phase 2 — Productivity & Media Providers

Focus: expanding coverage to tasks, calendar, media and social.

- Microsoft Graph provider:
  - `HasTasks` → Microsoft To Do
  - `HasCalendar` → basic views like Today, This Week
- Spotify provider:
  - `HasCollections` → playlists
  - `HasSavedItems` → liked songs
- YouTube provider:
  - `HasFeeds` → subscriptions feed
  - `HasCollections` → playlists
  - `HasSavedItems` → watch-later
- Reddit provider:
  - `HasFeeds` → home and subreddits
  - `HasSavedItems` → saved posts and comments
  - `HasCommunities` → subreddit subscriptions
- Bookmarks provider:
  - Local bookmarks DB OR integration with an existing CLI like `buku`
  - `HasCollections` and `HasSavedItems`

TUI improvements:

- Basic omnibar for filtering within current stream
- Quick switching between streams and providers
- Per-stream configuration for visible columns and sort order

## Phase 3 — Unified Views & Cross-Stream Features

Focus: make Scryforge more than the sum of its parts.

- Unified views:
  - `Saved`:
    - Aggregates all `HasSavedItems` providers
    - E.g., Reddit saved, Medium bookmarks via RSS + tags, YouTube watch-later, Spotify liked, bookmarks
  - `Playlists`:
    - Aggregates all `HasCollections` playlist-like providers
    - E.g., Spotify playlists, YouTube playlists, playlist-style bookmark folders
  - `Feeds`:
    - Aggregates all `HasFeeds` providers
    - E.g., RSS, email inboxes, YouTube subscriptions, Medium feeds
- Local metadata:
  - Tagging items across providers
  - Pinning
  - Local notes attached to items
- TUI enhancements:
  - Persistent layouts and workspaces
  - Saved filters

## Phase 4 — Optional Write Operations

Only after read-only flows are mature:

- Playlist editing:
  - Add/remove tracks/videos from playlists (Spotify, YouTube)
- Bookmark management:
  - Create/remove bookmarks and folders in the bookmarks provider
- Optional "mark read" propagation to email providers (via IMAP flags)
- Optional integration with Reddit, YouTube, etc. for actions like:
  - Mark watched
  - Star/like (carefully scoped)

These features must be added with attention to:

- Clear UI affordances and confirmations
- Idempotency and error handling
- Minimal OAuth scopes

## Phase 5 — Agents & Intelligent Views

Once the core product is stable:

- Scarab integration:
  - Allow Scarab agents to use Scryforge’s daemon API for read operations
  - Expose derived views as structured data to agents
- Intelligent views:
  - “Today” dashboard:
    - New emails, tasks due today, calendar, key feeds
  - “Deep work” view:
    - Only items tagged or classified as high priority
- AI-powered features (optional):
  - Summarize inbox or feeds
  - Cluster and deduplicate links across providers
  - Suggest tasks based on emails or save actions

This phase is intentionally open-ended and should be driven by experience
from earlier phases and user feedback.

# Scryforge Roadmap

This document outlines the phased development plan for Scryforge.

## Phase 0: Scaffolding and Dummy Provider

**Goal**: Establish project structure and prove the architecture with a minimal implementation.

### Deliverables

- [x] Workspace layout with all crate stubs
- [x] Core types defined in `fusabi-streams-core`
- [x] Documentation (README, ARCHITECTURE, ROADMAP, PROVIDERS)
- [ ] Dummy/mock provider that returns fake data
- [ ] Basic daemon with in-memory state
- [ ] Daemon API skeleton (streams.list, items.list)
- [ ] Minimal TUI that connects to daemon and displays dummy streams

### Success Criteria

- `cargo build` succeeds for all workspace members
- TUI can display a list of dummy streams from the daemon
- Architecture is validated end-to-end

---

## Phase 1: IMAP Email + RSS + Basic TUI

**Goal**: First real providers with a functional TUI.

### Deliverables

- [ ] `provider-email-imap` crate
  - Connect via IMAP (supports Gmail, Outlook, generic IMAP)
  - List mailboxes as streams
  - Fetch message headers and plain text bodies
  - Auth via Sigilforge (`auth://imap/account`)
- [ ] `provider-rss` crate
  - Parse RSS and Atom feeds
  - Support OPML import for feed list
  - Fetch and cache feed items
- [ ] TUI enhancements
  - Stream sidebar with provider grouping
  - Item list with sorting (date, unread status)
  - Preview pane for email/article content
  - Basic keyboard navigation (vim-style)
- [ ] Daemon improvements
  - Periodic sync scheduling
  - SQLite cache for items
  - Provider health monitoring

### Success Criteria

- Can view Gmail inbox and RSS feeds in the TUI
- Items persist across daemon restarts
- Responsive navigation with 1000+ items

---

## Phase 2: Expanded Providers

**Goal**: Add remaining MVP providers for comprehensive information access.

### Deliverables

- [ ] `provider-mstodo` crate
  - Microsoft To Do tasks (via MS Graph)
  - Calendar events (via MS Graph)
  - Read-only: view tasks and upcoming events
  - Auth via Sigilforge (`auth://microsoft/account`)
- [ ] `provider-spotify` crate
  - List playlists as collections
  - Liked tracks as saved items
  - Track metadata and album art URLs
  - Auth via Sigilforge (`auth://spotify/account`)
- [ ] `provider-youtube` crate
  - Subscription feed (HasFeeds)
  - Playlists (HasCollections)
  - Watch Later (HasSavedItems)
  - Auth via Sigilforge (`auth://youtube/account`)
- [ ] `provider-reddit` crate
  - Home feed and subreddit feeds (HasFeeds)
  - Saved posts (HasSavedItems)
  - Subreddit subscriptions (HasCommunities)
  - Auth via Sigilforge (`auth://reddit/account`)
- [ ] `provider-bookmarks` crate
  - Local bookmark storage (JSON/SQLite)
  - Optional buku integration
  - Bookmark folders as collections

### Success Criteria

- All listed providers functional in read-only mode
- Can switch between different provider types seamlessly
- Auth flow works via Sigilforge for all OAuth providers

---

## Phase 3: Unified Views and Cross-Stream Features

**Goal**: Provide aggregate views that span multiple providers.

### Deliverables

- [ ] Unified "All Feeds" view
  - Combines RSS, Reddit home, YouTube subscriptions, email (as timeline)
  - Sorted by date, filterable by provider
- [ ] Unified "Saved Items" view
  - Aggregates: Reddit saved, YouTube watch-later, Spotify liked, bookmarks
  - Cross-provider search
- [ ] Unified "Collections" view
  - Lists all playlists, bookmark folders, email labels
  - Navigate into any collection
- [ ] Enhanced search
  - Full-text search across all cached items
  - Provider-specific search (delegated to API where available)
- [ ] Omnibar improvements
  - Quick stream switching
  - Inline search
  - Command execution

### Success Criteria

- Can answer "what have I saved recently?" across all services
- Search returns results from all providers
- Omnibar enables rapid navigation without mouse

---

## Phase 4: Limited Write Operations

**Goal**: Enable "library" operations that manage content organization.

### Deliverables

- [ ] Playlist operations
  - Add/remove items from Spotify/YouTube playlists
  - Reorder items
- [ ] Bookmark operations
  - Create/edit/delete bookmarks
  - Organize into folders
- [ ] Email operations
  - Mark read/unread
  - Archive
  - Move between folders
- [ ] Task operations
  - Mark MS To Do tasks complete
  - Maybe: create simple tasks
- [ ] Save/unsave operations
  - Reddit save/unsave
  - YouTube add to/remove from watch-later

### Success Criteria

- Write operations succeed and reflect in external services
- Optimistic updates in TUI with error handling
- No data loss scenarios

---

## Phase 5+: Advanced Features

**Goal**: Extended capabilities and ecosystem integration.

### Potential Features

- [ ] **Scarab Integration**
  - Natural language queries
  - AI-assisted categorization
  - Smart summaries of feed content
- [ ] **Fusabi Plugin System**
  - Dynamic provider loading (.fzb)
  - User-defined providers via scripting (.fsx)
  - Plugin marketplace integration
- [ ] **Notification System**
  - Desktop notifications for new items
  - Priority filtering
  - Quiet hours
- [ ] **Offline Mode**
  - Full offline access to cached content
  - Queue actions for sync when online
- [ ] **Multi-device Sync**
  - Sync read status across devices
  - Via cloud storage or self-hosted sync
- [ ] **Rich Content**
  - Inline image viewing (sixel/kitty protocol)
  - Video thumbnails
  - Audio playback integration

---

## Non-Goals (Explicit Exclusions)

The following are explicitly out of scope for Scryforge:

- **Full email client**: No composer, no complex HTML rendering, no attachment handling
- **Social posting**: No creating Reddit posts, no tweeting, no publishing
- **Real-time chat**: Not a Slack/Discord/Matrix client
- **Media playback**: Links to external players, not embedded playback
- **GUI client**: Terminal-only for now

---

## Dependencies on Other Projects

| Project | Dependency | Notes |
|---------|------------|-------|
| Sigilforge | Required for OAuth providers | Must be running for auth |
| Fusabi | Required for plugin system | Phase 5+ for dynamic plugins |
| Scarab | Optional | AI features in Phase 5+ |

---

## Release Milestones

| Version | Phase | Key Features |
|---------|-------|--------------|
| 0.1.0 | 0 | Scaffolding, dummy provider, basic daemon/TUI |
| 0.2.0 | 1 | IMAP + RSS + functional TUI |
| 0.3.0 | 2 | All MVP providers |
| 0.4.0 | 3 | Unified views, search |
| 0.5.0 | 4 | Write operations |
| 1.0.0 | 5 | Stable API, plugin system, production-ready |

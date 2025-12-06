# Scryforge Provider Model

This document describes the provider abstraction used by Scryforge and
outlines the initial set of providers targeted for MVP and early
expansions.

## Provider Responsibilities

Each provider is responsible for:

- Mapping a **remote service** (e.g. IMAP, RSS, Spotify, YouTube, Reddit)
  into Scryforge's unified `Stream` and `Item` model.
- Implementing one or more **capabilities**:
  - `HasFeeds`
  - `HasCollections`
  - `HasSavedItems`
  - `HasCommunities`
  - `HasTasks`
  - `HasCalendar`
- Handling remote API calls, pagination, and data normalization.
- Consulting **Sigilforge** for credentials and tokens:
  - Providers should not read passwords or tokens from disk directly.
  - All secrets should be obtained through Sigilforge’s client API.

Providers SHOULD NOT:

- Implement their own UI.
- Persist their own local cache directly (that’s the daemon’s job).
- Handle low-level auth flows (device code, PKCE, etc.) themselves.

## Capabilities

### HasFeeds

Use when a provider exposes one or more feeds or timelines of items.

Examples:

- IMAP email:
  - Folders (INBOX, Archive, etc.) become feeds
- RSS/Atom:
  - Each subscribed feed becomes a feed
- YouTube:
  - Subscriptions feed
- Medium (via RSS):
  - Feeds for profiles, publications, tags

### HasCollections

Use when a provider exposes named collections with ordered items.

Examples:

- Spotify:
  - Playlists
- YouTube:
  - Playlists
- Bookmarks:
  - Folders or tags treated as collections

### HasSavedItems

Use for explicit, user-curated lists of saved/bookmarked items.

Examples:

- Reddit:
  - Saved posts and comments
- Medium:
  - Bookmarked articles
- YouTube:
  - Watch-later list, liked videos
- Spotify:
  - Liked songs
- Bookmarks:
  - Saved links

### HasCommunities

Use for joined/subscribed groups of content.

Examples:

- Reddit:
  - Subscribed subreddits
- YouTube:
  - Subscribed channels
- Medium:
  - Followed publications, authors
- RSS:
  - Subscribed feeds as "communities"

### HasTasks, HasCalendar

Use for productivity data:

- Microsoft Graph:
  - Tasks (Microsoft To Do)
  - Calendar events

## Target Provider List

### IMAP Email Provider

- Capability: `HasFeeds`
- Streams:
  - IMAP folders
- Items:
  - Messages with subject, from, snippet, etc.
- MVP:
  - List folders
  - List messages in a folder
  - View message content (plain text or simplified HTML)

### RSS Provider

- Capabilities: `HasFeeds`
- Streams:
  - Individual RSS/Atom feeds from configuration
- Items:
  - Articles with title, summary, link, date

### Medium (RSS) Provider

- Implemented via RSS provider or a convenience wrapper
- Capabilities: `HasFeeds`
- Streams:
  - Medium-author, publication, and tag feeds

### Spotify Provider

- Capabilities:
  - `HasCollections` (playlists)
  - `HasSavedItems` (liked songs)
- Streams:
  - Playlists, liked songs
- Items:
  - Tracks with artist, album, duration, etc.

### YouTube Provider

- Capabilities:
  - `HasFeeds` (subscriptions)
  - `HasCollections` (playlists)
  - `HasSavedItems` (watch-later, liked videos)
- Streams:
  - Subscriptions feed
  - Playlists
  - Saved/watch-later streams

### Reddit Provider

- Capabilities:
  - `HasFeeds` (home, subreddits)
  - `HasSavedItems` (saved posts/comments)
  - `HasCommunities` (subscribed subreddits)
- Streams:
  - Home feed
  - Subreddit feeds
  - Saved posts

### Bookmarks Provider

- Capabilities:
  - `HasCollections`
  - `HasSavedItems`
- Implementation options:
  - Native bookmarks DB
  - Integration with existing CLI like `buku`
- Streams:
  - Folders or tags as collections
  - Saved links list

### Microsoft Graph Provider

- Capabilities:
  - `HasTasks`
  - `HasCalendar`
- Streams:
  - Task lists (e.g. To Do lists)
  - Calendar views (Today, This Week, etc.)

## Extensibility

New providers can be added by:

- Implementing the required capability traits in Rust or Fusabi.
- Registering with the daemon so Scryforge can discover streams.
- Mapping provider-specific configuration (accounts, scopes) to Sigilforge
  account identifiers (e.g. `msgraph/work`, `spotify/personal`).

Long-term, provider development should be aided by reusable `fusabi-*` packages
for common API patterns, data types, and auth helpers.

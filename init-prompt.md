You are Claude, acting as a project scaffolding and architecture assistant.

You are at the root of a new repo called `scryforge`.

Scryforge is a Fusabi-powered, highly pluggable TUI “information rolodex” that unifies multiple read-mostly information streams into one terminal UI and daemon. It is part of the raibid-labs ecosystem (Scarab, Hibana, Tolaria, Phage, Fusabi).

High-level intent:

- Scryforge is a **kernel + plugins** system built around Fusabi.
- It has a **daemon (“hub”)** that talks to providers and exposes a local API.
- It has a **TUI client** (Ratatui-based) that talks to that daemon.
- It is **read-only for MVP** for content (emails, feeds, posts, etc.) but may do *limited* write operations later for “library-ish” operations (e.g. playlists, bookmarks).
- The **core abstraction** is “streams of items”: email inboxes, RSS feeds, saved items, playlists, subscriptions, bookmarks, etc.
- Scryforge should grow the Fusabi ecosystem by factoring shared pieces into `fusabi-*` crates/packages (streams, auth client, TUI widgets).

Key concepts and capabilities:

- Core primitives:
  - `Stream`: a logical feed or collection (Inbox, Playlist, Saved items, Subreddit, RSS feed, etc.).
  - `Item`: an entry in a stream (email, RSS article, YouTube video, Spotify track, Reddit post, bookmark, task, calendar entry).
  - `Action`: operations that can be taken on an item (read-only for MVP: open, preview, copy link, open in browser, tag locally; eventual R/W for playlists, bookmarks, etc.).
- Provider capabilities (traits that different services implement):
  - `HasFeeds`    → lists logical feeds and retrieves items for each (email inboxes, RSS feeds, Medium feeds, Reddit home/subreddits, YouTube subscriptions).
  - `HasCollections` → lists named collections with ordered items (Spotify/YouTube playlists, bookmark folders).
  - `HasSavedItems` → “saved/bookmarked/liked” items across services (Reddit saved, Medium bookmarks, YouTube watch-later, Spotify liked, bookmarks).
  - `HasCommunities` → membership/subscriptions (subreddits, channels, Medium publications, RSS feed list, etc.).
- MVP should include **read-only viewing** from a small set of providers:
  - Email via IMAP (Gmail + Outlook/IMAP, maybe not full Gmail API yet).
  - RSS feeds (including Medium via RSS).
  - Microsoft To Do and Calendar via Microsoft Graph (read-only).
  - Spotify (playlists, liked tracks).
  - YouTube (subscriptions, playlists, watch-later).
  - Reddit (home/subs, saved posts).
  - Bookmarks (local store and/or integration with tools like `buku`).
- Scryforge should NOT implement complex HTML email rendering, full composer, or heavy write flows in MVP. Focus on **navigation, display, and cross-stream views**.

Architecture constraints:

- Language: Rust + Fusabi (daemon plugins compiled to `.fzb`, client-side Fusabi `.fsx` for hot-reload, similar to other Fusabi-based projects).
- The daemon:
  - Runs provider plugins (likely in-process Rust crates at first).
  - Owns sync, caching, and calls to Sigilforge (the separate auth manager daemon) for tokens and secrets.
  - Exposes a simple local API (Unix socket or TCP; JSON-RPC or similar) for the TUI and for future consumers like Scarab.
- The TUI:
  - Uses Ratatui for layout and rendering.
  - Focuses on “explorer-style” navigation inspired by tools like yazi/broot: sidebars, lists, preview pane, fast filtering, omnibar/command palette.
  - Communicates only with the daemon API, not directly with providers.

Fusabi ecosystem angle:

- As part of scaffolding, create **stub crates** that can later be extracted or shared as `fusabi-community` packages, for example:
  - `fusabi-streams-core` → traits and types for `Stream`, `Item`, `Action`, `HasFeeds`, `HasCollections`, `HasSavedItems`, `HasCommunities`.
  - `fusabi-tui-core` → basic TUI event loop/state wiring for Ratatui-based Fusabi apps.
  - `fusabi-tui-widgets` → reusable widgets (stream list, item list, preview pane, status bar, omnibar).
  - (These can initially live in this repo, but should be structured so they could be published independently later.)

What I want you to do now:

1. **Design a Rust workspace layout** for Scryforge that includes:
   - A top-level `Cargo.toml` with workspace members for:
     - `scryforge-daemon` (the hub).
     - `scryforge-tui` (the TUI client).
     - `fusabi-streams-core` (core traits/types).
     - `fusabi-tui-core` and `fusabi-tui-widgets` (minimal stubs).
     - A placeholder `providers/` directory for future provider crates (e.g., `provider-email-imap`, `provider-rss`, `provider-spotify`, etc.).
   - A `docs/` directory with initial design docs.

2. **Create the following documentation files** (content, not just filenames):
   - `README.md`  
     - High-level description of Scryforge.
     - Explanation of the daemon + TUI architecture.
     - Summary of the core abstractions (Stream, Item, Actions, provider capabilities).
     - MVP scope (read-only, which providers, what is explicitly out-of-scope).
   - `docs/ARCHITECTURE.md`  
     - More detailed architecture: workspace layout, crate responsibilities, daemon API high-level shape, plugin model.
     - Explanation of how Scryforge expects to talk to Sigilforge for auth (`auth://service/account` style references).
   - `docs/ROADMAP.md`  
     - Phased roadmap:
       - Phase 0: scaffolding and dummy provider.
       - Phase 1: IMAP email + RSS + basic TUI.
       - Phase 2: MS To Do/Calendar + Spotify/YouTube + Reddit + Bookmarks.
       - Phase 3: unified “Saved”, “Playlists”, “Feeds” views.
       - Phase 4+: optional write operations (playlists/bookmarks), AI/agent integrations via Scarab.
   - `docs/PROVIDERS.md`  
     - Describe the provider capability model (`HasFeeds`, `HasCollections`, etc.).
     - List targeted providers and how they map to these capabilities.

3. **Scaffold minimal Rust crates**:
   - For each workspace member, create:
     - `Cargo.toml` with appropriate package name, dependencies (just the minimal ones for now), and edition.
     - `src/lib.rs` or `src/main.rs` with:
       - For the daemon: a placeholder async main that just logs startup.
       - For the TUI: a placeholder main that explains what the TUI will eventually do.
       - For the core `fusabi-*` crates: basic trait and type stubs with `TODO` comments.

4. **Create a `NEXT_STEPS.md` aimed at future Claude sessions** that:
   - Lists concrete next tasks (e.g., implement basic daemon API types, prototype one `HasFeeds` provider, wire TUI to a dummy provider).
   - Explains how future tooling should extend the scaffolding without breaking the architecture.

Important:

- Be explicit and concrete in the docs: these are for future AI-assisted development, so they should carry enough detail that another assistant can pick up the work without seeing this prompt.
- When generating code, keep it minimal and idiomatic; focus on clear structure and naming rather than completeness.
- You may assume that Sigilforge will exist as a separate project responsible for auth/token management, exposed via a local API or library, but you do NOT need to implement Sigilforge here—just reference it in docs and module layout where appropriate.

Now, please:
- Propose the workspace layout.
- Generate the initial `README.md` and docs listed above.
- Create minimal `Cargo.toml` and `src/*` stubs for each crate as described.
- Summarize what you created at the end so I can see the structure at a glance.


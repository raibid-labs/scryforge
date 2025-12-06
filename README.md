# Scryforge

Scryforge is a Fusabi-powered, highly pluggable **information rolodex** for the terminal.

It provides a unified TUI + daemon for reading and navigating information streams from many
services:

- Email (Gmail / Outlook via IMAP)
- RSS / Atom feeds (including Medium via RSS)
- Bookmarks and saved links
- Media and subscriptions (Spotify, YouTube, Reddit, etc.)
- Productivity data (Microsoft To Do, Microsoft Calendar via Graph)
- Other future providers (GitHub, X/Twitter, etc.)

Scryforge focuses on **read-mostly** workflows for MVP: you can *view* and *triage* information
across services in one place, with optional light write operations (like playlist or bookmark
management) added later.

Scryforge is part of the raibid-labs ecosystem alongside projects like:

- **Fusabi** — F#-inspired scripting embedded in Rust
- **Scarab** — agent / automation infrastructure
- **Tolaria** — local experimentation / dev environment
- **Phage** — transformation/processing engine
- **Sigilforge** — auth, secrets, and token management daemon used by Scryforge

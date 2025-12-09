# Scryforge Command Reference

This document provides a comprehensive reference for all commands and search syntax available in the Scryforge omnibar.

## Table of Contents

- [Omnibar Overview](#omnibar-overview)
- [Commands](#commands)
  - [Application Commands](#application-commands)
  - [Sync Commands](#sync-commands)
  - [View Commands](#view-commands)
  - [Plugin Commands](#plugin-commands)
- [Search Syntax](#search-syntax)
  - [Simple Search](#simple-search)
  - [Advanced Search Filters](#advanced-search-filters)
  - [Field-Specific Search](#field-specific-search)
  - [Status Filters](#status-filters)
  - [Date Filters](#date-filters)
  - [Boolean Operators](#boolean-operators)
- [Autocomplete](#autocomplete)
- [Examples](#examples)

## Omnibar Overview

The omnibar is the command and search bar at the bottom of the TUI. It has two modes:

### Search Mode

Activated by pressing `/`:
- Immediately start typing your search query
- Input does **not** start with `:` prefix
- Executed as a search when you press `Enter`

```
/rust programming
```

### Command Mode

Activated by pressing `:`:
- Input starts with `:` prefix
- Autocomplete suggestions appear as you type
- Executed as a command when you press `Enter`

```
:quit
```

### Basic Usage

1. Press `/` or `:` to activate the omnibar
2. Type your search query or command
3. Press `Enter` to execute
4. Press `Esc` to cancel

## Commands

All commands are prefixed with `:` and can be typed in the omnibar.

### Application Commands

Control the TUI application lifecycle.

#### `:quit` (aliases: `:q`, `:exit`)

Exit the application.

```
:quit
:q
:exit
```

**Description**: Closes the TUI and terminates the client connection to the daemon. The daemon continues running.

#### `:help` (alias: `:h`)

Show help information.

```
:help
:h
```

**Description**: Displays comprehensive help text in the status bar or preview pane (implementation varies). For detailed help, refer to this documentation.

### Sync Commands

Trigger synchronization with data providers.

#### `:sync`

Sync all enabled providers.

```
:sync
```

**Description**: Requests the daemon to synchronize data from all enabled providers. Sync intervals are configured per-provider in `config.toml`, but this command triggers an immediate sync regardless of the schedule.

**Status**: Shows "Syncing all providers..." in the status bar.

#### `:sync <provider>`

Sync a specific provider by name.

```
:sync reddit
:sync rss
:sync email
:sync my provider
```

**Description**: Triggers an immediate sync for only the specified provider. Provider names can contain spaces and are case-sensitive (matches the provider ID in the configuration).

**Examples**:
```
:sync dummy          # Sync the dummy provider
:sync reddit         # Sync Reddit provider
:sync my custom rss  # Multi-word provider name
```

**Alias**: `:s` can be used as shorthand:
```
:s
:s reddit
```

### View Commands

Refresh and manipulate the current view.

#### `:refresh` (alias: `:r`)

Refresh the current view by reloading data from the daemon.

```
:refresh
:r
```

**Description**: Fetches fresh data from the daemon for the current view. This doesn't trigger provider syncs, but re-queries the local cache. Useful for seeing updates after a background sync completes.

### Plugin Commands

Manage provider plugins and extensions.

#### `:plugin` (alias: `:plugins`)

List all loaded plugins.

```
:plugin
:plugins
```

**Description**: Shows information about all loaded provider plugins in the system. Equivalent to `:plugin list`.

#### `:plugin list` (alias: `:plugin ls`)

List all loaded plugins.

```
:plugin list
:plugin ls
```

**Description**: Displays a list of all provider plugins known to the daemon, including their enabled/disabled status.

**Output**: Shows plugin list in status bar or preview pane (implementation varies).

#### `:plugin enable <id>`

Enable a specific plugin.

```
:plugin enable rss
:plugin enable my-custom-provider
```

**Description**: Enables the specified provider plugin, allowing it to sync data. The plugin must already be installed and known to the daemon.

**Alias**: `:plugin on <id>`

```
:plugin on reddit
```

**Requirements**: Plugin ID must be provided. Use `:plugin list` to see available plugin IDs.

#### `:plugin disable <id>`

Disable a specific plugin.

```
:plugin disable reddit
:plugin disable spotify
```

**Description**: Disables the specified provider plugin, preventing it from syncing data. Existing cached data remains accessible until cleared.

**Alias**: `:plugin off <id>`

```
:plugin off email
```

**Requirements**: Plugin ID must be provided.

#### `:plugin info <id>` (alias: `:plugin show <id>`)

Show detailed information about a specific plugin.

```
:plugin info rss
:plugin show reddit
```

**Description**: Displays metadata and capabilities for the specified provider plugin, including:
- Plugin name and version
- Supported capabilities (feeds, collections, saved items, etc.)
- Current configuration
- Sync status

**Requirements**: Plugin ID must be provided.

#### `:plugin reload` (alias: `:plugin refresh`)

Reload all plugins from disk.

```
:plugin reload
:plugin refresh
```

**Description**: Hot-reloads all provider plugins without restarting the daemon. Useful during development or after installing new provider plugins.

**Note**: This may temporarily interrupt ongoing syncs.

## Search Syntax

Search queries do **not** use the `:` prefix. Activate search mode with `/` and type your query.

### Simple Search

Just type words to search across all fields (title, content, etc.).

```
rust programming
getting started guide
kubernetes deployment
```

**Description**: Performs a full-text search across title, content, and other indexed fields. Results are ranked by relevance.

### Advanced Search Filters

#### Provider Filter: `provider:<name>`

Filter results to a specific provider.

```
provider:reddit rust
provider:rss kubernetes
```

**Description**: Only show items from the specified provider.

**Negation**: Use `-provider:` to exclude a provider:

```
-provider:reddit rust
```

Shows all Rust content **except** from Reddit.

### Stream Filter: `in:<stream>` or `stream:<stream>`

Filter to a specific stream.

```
in:inbox urgent
stream:my-playlist rock music
```

**Description**: Only show items from the specified stream. Stream names are provider-specific (e.g., inbox for email, subreddit name for Reddit, playlist name for Spotify).

### Field-Specific Search

#### Title Search: `title:<keyword>`

Search only in item titles.

```
title:kubernetes
title:"getting started"
```

**Description**: Restricts search to the title field. Useful for finding items by headline.

#### Content Search: `content:<keyword>`

Search only in item content/body.

```
content:async
content:"error handling"
```

**Description**: Restricts search to the content/body field. Useful for finding items by their full text.

### Type Filter: `type:<content-type>`

Filter by content type.

```
type:article
type:email
type:video
type:track
```

**Supported types**:
- `article` - Articles, blog posts, RSS items
- `email` - Email messages
- `video` - YouTube videos, video content
- `track` / `song` / `music` - Spotify tracks, audio content
- `task` / `todo` - Microsoft To-Do tasks
- `event` / `calendar` - Calendar events
- `bookmark` - Bookmarks

**Example**:
```
type:video kubernetes tutorial
```

### Status Filters

#### Read Status: `is:read` or `is:unread`

Filter by read/unread status.

```
is:unread
is:read
```

**Example**:
```
is:unread urgent
```

Shows all unread items containing "urgent".

#### Saved Status: `is:saved` (aliases: `is:starred`, `is:favorite`)

Filter to saved/bookmarked items.

```
is:saved
is:starred
is:favorite
```

**Example**:
```
is:saved kubernetes
```

Shows all saved items about Kubernetes.

### Date Filters

#### Relative Date: `since:<duration>`

Show items from the last N days/weeks/months.

```
since:7d      # Last 7 days
since:30d     # Last 30 days
since:2w      # Last 2 weeks
since:3m      # Last 3 months
```

**Supported units**:
- `d` - days
- `w` - weeks
- `m` - months

**Example**:
```
since:7d is:unread
```

Shows unread items from the last 7 days.

#### Absolute Date: `date:<date>`

Show items from a specific date.

```
date:2024-01-15
date:2024-12-01
```

**Format**: ISO 8601 date format (`YYYY-MM-DD`)

#### Date Range: `date:<start>..<end>`

Show items within a date range.

```
date:2024-01-01..2024-06-30
date:2024-12-01..2024-12-31
```

**Format**: Two ISO 8601 dates separated by `..`

**Example**:
```
date:2024-01-01..2024-06-30 type:article
```

Shows all articles from the first half of 2024.

### Boolean Operators

**Note**: Advanced boolean operators are passed to the daemon's FTS5 parser. The TUI preserves the full query string.

#### Quoted Phrases: `"exact phrase"`

Search for an exact phrase.

```
"getting started"
"hello world"
```

**Description**: Finds items containing the exact phrase, in that order.

**Example**:
```
"rust async" since:30d
```

Shows items with the exact phrase "rust async" from the last 30 days.

#### AND (implicit)

Multiple terms without operators are implicitly AND-ed.

```
rust programming
kubernetes docker
```

Equivalent to:
```
rust AND programming
kubernetes AND docker
```

#### Combining Filters

All filters can be combined in a single query:

```
title:kubernetes -provider:reddit is:unread since:7d
provider:rss type:article "getting started"
in:inbox is:unread urgent
```

## Autocomplete

The omnibar provides autocomplete suggestions for commands as you type.

### How It Works

1. Press `:` to enter command mode
2. Start typing a command
3. Suggestions appear below the omnibar
4. Continue typing or press `Enter` to execute

### Suggestion Format

Suggestions show the command and a brief description:

```
:q - Exit (short)
:quit - Exit the application
:sync - Sync all providers
:sync <provider> - Sync specific provider
```

### Autocomplete Examples

Typing `:q` shows:
- `:quit - Exit the application`
- `:q - Exit (short)`

Typing `:s` shows:
- `:sync - Sync all providers`
- `:sync <provider> - Sync specific provider`

Typing `:h` shows:
- `:help - Show help`
- `:h - Help (short)`

Typing `:plugin` shows:
- `:plugin list - List loaded plugins`
- `:plugin enable <id> - Enable a plugin`
- `:plugin disable <id> - Disable a plugin`
- `:plugin info <id> - Show plugin details`
- `:plugin reload - Reload plugins`

## Examples

### Common Use Cases

#### Exit the application
```
:quit
:q
```

#### Search for Rust content
```
/rust
```

#### Find unread Kubernetes articles
```
/kubernetes is:unread type:article
```

#### Sync Reddit provider
```
:sync reddit
```

#### Find recent saved items
```
/is:saved since:7d
```

#### Search titles only
```
/title:"getting started"
```

#### Find urgent unread emails
```
/in:inbox is:unread urgent
```

#### Exclude Reddit from search
```
/rust programming -provider:reddit
```

#### Find items from a specific date range
```
/date:2024-01-01..2024-06-30 type:article
```

#### Show plugin information
```
:plugin info rss
```

#### Refresh the current view
```
:refresh
:r
```

### Advanced Queries

#### Complex multi-filter search
```
/title:kubernetes -provider:reddit is:unread since:7d type:article
```

**Translation**: Find articles with "kubernetes" in the title, not from Reddit, that are unread, from the last 7 days.

#### Exact phrase with filters
```
/"async await" provider:rss since:30d
```

**Translation**: Find RSS items with the exact phrase "async await" from the last 30 days.

#### Multiple status filters
```
/is:unread is:saved important
```

**Translation**: Find items that are both unread and saved, containing "important".

#### Stream-specific search
```
/in:my-playlist rock since:7d
```

**Translation**: Find items in "my-playlist" stream containing "rock" from the last 7 days.

## Error Handling

### Unknown Commands

If you enter a command that doesn't exist, you'll see:
```
Unknown command: <your-command>
```

**Solution**: Check the command syntax. Use `:help` to see available commands.

### Missing Arguments

Some commands require arguments:
```
:plugin enable     # Error: requires plugin ID
:sync             # OK: syncs all providers
:sync reddit      # OK: syncs specific provider
```

**Solution**: Provide the required argument or use `:help` for syntax.

### Invalid Filters

Search filters are parsed gracefully. Invalid filters are treated as search terms:
```
/type:invalid rust
```

Will search for "rust" and ignore the invalid type filter (or treat "invalid" as a search term).

## Tips and Tricks

### Quick Commands

- Learn the short aliases: `:q`, `:s`, `:r`, `:h`
- Use autocomplete: Type `:s` and press `Tab` to see sync options
- Commands are case-insensitive: `:QUIT` works the same as `:quit`

### Efficient Searching

- Start broad, then refine: `/rust` → `/rust is:unread` → `/rust is:unread since:7d`
- Use title search for faster results: `title:` is more efficient than full-text search
- Combine type and provider filters: `type:article provider:rss`

### Common Workflows

**Check unread email:**
```
/in:inbox is:unread
```

**Find saved programming articles:**
```
/is:saved type:article programming
```

**Review recent important items:**
```
/important since:7d
```

**Search within a specific stream:**
```
/in:rust-subreddit is:unread
```

## Troubleshooting

### Omnibar won't activate

- **Check mode**: Press `Esc` to ensure you're in normal mode
- **Check focus**: Omnibar works from any pane

### Search returns no results

- **Check spelling**: Typos in search terms
- **Too many filters**: Try removing some filters
- **Wrong provider**: Check `provider:` filter is correct

### Command doesn't execute

- **Check prefix**: Commands need `:` prefix
- **Check syntax**: Use autocomplete to verify command exists
- **Check arguments**: Some commands require arguments

### Autocomplete not showing

- **Must start with :**: Only command mode has autocomplete
- **Type more**: Suggestions appear as you type
- **Implementation note**: Autocomplete is basic; not all partial matches shown

## See Also

- [KEYBINDINGS.md](KEYBINDINGS.md) - Complete keyboard shortcut reference
- [GETTING_STARTED.md](GETTING_STARTED.md) - Basic usage guide
- [CONFIGURATION.md](CONFIGURATION.md) - Provider configuration
- [ARCHITECTURE.md](ARCHITECTURE.md) - Search implementation details

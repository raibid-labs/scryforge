# Scryforge Configuration Guide

This document describes the configuration file format for `scryforge-daemon` and all available configuration options.

## Table of Contents

- [Configuration File Location](#configuration-file-location)
- [File Format](#file-format)
- [Configuration Sections](#configuration-sections)
  - [Daemon Configuration](#daemon-configuration)
  - [Cache Configuration](#cache-configuration)
  - [Provider Configuration](#provider-configuration)
- [Example Configurations](#example-configurations)
- [Validation Rules](#validation-rules)
- [Environment Variables](#environment-variables)

## Configuration File Location

Scryforge follows the XDG Base Directory Specification for configuration:

- **Linux/macOS**: `$XDG_CONFIG_HOME/scryforge/config.toml` (defaults to `~/.config/scryforge/config.toml`)
- **Windows**: `%APPDATA%\raibid-labs\scryforge\config\config.toml`

On first run, if the configuration file doesn't exist, `scryforge-daemon` automatically creates a default configuration with helpful comments.

## File Format

The configuration file uses [TOML](https://toml.io) format, which is human-readable and easy to edit. The file is organized into three main sections:

1. `[daemon]` - Daemon server settings
2. `[cache]` - Cache and database settings
3. `[providers.*]` - Per-provider configuration

## Configuration Sections

### Daemon Configuration

The `[daemon]` section controls the daemon server behavior.

```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "info"
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `bind_address` | String | `"127.0.0.1:3030"` | Socket address for the JSON-RPC API server. Use `127.0.0.1` for localhost-only, or `0.0.0.0` to allow external connections (not recommended). |
| `log_level` | String | `"info"` | Logging verbosity level. Valid values: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`. |

#### Log Levels Explained

- **trace**: Most verbose, shows all execution details
- **debug**: Detailed information for debugging
- **info**: General informational messages (recommended default)
- **warn**: Warning messages for potential issues
- **error**: Only error messages

#### Examples

Development mode with verbose logging:
```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "debug"
```

Production mode with minimal logging:
```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "warn"
```

Custom port:
```toml
[daemon]
bind_address = "127.0.0.1:8080"
log_level = "info"
```

### Cache Configuration

The `[cache]` section controls data caching and persistence.

```toml
[cache]
# path = "/custom/path/to/cache.db"  # Optional: custom cache location
max_items_per_stream = 1000
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `path` | String (Optional) | `$XDG_DATA_HOME/scryforge/cache.db` | Path to the SQLite cache database. If not specified, uses the XDG data directory (`~/.local/share/scryforge/cache.db` on Linux/macOS). |
| `max_items_per_stream` | Integer | `1000` | Maximum number of items to cache per stream. Older items are automatically pruned when this limit is exceeded. Must be greater than 0. |

#### Examples

Default cache location:
```toml
[cache]
max_items_per_stream = 1000
```

Custom cache location:
```toml
[cache]
path = "/mnt/data/scryforge/cache.db"
max_items_per_stream = 5000
```

Large cache for heavy users:
```toml
[cache]
max_items_per_stream = 10000
```

Minimal cache for limited storage:
```toml
[cache]
max_items_per_stream = 100
```

### Provider Configuration

Each provider has its own configuration section under `[providers.<provider-id>]`. All providers share common configuration options, plus provider-specific settings.

#### Common Provider Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | Boolean | `true` | Whether this provider is active. Set to `false` to disable without removing the configuration. |
| `sync_interval_minutes` | Integer | `15` | How often to sync data from this provider, in minutes. Must be greater than 0. |
| `settings` | Table | `{}` | Provider-specific settings (varies by provider). |

#### Provider Configuration Template

```toml
[providers.<provider-id>]
enabled = true
sync_interval_minutes = 15

[providers.<provider-id>.settings]
# Provider-specific settings go here
```

#### Dummy Provider Example

The dummy provider is used for testing and development:

```toml
[providers.dummy]
enabled = true
sync_interval_minutes = 15

[providers.dummy.settings]
# No special settings required for dummy provider
```

#### RSS Provider Example (Future)

```toml
[providers.rss]
enabled = true
sync_interval_minutes = 30

[providers.rss.settings]
feeds = [
    "https://example.com/feed.xml",
    "https://blog.rust-lang.org/feed.xml",
    "https://www.reddit.com/r/rust/.rss",
]
# Optional: custom user agent
user_agent = "Scryforge RSS Reader/1.0"
# Optional: request timeout in seconds
timeout_seconds = 30
```

#### Email (IMAP) Provider Example (Future)

```toml
[providers.email]
enabled = true
sync_interval_minutes = 5

[providers.email.settings]
imap_server = "imap.gmail.com"
imap_port = 993
use_tls = true
# Credentials are retrieved from Sigilforge, not stored in config
username = "user@example.com"
```

#### Reddit Provider Example (Future)

```toml
[providers.reddit]
enabled = true
sync_interval_minutes = 15

[providers.reddit.settings]
# OAuth credentials retrieved from Sigilforge
subreddits = ["rust", "programming", "linux"]
include_saved = true
include_home = true
# Optional: limit posts per subreddit
post_limit = 50
```

#### Spotify Provider Example (Future)

```toml
[providers.spotify]
enabled = true
sync_interval_minutes = 60

[providers.spotify.settings]
# OAuth credentials retrieved from Sigilforge
include_playlists = true
include_liked_tracks = true
include_recently_played = true
# Optional: specific playlist IDs to sync
playlist_ids = ["37i9dQZF1DXcBWIGoYBM5M"]
```

#### YouTube Provider Example (Future)

```toml
[providers.youtube]
enabled = true
sync_interval_minutes = 30

[providers.youtube.settings]
# OAuth credentials retrieved from Sigilforge
include_subscriptions = true
include_watch_later = true
include_liked_videos = true
# Optional: limit videos per channel
video_limit = 20
```

#### Microsoft To-Do Provider Example (Future)

```toml
[providers.mstodo]
enabled = true
sync_interval_minutes = 10

[providers.mstodo.settings]
# OAuth credentials retrieved from Sigilforge
include_all_lists = true
# Optional: specific list IDs
list_ids = ["AQMkADAwATM0MDAAMS1iMjQ1LTQwOGMtMDACLTAwCgAuAAAD..."]
```

## Example Configurations

### Minimal Configuration

The absolute minimum configuration required:

```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "info"

[cache]
max_items_per_stream = 1000
```

### Default Configuration

This is what the daemon creates on first run:

```toml
# Scryforge Daemon Configuration
# This file configures the scryforge-daemon behavior.

[daemon]
# Bind address for the JSON-RPC API server
# Default: "127.0.0.1:3030"
bind_address = "127.0.0.1:3030"

# Log level: trace, debug, info, warn, error
# Default: "info"
log_level = "info"

[cache]
# Path to the SQLite cache database
# If not specified, defaults to $XDG_DATA_HOME/scryforge/cache.db
# path = "/path/to/cache.db"

# Maximum number of items to keep per stream
# Default: 1000
max_items_per_stream = 1000

# Provider-specific configurations
# Each provider can be configured with:
# - enabled: Whether the provider is enabled (default: true)
# - sync_interval_minutes: How often to sync data (default: 15)
# - settings: Provider-specific settings (varies by provider)

# Example: Dummy provider configuration
[providers.dummy]
enabled = true
sync_interval_minutes = 15

# Provider-specific settings are defined here
[providers.dummy.settings]
# Add provider-specific settings as needed
# For the dummy provider, no special settings are required
```

### Full Featured Configuration

Example with multiple providers configured:

```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "debug"

[cache]
path = "/home/user/scryforge/cache.db"
max_items_per_stream = 5000

# RSS feeds
[providers.rss]
enabled = true
sync_interval_minutes = 30

[providers.rss.settings]
feeds = [
    "https://blog.rust-lang.org/feed.xml",
    "https://lwn.net/headlines/rss",
]

# Email
[providers.email]
enabled = true
sync_interval_minutes = 5

[providers.email.settings]
imap_server = "imap.gmail.com"
imap_port = 993
use_tls = true
username = "user@gmail.com"

# Reddit
[providers.reddit]
enabled = true
sync_interval_minutes = 15

[providers.reddit.settings]
subreddits = ["rust", "programming"]
include_saved = true
include_home = true

# Spotify
[providers.spotify]
enabled = false  # Disabled
sync_interval_minutes = 60

[providers.spotify.settings]
include_playlists = true
include_liked_tracks = true
```

### Development Configuration

Optimized for development with verbose logging and frequent syncs:

```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "debug"  # Verbose logging

[cache]
max_items_per_stream = 100  # Smaller cache for faster iteration

[providers.dummy]
enabled = true
sync_interval_minutes = 1  # Frequent syncs for testing
```

### Production Configuration

Optimized for production use with minimal logging:

```toml
[daemon]
bind_address = "127.0.0.1:3030"
log_level = "warn"  # Only warnings and errors

[cache]
path = "/var/lib/scryforge/cache.db"
max_items_per_stream = 10000  # Large cache

[providers.rss]
enabled = true
sync_interval_minutes = 60  # Hourly syncs

[providers.rss.settings]
feeds = ["https://example.com/feed.xml"]
timeout_seconds = 60
```

## Validation Rules

The daemon validates the configuration on startup. The following rules are enforced:

### Daemon Section

- `bind_address` must be a valid socket address (e.g., `127.0.0.1:3030`, `0.0.0.0:8080`)
- `log_level` must be one of: `trace`, `debug`, `info`, `warn`, `error` (case-insensitive)

### Cache Section

- `max_items_per_stream` must be greater than 0
- `path` (if specified) must be a valid file path

### Provider Sections

- `sync_interval_minutes` must be greater than 0 for each provider
- Provider-specific settings vary by provider implementation

### Validation Errors

If validation fails, the daemon will exit with an error message:

```
Error: Invalid bind_address: not-a-valid-address
Error: Invalid log_level: invalid. Must be one of: trace, debug, info, warn, error
Error: cache.max_items_per_stream must be greater than 0
Error: Provider 'rss': sync_interval_minutes must be greater than 0
```

## Environment Variables

While most configuration is done via the TOML file, some environment variables affect behavior:

| Variable | Description |
|----------|-------------|
| `XDG_CONFIG_HOME` | Base directory for configuration files (default: `~/.config`) |
| `XDG_DATA_HOME` | Base directory for data files (default: `~/.local/share`) |
| `RUST_LOG` | Override log level (takes precedence over `log_level` in config) |

### Example with Environment Variables

```bash
# Use custom config directory
export XDG_CONFIG_HOME=/custom/config
scryforge-daemon

# Override log level
export RUST_LOG=debug
scryforge-daemon

# Custom data directory for cache
export XDG_DATA_HOME=/mnt/data
scryforge-daemon
```

## Hot Reloading

**Note**: Configuration hot-reloading is planned but not yet implemented. Currently, you must restart the daemon for configuration changes to take effect:

```bash
# Kill the daemon
pkill scryforge-daemon

# Restart with new configuration
scryforge-daemon
```

## Troubleshooting

### Configuration not found

If you see "Failed to read config file", ensure:
- The file exists at the correct path
- You have read permissions
- The path uses correct separators for your OS

### Invalid TOML syntax

TOML is whitespace-sensitive. Common issues:
- Missing quotes around strings with special characters
- Incorrect indentation
- Missing equals signs

Use a TOML validator or editor with TOML support.

### Validation errors

If the daemon fails to start with validation errors:
- Check that all required fields are present
- Verify values are within acceptable ranges
- Ensure `bind_address` is a valid socket address

### Default configuration

To regenerate the default configuration:
1. Delete or rename your existing config file
2. Restart the daemon - it will create a fresh default config

```bash
mv ~/.config/scryforge/config.toml ~/.config/scryforge/config.toml.bak
scryforge-daemon
```

## See Also

- [GETTING_STARTED.md](GETTING_STARTED.md) - Installation and first run guide
- [PROVIDERS.md](PROVIDERS.md) - Detailed provider capability model
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture overview

# scarab-scryforge

Scarab plugin for Scryforge integration.

This plugin connects to the scryforge-daemon via JSON-RPC and provides status bar integration and menu actions for the Scarab terminal emulator.

## Features

- **Status Bar Integration**: Shows unread item count in the Scarab status bar
- **Health Monitoring**: Visual indicators for daemon connection health
- **Menu Actions**:
  - Sync All - Trigger synchronization for all providers
  - Mark All Read - Mark all items as read across all feeds
  - Open TUI - Launch the scryforge-tui interface
  - Refresh Status - Update the status bar immediately

## Installation

Add the plugin to your Scarab configuration:

```toml
[[plugins]]
name = "scryforge"
path = "/path/to/scarab-scryforge/target/release/libscarab_scryforge.so"
enabled = true
```

## Configuration

The plugin expects scryforge-daemon to be running on `http://127.0.0.1:3030` by default.

## Status Bar Display

The plugin displays information in the following format:

- **Healthy**: `📬 5 unread | 30m ago`
- **Unhealthy**: `📬 5 unread ⚠`
- **No unread**: `📬 0 unread`

Colors follow the Catppuccin Mocha theme:
- Green (#a6e3a1) - Healthy connection
- Blue (#89b4fa) - Unread count
- Yellow (#f9e2af) - Unhealthy connection
- Red (#f38ba8) - Warning indicator

## Background Updates

The plugin automatically updates the status every 30 seconds by:
- Checking daemon health
- Fetching unread counts
- Updating sync timestamps

## Dependencies

- `scarab-plugin-api` - Scarab plugin API
- `jsonrpsee` - JSON-RPC client for daemon communication
- `tokio` - Async runtime
- `chrono` - Time handling

## Development

Build the plugin:

```bash
cargo build -p scarab-scryforge --release
```

Run tests:

```bash
cargo test -p scarab-scryforge
```

## License

MIT OR Apache-2.0

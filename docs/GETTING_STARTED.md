# Getting Started with Scryforge

This guide will help you install, configure, and start using Scryforge - a terminal-based information rolodex that unifies multiple data streams into a single interface.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [First Run](#first-run)
- [Basic Usage](#basic-usage)
- [Next Steps](#next-steps)

## Prerequisites

Before installing Scryforge, ensure you have the following:

- **Rust**: Version 1.70 or later. Install via [rustup.rs](https://rustup.rs)
- **Operating System**: Linux, macOS, or Windows with WSL
- **Terminal**: Any modern terminal emulator with Unicode support

To verify your Rust installation:

```bash
rustc --version
cargo --version
```

## Installation

### Building from Source

1. **Clone the repository:**

```bash
git clone https://github.com/raibid-labs/scryforge.git
cd scryforge
```

2. **Build the project:**

```bash
# Build all workspace crates
cargo build --release

# Binaries will be in target/release/
```

3. **Install binaries (optional):**

```bash
# Install to ~/.cargo/bin (make sure it's in your PATH)
cargo install --path scryforge-daemon
cargo install --path scryforge-tui
```

### Verify Installation

Check that the binaries are available:

```bash
# If installed
scryforge-daemon --version
scryforge-tui --version

# Or run directly from the repository
cargo run --bin scryforge-daemon -- --version
cargo run --bin scryforge-tui -- --version
```

## First Run

Scryforge uses a daemon + TUI architecture. You'll need to run both components.

### Step 1: Start the Daemon

The daemon manages data providers, caching, and serves the API for the TUI.

```bash
# From the repository root
cargo run --bin scryforge-daemon

# Or if installed
scryforge-daemon
```

On first run, the daemon will:
- Create a default configuration file at `~/.config/scryforge/config.toml` (or `$XDG_CONFIG_HOME/scryforge/config.toml`)
- Initialize the cache database at `~/.local/share/scryforge/cache.db` (or `$XDG_DATA_HOME/scryforge/cache.db`)
- Start the JSON-RPC server on `127.0.0.1:3030`

You should see log output indicating the daemon has started:

```
INFO scryforge_daemon: Starting scryforge-daemon
INFO scryforge_daemon: Created default configuration file at: /home/user/.config/scryforge/config.toml
INFO scryforge_daemon: Daemon listening on 127.0.0.1:3030
```

Keep the daemon running in this terminal.

### Step 2: Launch the TUI

In a **new terminal**, start the TUI client:

```bash
# From the repository root
cargo run --bin scryforge-tui

# Or if installed
scryforge-tui
```

The TUI will connect to the daemon and display the three-pane interface:

```
┌─────────────────────────────────────────────────────────────────────┐
│                          scryforge-tui                               │
│  ┌───────────┐ ┌─────────────────────┐ ┌─────────────────────────┐ │
│  │  Streams  │ │       Items         │ │        Preview          │ │
│  │           │ │                     │ │                         │ │
│  │ > Inbox   │ │ ● Subject line...   │ │  Email Title            │ │
│  │   RSS     │ │   Another item...   │ │                         │ │
│  │   Reddit  │ │   Third item...     │ │  From: sender@...       │ │
│  │   Spotify │ │                     │ │  Date: 2024-01-15       │ │
│  │           │ │                     │ │                         │ │
│  │           │ │                     │ │  Body content here...   │ │
│  └───────────┘ └─────────────────────┘ └─────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Type to search or press : for commands...                      ││
│  └─────────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Connected to daemon | Press ? for help                          ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

## Basic Usage

### Navigation

Scryforge uses vim-style keybindings for navigation:

- **Switch between panes**: Press `Tab` or `l` (right), `Shift+Tab` or `h` (left)
- **Navigate lists**: Press `j` (down) or `k` (up), or use arrow keys `↓`/`↑`
- **Jump to first/last**: Press `g` (first) or `G` (last)
- **Focus omnibar**: Press `/` (search) or `:` (commands)
- **Show help**: Press `?` to see all keybindings in the status bar
- **Quit**: Press `q`

### Viewing Items

1. **Navigate to a stream** in the left sidebar using `j`/`k`
2. The middle pane will populate with items from that stream
3. **Navigate items** using `j`/`k` in the middle pane
4. The right preview pane automatically shows details for the selected item

Items are automatically marked as read when selected.

### Basic Actions

- **Mark as read/unread**: Press `r` (toggles read status)
- **Save/unsave item**: Press `s` (toggles saved status)
- **Archive item**: Press `e` (removes from view)
- **Add to collection**: Press `a` (opens collection picker)

### Searching

Press `/` to activate the search omnibar. You can:

- **Simple search**: Just type text: `rust programming`
- **Advanced filters**: Use special syntax (see [COMMANDS.md](COMMANDS.md) for full syntax)

Examples:
```
title:kubernetes                  # Search in titles only
is:unread                         # Show only unread items
provider:reddit                   # Filter by provider
since:7d                          # Items from last 7 days
```

Press `Enter` to execute the search, or `Esc` to cancel.

### Commands

Press `:` to activate command mode. Common commands:

- `:quit` or `:q` - Exit the application
- `:sync` - Sync all providers
- `:refresh` or `:r` - Refresh the current view
- `:help` or `:h` - Show help

See [COMMANDS.md](COMMANDS.md) for the complete command reference.

## Current State

**Note**: Scryforge is in active development. The current version includes:

### What Works
- TUI renders with three-pane layout
- Keyboard navigation with vim-style keys
- Daemon starts and serves JSON-RPC API
- Basic provider system (currently with dummy data)
- Item marking (read/unread, save/unsave)
- Omnibar with search and command parsing

### What's In Progress
- Real provider implementations (RSS, Email, Reddit, etc.)
- Persistent caching and sync
- Advanced search with filters
- Configuration hot-reloading

See the [ROADMAP.md](ROADMAP.md) for the full development plan.

## Next Steps

Now that you have Scryforge running, you can:

1. **Configure providers**: Edit `~/.config/scryforge/config.toml` to enable and configure data providers. See [CONFIGURATION.md](CONFIGURATION.md)
2. **Learn keybindings**: Review all available keyboard shortcuts in [KEYBINDINGS.md](KEYBINDINGS.md)
3. **Master commands**: Explore the full command palette in [COMMANDS.md](COMMANDS.md)
4. **Understand architecture**: Read [ARCHITECTURE.md](ARCHITECTURE.md) to learn how Scryforge works
5. **Contribute**: Check [ROADMAP.md](ROADMAP.md) for planned features and open issues

## Troubleshooting

### Daemon won't start

- Check if port 3030 is already in use: `lsof -i :3030` or `netstat -tuln | grep 3030`
- Check the log output for specific errors
- Verify configuration file syntax: `cat ~/.config/scryforge/config.toml`

### TUI can't connect to daemon

- Ensure the daemon is running in another terminal
- Check that the daemon is listening on `127.0.0.1:3030`
- Verify no firewall is blocking localhost connections

### Configuration file not found

- The configuration is automatically created on first run
- Check the path: `~/.config/scryforge/config.toml` on Linux/macOS
- Create the directory manually if needed: `mkdir -p ~/.config/scryforge`

### Display issues

- Ensure your terminal supports Unicode and 256 colors
- Try a different terminal emulator
- Check terminal size: Scryforge requires at least 80x24 characters

## Getting Help

- **In-app help**: Press `?` while in the TUI
- **Documentation**: Browse the `docs/` directory
- **Issues**: Report bugs at https://github.com/raibid-labs/scryforge/issues
- **Discussions**: Ask questions in GitHub Discussions

## Quick Reference

| Action | Key |
|--------|-----|
| Move between panes | `Tab`, `h`, `l` |
| Navigate up/down | `j`, `k`, `↑`, `↓` |
| Jump to first/last | `g`, `G` |
| Search | `/` |
| Commands | `:` |
| Toggle read/unread | `r` |
| Save/unsave | `s` |
| Archive | `e` |
| Add to collection | `a` |
| Help | `?` |
| Quit | `q` |

For a complete keybinding reference, see [KEYBINDINGS.md](KEYBINDINGS.md).

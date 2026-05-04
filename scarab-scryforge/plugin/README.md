# scarab-scryforge Fusabi plugin (Phase A)

This directory holds the Fusabi-language (`.fsx`) version of the
scarab-scryforge plugin, tracked upstream at
[scarab#253](https://github.com/raibid-labs/scarab/issues/253).

## Status

Phase A only. Validates the `.fsx -> .fzb -> daemon load` path end to end
without any JSON-RPC integration or VM host bindings. The status bar text
"📬 3 unread" you see when the plugin loads is broadcast by the scarab
daemon as a temporary bridge while the .fzb adapter learns to call
`add_status_item` directly.

## Building

```bash
# From the scarab repo
cargo run -p scarab-plugin-compiler -- /path/to/scryforge/scarab-scryforge/plugin/scryforge.fsx

# Or copy the .fzb output into your scarab plugin path
SCARAB_PLUGIN_PATH=/path/to/this/plugin/dir cargo run -p scarab-daemon
```

## Relationship to the Rust crate

The Rust `ScryforgePlugin` in `../src/lib.rs` already implements what's
deferred here to Phase B/C: JSON-RPC client, 30s polling, menu actions
(`Sync All`, `Mark All Read`, `Open TUI`, `Refresh Status`), health
monitoring, etc.

The `.fsx` version exists as a forward-looking placeholder. Once the
Fusabi VM gains:

- a synchronous `net_http` binding usable from a hook (Phase B)
- host bindings for `add_status_item`, menu, focusables (Phase B/C)

this script grows to cover the same surface as the Rust crate, and the
Rust crate becomes a thin shell that loads the `.fzb`.

## Canonical source

The canonical copy of this `.fsx` lives in the scarab repo at
`examples/fusabi/scryforge.fsx`. This file is a mirror; updates should
land there first and then propagate here.

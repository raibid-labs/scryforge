# Plugin Development Guide

This guide explains how to create Fusabi plugins for Scryforge. Plugins allow you to extend Scryforge with new providers, actions, and functionality without modifying the core daemon.

## Table of Contents

1. [Overview](#overview)
2. [Plugin Structure](#plugin-structure)
3. [Manifest Format](#manifest-format)
4. [Capability System](#capability-system)
5. [Bytecode Format](#bytecode-format)
6. [Plugin Types](#plugin-types)
7. [Creating a Plugin](#creating-a-plugin)
8. [Testing](#testing)
9. [Distribution](#distribution)

## Overview

Fusabi plugins are dynamically-loaded extensions that run in the Scryforge daemon. They are compiled to Fusabi bytecode (.fzb) and include a manifest file declaring metadata and required capabilities.

**Key Features**:
- Sandboxed execution with capability-based security
- Hot-reload support (planned)
- No daemon recompilation required
- Standard plugin API via `fusabi-plugin-api`

**Plugin Locations**:
```
~/.local/share/scryforge/plugins/     # User plugins
/usr/share/scryforge/plugins/         # System-wide plugins
```

## Plugin Structure

Each plugin is a directory containing at minimum:

```
my-plugin/
├── manifest.toml       # Plugin metadata and configuration
└── plugin.fzb          # Compiled Fusabi bytecode
```

Optional files:

```
my-plugin/
├── manifest.toml
├── plugin.fzb
├── README.md           # Plugin documentation
├── LICENSE             # License file
└── assets/             # Static assets (icons, etc.)
    └── icon.png
```

### Directory Naming

Plugin directories should match the plugin ID in kebab-case:

```
spotify-extended/       # Plugin ID: spotify-extended
my-custom-feed/         # Plugin ID: my-custom-feed
github-issues/          # Plugin ID: github-issues
```

## Manifest Format

The `manifest.toml` file declares plugin metadata and configuration.

### Basic Structure

```toml
[plugin]
id = "my-plugin"
name = "My Plugin"
version = "1.0.0"
description = "A custom Scryforge plugin"
authors = ["Author Name <author@example.com>"]
license = "MIT"
homepage = "https://github.com/user/my-plugin"
repository = "https://github.com/user/my-plugin"
plugin_type = "provider"
entry_point = "plugin.fzb"

capabilities = ["network", "credentials"]

[provider]
id = "my-provider"
display_name = "My Provider"
has_feeds = true
has_collections = false
has_saved_items = false
has_communities = false
oauth_provider = "myservice"

[rate_limit]
requests_per_second = 10.0
max_concurrent = 5
retry_delay_ms = 1000

[config]
api_base_url = "https://api.example.com"
timeout_seconds = 30
```

### Plugin Metadata (`[plugin]`)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique plugin identifier (kebab-case) |
| `name` | String | Yes | Human-readable name |
| `version` | String | Yes | Semantic version (e.g., "1.0.0") |
| `description` | String | No | Brief description of plugin functionality |
| `authors` | Array[String] | No | Plugin authors |
| `license` | String | No | License identifier (e.g., "MIT", "Apache-2.0") |
| `homepage` | String | No | Plugin homepage URL |
| `repository` | String | No | Source code repository URL |
| `plugin_type` | String | No | Plugin type: "provider", "action", "theme", "extension" (default: "provider") |
| `entry_point` | String | No | Bytecode file name (default: "plugin.fzb") |

### Capabilities

Plugins must declare all required capabilities:

```toml
capabilities = [
    "network",        # HTTP/HTTPS requests
    "credentials",    # Access to OAuth tokens
    "cache_read",     # Read from Scryforge cache
]
```

See [Capability System](#capability-system) for all available capabilities.

### Provider Configuration (`[provider]`)

For provider plugins only:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Provider ID (must match plugin ID) |
| `display_name` | String | No | Display name (defaults to plugin name) |
| `icon` | String | No | Icon name or path |
| `has_feeds` | Boolean | No | Supports feeds (default: false) |
| `has_collections` | Boolean | No | Supports collections (default: false) |
| `has_saved_items` | Boolean | No | Supports saved items (default: false) |
| `has_communities` | Boolean | No | Supports communities (default: false) |
| `oauth_provider` | String | No | OAuth provider name for Sigilforge |

### Rate Limiting (`[rate_limit]`)

Optional rate limiting configuration:

| Field | Type | Description |
|-------|------|-------------|
| `requests_per_second` | Float | Maximum requests per second |
| `max_concurrent` | Integer | Maximum concurrent requests |
| `retry_delay_ms` | Integer | Delay in milliseconds after rate limit hit |

### Custom Configuration (`[config]`)

Add any custom key-value pairs for your plugin:

```toml
[config]
api_base_url = "https://api.example.com"
api_version = "v2"
timeout_seconds = 30
max_items_per_page = 100
cache_ttl_minutes = 15
```

Access these values in your plugin code.

## Capability System

The capability system controls what permissions plugins have. Plugins can only use capabilities they declare in their manifest.

### Available Capabilities

| Capability | Description |
|------------|-------------|
| `network` | Make HTTP/HTTPS requests to external services |
| `file_read` | Read files from the filesystem |
| `file_write` | Write files to the filesystem |
| `environment` | Access environment variables |
| `process` | Spawn subprocesses |
| `credentials` | Access OAuth tokens and credentials via Sigilforge |
| `cache_read` | Read from the Scryforge cache database |
| `cache_write` | Write to the Scryforge cache database |
| `notifications` | Send notifications to the user |
| `clipboard` | Access the system clipboard |
| `open_url` | Open URLs in the default browser |

### Declaring Capabilities

In `manifest.toml`:

```toml
# Minimal permissions (no external access)
capabilities = []

# Read-only data provider
capabilities = ["network", "credentials", "cache_write"]

# Full-featured provider with notifications
capabilities = [
    "network",
    "credentials",
    "cache_read",
    "cache_write",
    "notifications",
    "open_url",
]
```

### Runtime Enforcement

The Fusabi runtime will enforce capabilities at execution time (implementation planned):

```rust
// Plugin code
http_get("https://example.com")  // ✓ Allowed if "network" capability declared
                                  // ✗ Rejected at runtime if not declared

read_file("/etc/passwd")          // ✓ Allowed if "file_read" declared
                                  // ✗ Rejected otherwise
```

### Best Practices

1. **Principle of Least Privilege**: Only request capabilities you need
2. **Document Requirements**: Explain why each capability is needed
3. **Avoid Broad Permissions**: Don't request `file_write` if you only need `cache_write`

## Bytecode Format

Fusabi bytecode (.fzb) is a compiled representation of your plugin code.

### Format Overview

```
+----------------+
| Magic (4 bytes)|  "FZB\x01" (version 1)
+----------------+
| Metadata       |  JSON: plugin_id, version, compiled_at, compiler_version
+----------------+
| Constants      |  Constant pool: strings, numbers, etc.
+----------------+
| Functions      |  Function definitions with parameters
+----------------+
| Instructions   |  Bytecode instructions
+----------------+
```

### Current Implementation

The current implementation uses JSON encoding for development:

```json
{
  "version": 1,
  "metadata": {
    "plugin_id": "my-plugin",
    "plugin_version": "1.0.0",
    "compiled_at": "2025-01-15T10:30:00Z",
    "compiler_version": "0.1.0"
  },
  "constants": [
    {"type": "String", "value": "Hello, World!"},
    {"type": "Int", "value": 42}
  ],
  "functions": [
    {
      "name": "main",
      "params": [],
      "local_count": 0,
      "instructions": [
        {"op": "LoadConst", "index": 0},
        {"op": "Return"}
      ]
    }
  ],
  "entry_point": "main"
}
```

### Instruction Set

Available bytecode instructions:

| Instruction | Arguments | Description |
|-------------|-----------|-------------|
| `LoadConst` | index: usize | Load constant from pool |
| `LoadLocal` | index: usize | Load local variable |
| `StoreLocal` | index: usize | Store to local variable |
| `LoadGlobal` | name: String | Load global variable |
| `StoreGlobal` | name: String | Store to global variable |
| `Call` | name: String, arg_count: usize | Call function |
| `CallMethod` | name: String, arg_count: usize | Call method on object |
| `Return` | - | Return from function |
| `Jump` | offset: i32 | Unconditional jump |
| `JumpIfFalse` | offset: i32 | Jump if top of stack is false |
| `Pop` | - | Pop value from stack |
| `Dup` | - | Duplicate top of stack |
| `Add` | - | Binary addition |
| `Sub` | - | Binary subtraction |
| `Mul` | - | Binary multiplication |
| `Div` | - | Binary division |
| `Eq` | - | Equality comparison |
| `Ne` | - | Not equal comparison |
| `Lt` | - | Less than comparison |
| `Le` | - | Less than or equal |
| `Gt` | - | Greater than comparison |
| `Ge` | - | Greater than or equal |
| `Not` | - | Logical NOT |
| `And` | - | Logical AND |
| `Or` | - | Logical OR |
| `MakeArray` | count: usize | Create array from stack |
| `MakeObject` | count: usize | Create object from stack |
| `GetProperty` | name: String | Get object property |
| `SetProperty` | name: String | Set object property |
| `GetIndex` | - | Array/object indexing |
| `SetIndex` | - | Set array/object index |
| `Await` | - | Await async value |
| `Nop` | - | No operation |

### Example: Hello World Plugin

```json
{
  "version": 1,
  "metadata": {
    "plugin_id": "hello-world",
    "plugin_version": "1.0.0"
  },
  "constants": [
    {"type": "String", "value": "Hello, World!"}
  ],
  "functions": [
    {
      "name": "get_message",
      "params": [],
      "local_count": 0,
      "instructions": [
        {"op": "LoadConst", "index": 0},
        {"op": "Return"}
      ]
    }
  ],
  "entry_point": "get_message"
}
```

## Plugin Types

### Provider Plugin

Provides data from external services (feeds, collections, etc.).

```toml
[plugin]
plugin_type = "provider"

[provider]
id = "my-provider"
has_feeds = true
```

**Required Functions**:
- `health_check() -> HealthResult`
- `sync() -> SyncResult`
- `list_feeds() -> Vec<Feed>` (if `has_feeds = true`)
- `get_feed_items(feed_id: String) -> Vec<Item>` (if `has_feeds = true`)

### Action Plugin

Adds custom actions that can be performed on items.

```toml
[plugin]
plugin_type = "action"
```

**Required Functions**:
- `get_actions(item: Item) -> Vec<Action>`
- `execute_action(item: Item, action: String) -> ActionResult`

### Theme Plugin

Customizes TUI appearance (future).

```toml
[plugin]
plugin_type = "theme"
```

### Extension Plugin

Generic extension for custom functionality.

```toml
[plugin]
plugin_type = "extension"
```

## Creating a Plugin

### Step 1: Design Your Plugin

1. Choose plugin type (provider, action, theme, extension)
2. Identify required capabilities
3. Plan data structures and API integration

### Step 2: Create Plugin Directory

```bash
mkdir -p ~/.local/share/scryforge/plugins/my-plugin
cd ~/.local/share/scryforge/plugins/my-plugin
```

### Step 3: Write Manifest

Create `manifest.toml`:

```toml
[plugin]
id = "my-plugin"
name = "My Custom Plugin"
version = "1.0.0"
description = "A custom provider for My Service"
authors = ["Your Name <you@example.com>"]
license = "MIT"
plugin_type = "provider"

capabilities = ["network", "credentials", "cache_write"]

[provider]
id = "my-plugin"
display_name = "My Service"
has_feeds = true
oauth_provider = "myservice"

[rate_limit]
requests_per_second = 10.0

[config]
api_base_url = "https://api.myservice.com"
```

### Step 4: Write Plugin Code

Currently, create JSON bytecode manually (compiler planned):

Create `plugin.fzb`:

```json
{
  "version": 1,
  "metadata": {
    "plugin_id": "my-plugin",
    "plugin_version": "1.0.0"
  },
  "constants": [
    {"type": "String", "value": "My Service"},
    {"type": "String", "value": "https://api.myservice.com"}
  ],
  "functions": [
    {
      "name": "health_check",
      "params": [],
      "local_count": 0,
      "instructions": [
        {"op": "LoadConst", "index": 0},
        {"op": "Call", "name": "http_get", "arg_count": 1},
        {"op": "Return"}
      ]
    },
    {
      "name": "list_feeds",
      "params": [],
      "local_count": 0,
      "instructions": [
        {"op": "LoadConst", "index": 1},
        {"op": "Call", "name": "http_get", "arg_count": 1},
        {"op": "Call", "name": "parse_feeds", "arg_count": 1},
        {"op": "Return"}
      ]
    }
  ],
  "entry_point": "health_check"
}
```

### Step 5: Add with Magic Bytes (Optional)

For binary format, prepend magic bytes:

```bash
# Add FZB magic bytes to the beginning
printf 'FZB\x01' > plugin.fzb.tmp
cat plugin.fzb >> plugin.fzb.tmp
mv plugin.fzb.tmp plugin.fzb
```

### Step 6: Validate Plugin

Check that your plugin loads correctly:

```bash
# The daemon will log plugin discovery on startup
scryforge-daemon

# Look for:
# INFO fusabi_runtime: Discovered plugin: my-plugin v1.0.0
# INFO fusabi_runtime: Loaded plugin: my-plugin
```

## Testing

### Manifest Validation

Test manifest parsing:

```rust
use fusabi_runtime::PluginManifest;

#[test]
fn test_manifest() {
    let manifest = PluginManifest::from_file("manifest.toml").unwrap();

    assert_eq!(manifest.plugin.id, "my-plugin");
    assert_eq!(manifest.plugin.version, "1.0.0");
    assert!(manifest.capabilities.contains(&"network".to_string()));
}
```

### Bytecode Validation

Test bytecode loading:

```rust
use fusabi_runtime::{BytecodeLoader, Bytecode};

#[test]
fn test_bytecode() {
    let bytecode = BytecodeLoader::load("plugin.fzb").unwrap();

    assert_eq!(bytecode.version, 1);
    assert_eq!(bytecode.metadata.plugin_id, "my-plugin");
    assert!(!bytecode.functions.is_empty());

    BytecodeLoader::validate(&bytecode).unwrap();
}
```

### Integration Testing

Test with the daemon:

```bash
# Start daemon with plugin
scryforge-daemon

# In another terminal, use scryforge-tui to test
scryforge-tui

# Check that your provider appears in the streams list
```

## Distribution

### Packaging

Create a tarball for distribution:

```bash
cd ~/.local/share/scryforge/plugins
tar -czf my-plugin-1.0.0.tar.gz my-plugin/
```

### Installation

Users install by extracting to their plugins directory:

```bash
cd ~/.local/share/scryforge/plugins
tar -xzf my-plugin-1.0.0.tar.gz
```

Or system-wide:

```bash
sudo tar -xzf my-plugin-1.0.0.tar.gz -C /usr/share/scryforge/plugins/
```

### Version Compatibility

Declare minimum Scryforge version in manifest:

```toml
[plugin]
min_scryforge_version = "0.2.0"
```

## Advanced Topics

### State Management

Plugins should store state in the cache:

```json
{
  "op": "Call",
  "name": "cache_set",
  "arg_count": 2
}
```

### OAuth Integration

Request tokens via Sigilforge:

```json
{
  "op": "Call",
  "name": "fetch_token",
  "arg_count": 2
}
```

### Error Handling

Return errors from functions:

```json
{
  "op": "Call",
  "name": "error",
  "arg_count": 1
}
```

### Async Operations

Use `Await` instruction for async calls:

```json
[
  {"op": "Call", "name": "http_get", "arg_count": 1},
  {"op": "Await"},
  {"op": "Return"}
]
```

## Troubleshooting

### Plugin Not Loading

Check daemon logs:

```bash
scryforge-daemon 2>&1 | grep -i plugin
```

Common issues:
- Invalid manifest format (check TOML syntax)
- Invalid bytecode (check JSON format)
- Missing capabilities in manifest
- Plugin ID mismatch between manifest and bytecode

### Runtime Errors

Enable debug logging:

```bash
RUST_LOG=debug scryforge-daemon
```

Check for:
- Capability violations
- Invalid function calls
- Stack underflow/overflow
- Type mismatches

### Plugin Conflicts

If multiple plugins have the same ID, the first loaded wins:

```
WARN fusabi_runtime: Plugin 'my-plugin' already loaded, skipping duplicate
```

Ensure unique plugin IDs.

## Future Enhancements

Planned features for the plugin system:

1. **Fusabi Compiler**: Compile from a high-level language to bytecode
2. **Hot Reload**: Reload plugins without restarting daemon
3. **Plugin Store**: Central repository for discovering plugins
4. **Dependency Management**: Plugins depending on other plugins
5. **Binary Bytecode**: Optimized binary format for production
6. **Sandboxing**: Enhanced security with process isolation
7. **Plugin API Extensions**: More runtime functions for plugins

## Resources

- [Fusabi Runtime Source](../crates/fusabi-runtime/src/)
- [Plugin API Documentation](../crates/fusabi-plugin-api/)
- [Architecture Overview](./ARCHITECTURE.md)
- [Provider Development Guide](./PROVIDER_DEVELOPMENT.md)

## Example Plugins

### Minimal Provider

```toml
[plugin]
id = "minimal-provider"
name = "Minimal Provider"
version = "1.0.0"
plugin_type = "provider"

capabilities = []

[provider]
id = "minimal-provider"
has_feeds = true
```

```json
{
  "version": 1,
  "metadata": {"plugin_id": "minimal-provider", "plugin_version": "1.0.0"},
  "constants": [
    {"type": "String", "value": "Test Feed"}
  ],
  "functions": [
    {
      "name": "list_feeds",
      "params": [],
      "local_count": 0,
      "instructions": [
        {"op": "MakeArray", "count": 0},
        {"op": "Return"}
      ]
    }
  ],
  "entry_point": "list_feeds"
}
```

This creates a provider with no feeds.

## Next Steps

1. Design your plugin following this guide
2. Create manifest and bytecode files
3. Test with the daemon
4. Package and distribute
5. Submit to the plugin repository (when available)

Happy plugin development!

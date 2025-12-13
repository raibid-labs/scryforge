# Scarab Integration Plan

This document outlines what's needed to integrate Scryforge and Sigilforge as Scarab plugins.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         SCARAB                                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ scarab-scryforge│  │ scarab-sigilforge│  │  other plugins  │ │
│  │     plugin      │  │     plugin       │  │                 │ │
│  └────────┬────────┘  └────────┬─────────┘  └─────────────────┘ │
│           │                    │                                 │
│  ┌────────┴────────────────────┴─────────┐                      │
│  │            Status Bar                  │                      │
│  │  [Scryforge: 3 unread] [Auth: ✓]      │                      │
│  └────────────────────────────────────────┘                      │
└─────────────────────────────────────────────────────────────────┘
           │                    │
           ▼                    ▼
┌──────────────────┐  ┌──────────────────┐
│ scryforge-daemon │  │ sigilforge-daemon│
│   (JSON-RPC)     │  │   (JSON-RPC)     │
└──────────────────┘  └──────────────────┘
```

## New Crates Needed

### 1. `scarab-scryforge` (New Plugin)

A Scarab plugin that:
- Connects to scryforge-daemon via JSON-RPC
- Shows unread counts / sync status in status bar
- Provides menu for quick actions (sync, mark read)
- Registers focusable regions for scarab-nav

```rust
// scarab-scryforge/src/lib.rs
pub struct ScryforgePlugin {
    metadata: PluginMetadata,
    client: Option<ScryforgeClient>,
    unread_count: usize,
    last_sync: Option<DateTime<Utc>>,
}

#[async_trait]
impl Plugin for ScryforgePlugin {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }

    async fn on_load(&mut self, ctx: &mut PluginContext) -> Result<()> {
        // Connect to scryforge-daemon
        self.client = Some(ScryforgeClient::connect("127.0.0.1:3030").await?);
        // Start background polling for status updates
        Ok(())
    }

    fn get_menu(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::new("Sync All", MenuAction::Remote("sync_all"))
                .with_icon("🔄"),
            MenuItem::new("Mark All Read", MenuAction::Remote("mark_all_read"))
                .with_icon("✓"),
            MenuItem::new("Open TUI", MenuAction::Command("scryforge-tui".into()))
                .with_icon("📺"),
        ]
    }

    // Status bar: show unread count
    fn get_status_items(&self) -> Vec<RenderItem> {
        vec![
            RenderItem::Icon("📬".to_string()),
            RenderItem::Text(format!("{} unread", self.unread_count)),
        ]
    }
}
```

### 2. `scarab-sigilforge` (New Plugin)

A Scarab plugin that:
- Shows auth status for configured accounts
- Provides menu to add/remove accounts
- Warns when tokens are expiring

```rust
// scarab-sigilforge/src/lib.rs
pub struct SigilforgePlugin {
    metadata: PluginMetadata,
    accounts: Vec<AccountStatus>,
}

struct AccountStatus {
    service: String,
    account: String,
    token_valid: bool,
    expires_soon: bool,
}

#[async_trait]
impl Plugin for SigilforgePlugin {
    fn get_menu(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::new("Add Account", MenuAction::SubMenu(vec![
                MenuItem::new("Google", MenuAction::Remote("add_google")),
                MenuItem::new("GitHub", MenuAction::Remote("add_github")),
                MenuItem::new("Spotify", MenuAction::Remote("add_spotify")),
            ])),
            MenuItem::new("List Accounts", MenuAction::Remote("list_accounts")),
        ]
    }

    fn get_status_items(&self) -> Vec<RenderItem> {
        let all_valid = self.accounts.iter().all(|a| a.token_valid);
        let color = if all_valid { "#a6e3a1" } else { "#f38ba8" };
        vec![
            RenderItem::Foreground(Color::Hex(color.to_string())),
            RenderItem::Icon(if all_valid { "🔐" } else { "⚠️" }.to_string()),
            RenderItem::Text(format!("{} accounts", self.accounts.len())),
        ]
    }
}
```

## Changes to Existing Projects

### scryforge-daemon

**No changes required for basic integration.** The JSON-RPC API is already sufficient.

Optional enhancements:
- Add `status.summary` RPC method returning unread counts per stream
- Add WebSocket support for push notifications (instead of polling)

### sigilforge

**Minor changes:**
- Ensure sigilforge-daemon exposes account status via RPC
- Add `accounts.status` method returning token validity and expiry

### scarab

**No core changes required.** Plugins use existing `scarab-plugin-api`.

### scarab-nav

**No changes required.** Plugins register focusables through existing API.

### fusabi

**No changes required.** Fusabi is for data provider plugins within scryforge, separate from scarab plugins.

## Implementation Phases

### Phase 1: Basic Status Bar Integration
- [ ] Create `scarab-scryforge` crate skeleton
- [ ] Implement `Plugin` trait with metadata
- [ ] Connect to scryforge-daemon on load
- [ ] Show basic status (connected/disconnected) in status bar
- [ ] Create `scarab-sigilforge` crate skeleton
- [ ] Show account count and auth status

### Phase 2: Menu Actions
- [ ] Add sync menu item for scryforge
- [ ] Add "Open TUI" menu item
- [ ] Add account management menu for sigilforge
- [ ] Handle `on_remote_command` callbacks

### Phase 3: Navigation Integration
- [ ] Register focusable regions for menu items
- [ ] Implement hint-based navigation
- [ ] Handle keyboard shortcuts

### Phase 4: Rich Features
- [ ] Background polling for unread counts
- [ ] Token expiry warnings
- [ ] Desktop notifications for new items
- [ ] Quick preview overlay

## File Structure

```
raibid-labs/
├── scarab/
│   └── (existing)
├── scarab-nav/
│   └── (existing)
├── scryforge/
│   ├── (existing)
│   └── scarab-scryforge/          # NEW
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── client.rs          # JSON-RPC client
│           └── status.rs          # Status bar rendering
├── sigilforge/
│   ├── (existing)
│   └── scarab-sigilforge/         # NEW
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── status.rs
```

## Dependencies

### scarab-scryforge
```toml
[dependencies]
scarab-plugin-api = { path = "../../scarab/crates/scarab-plugin-api" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
jsonrpsee = { version = "0.24", features = ["http-client"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = "0.4"
tracing = "0.1"
```

### scarab-sigilforge
```toml
[dependencies]
scarab-plugin-api = { path = "../../scarab/crates/scarab-plugin-api" }
sigilforge-client = { path = "../sigilforge-client" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
chrono = "0.4"
tracing = "0.1"
```

## Status Bar Layout Example

```
┌────────────────────────────────────────────────────────────────────┐
│ [left side]                                          [right side]  │
│ ~/project main ✓                    📬 3 unread │ 🔐 2 accounts │ │
└────────────────────────────────────────────────────────────────────┘
```

## Menu Structure

### Scryforge Menu
```
📬 Scryforge
├── 🔄 Sync All
├── ✓ Mark All Read
├── 📺 Open TUI
└── ⚙️ Settings
    ├── Sync Interval
    └── Notifications
```

### Sigilforge Menu
```
🔐 Sigilforge
├── ➕ Add Account
│   ├── Google
│   ├── GitHub
│   └── Spotify
├── 📋 List Accounts
└── 🗑️ Remove Account
```

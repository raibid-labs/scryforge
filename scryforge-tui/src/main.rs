//! # scryforge-tui
//!
//! The Scryforge terminal user interface.
//!
//! This TUI client provides an explorer-style interface for browsing information
//! streams managed by the scryforge-daemon. It features:
//!
//! - Three-pane layout: streams sidebar, item list, preview pane
//! - Vim-style keyboard navigation
//! - Fast filtering and search via omnibar
//! - Cross-stream unified views
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                          scryforge-tui                               │
//! │  ┌───────────┐ ┌─────────────────────┐ ┌─────────────────────────┐ │
//! │  │  Streams  │ │       Items         │ │        Preview          │ │
//! │  │           │ │                     │ │                         │ │
//! │  │ > Inbox   │ │ ● Subject line...   │ │  Email Title            │ │
//! │  │   RSS     │ │   Another item...   │ │                         │ │
//! │  │   Reddit  │ │   Third item...     │ │  From: sender@...       │ │
//! │  │   Spotify │ │                     │ │  Date: 2024-01-15       │ │
//! │  │           │ │                     │ │                         │ │
//! │  │           │ │                     │ │  Body content here...   │ │
//! │  │           │ │                     │ │                         │ │
//! │  └───────────┘ └─────────────────────┘ └─────────────────────────┘ │
//! │  ┌─────────────────────────────────────────────────────────────────┐│
//! │  │ : Type to search or press : for commands...                    ││
//! │  └─────────────────────────────────────────────────────────────────┘│
//! │  ┌─────────────────────────────────────────────────────────────────┐│
//! │  │ Ready | Connected to daemon | 3 providers active                ││
//! │  └─────────────────────────────────────────────────────────────────┘│
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Keyboard Shortcuts
//!
//! | Key | Action |
//! |-----|--------|
//! | `h/l` or `Tab` | Move focus between panes |
//! | `j/k` or `↑/↓` | Navigate within list |
//! | `Enter` | Open selected item |
//! | `/` | Focus omnibar for search |
//! | `:` | Focus omnibar for commands |
//! | `q` | Quit |
//! | `?` | Show help |
//!
//! ## Running
//!
//! ```bash
//! # Make sure daemon is running first
//! cargo run --bin scryforge-daemon &
//!
//! # Start the TUI
//! cargo run --bin scryforge-tui
//! ```

use anyhow::Result;
use crossterm::event::KeyCode;
use fusabi_tui_core::prelude::*;
use fusabi_tui_widgets::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use scryforge_provider_core::{Collection, Item, Stream};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub mod command;
mod daemon_client;
pub mod search;
use daemon_client::{get_daemon_url, spawn_client_task};
use daemon_client::{Command as DaemonCommand, Message};

fn main() -> Result<()> {
    // Initialize logging to file
    // TODO: Set up file-based logging properly
    // For now, we suppress output to avoid interfering with TUI

    // Create tokio runtime for async operations
    let rt = tokio::runtime::Runtime::new()?;

    // Run the TUI in the runtime
    rt.block_on(async_main())
}

async fn async_main() -> Result<()> {
    // Set up daemon client channels
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();

    // Spawn the daemon client task
    let daemon_url = get_daemon_url();
    let _client_handle = spawn_client_task(daemon_url, cmd_rx, msg_tx);

    // Initialize terminal
    let mut terminal = TerminalWrapper::new()?;

    // Create app state (starts empty, will be populated from daemon)
    let mut app = App::new(cmd_tx.clone());

    // Request initial data from daemon
    let _ = cmd_tx.send(DaemonCommand::FetchStreams);

    // Main event loop
    loop {
        // Render
        terminal.terminal().draw(|frame| {
            app.render(frame);
        })?;

        // Handle daemon messages (non-blocking)
        while let Ok(msg) = msg_rx.try_recv() {
            app.handle_daemon_message(msg);
        }

        // Handle UI events
        if let Some(event) = poll_event(std::time::Duration::from_millis(100))? {
            if !app.handle_event(event) {
                break;
            }
        }

        if app.should_quit() {
            break;
        }
    }

    // Send shutdown command to daemon client
    let _ = cmd_tx.send(DaemonCommand::Shutdown);

    // Cleanup handled by TerminalWrapper::Drop
    Ok(())
}

// ============================================================================
// Application State
// ============================================================================

struct App {
    streams: Vec<Stream>,
    items: Vec<Item>,
    collections: Vec<Collection>,
    stream_state: ListState,
    item_state: ListState,
    collection_state: ListState,
    focused: FocusedPane,
    omnibar_input: String,
    omnibar_active: bool,
    omnibar_suggestions: Vec<String>,
    collection_picker_active: bool,
    quit: bool,
    theme: Theme,
    status_message: String,
    cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
    daemon_connected: bool,
    provider_statuses: HashMap<String, ProviderSyncStatus>,
    toasts: Vec<Toast>,
    active_search_filter: Option<String>,
}

impl App {
    fn new(cmd_tx: mpsc::UnboundedSender<DaemonCommand>) -> Self {
        Self {
            streams: Vec::new(),
            items: Vec::new(),
            collections: Vec::new(),
            stream_state: ListState::new(0),
            item_state: ListState::new(0),
            collection_state: ListState::new(0),
            focused: FocusedPane::StreamList,
            omnibar_input: String::new(),
            omnibar_active: false,
            omnibar_suggestions: Vec::new(),
            collection_picker_active: false,
            quit: false,
            theme: Theme::default(),
            status_message: "Connecting to daemon...".to_string(),
            cmd_tx,
            daemon_connected: false,
            provider_statuses: HashMap::new(),
            toasts: Vec::new(),
            active_search_filter: None,
        }
    }

    fn handle_daemon_message(&mut self, msg: Message) {
        match msg {
            Message::Ready => {
                self.daemon_connected = true;
                self.status_message = "Connected to daemon - Press ? for help".to_string();
            }
            Message::StreamsLoaded(streams) => {
                let count = streams.len();
                self.streams = streams;
                self.stream_state = ListState::new(count);
                if count > 0 {
                    self.stream_state.select_first();
                    // Auto-fetch items for first stream
                    if let Some(stream) = self.streams.first() {
                        let _ = self
                            .cmd_tx
                            .send(DaemonCommand::FetchItems(stream.id.as_str().to_string()));
                    }
                }
                self.status_message = format!("Loaded {} streams", count);
            }
            Message::ItemsLoaded(items) => {
                let count = items.len();
                self.items = items;
                self.item_state = ListState::new(count);
                if count > 0 {
                    self.item_state.select_first();
                    // Auto-mark first item as read when items are loaded
                    self.auto_mark_selected_as_read();
                }
                self.status_message = format!("Loaded {} items", count);
            }
            Message::Error(err) => {
                self.status_message = format!("Error: {}", err);
                self.daemon_connected = false;
                self.add_toast(Toast::error(format!("Error: {}", err)));
            }
            Message::Disconnected => {
                self.status_message = "Disconnected from daemon".to_string();
                self.daemon_connected = false;
                self.add_toast(Toast::warning("Disconnected from daemon"));
            }
            Message::CollectionsLoaded(collections) => {
                let count = collections.len();
                self.collections = collections;
                self.collection_state = ListState::new(count);
                if count > 0 {
                    self.collection_state.select_first();
                }
                self.status_message = format!("Loaded {} collections", count);
            }
            Message::CollectionCreated(collection) => {
                self.status_message = format!("Created collection: {}", collection.name);
                self.add_toast(Toast::success(format!("Created: {}", collection.name)));
                // Refresh collections list
                let _ = self.cmd_tx.send(DaemonCommand::FetchCollections);
            }
            Message::ItemAddedToCollection => {
                self.status_message = "Item added to collection".to_string();
                self.collection_picker_active = false;
                self.add_toast(Toast::success("Added to collection"));
            }
            Message::ItemRemovedFromCollection => {
                self.status_message = "Item removed from collection".to_string();
                self.add_toast(Toast::success("Removed from collection"));
            }
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let size = frame.area();

        // Remove expired toasts
        self.toasts.retain(|t| !t.is_expired());

        // Main layout: content + omnibar + status bar
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Omnibar
                Constraint::Length(1), // Status bar
            ])
            .split(size);

        // Content layout: streams | items | preview
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20), // Streams
                Constraint::Percentage(35), // Items
                Constraint::Percentage(45), // Preview
            ])
            .split(main_chunks[0]);

        // Render streams
        StreamListWidget::new(&self.streams, self.stream_state.selected, &self.theme)
            .focused(self.focused == FocusedPane::StreamList)
            .render(frame, content_chunks[0]);

        // Render items
        ItemListWidget::new(&self.items, self.item_state.selected, &self.theme)
            .focused(self.focused == FocusedPane::ItemList)
            .render(frame, content_chunks[1]);

        // Render preview
        let selected_item = self.item_state.selected.and_then(|i| self.items.get(i));
        PreviewWidget::new(selected_item, &self.theme)
            .focused(self.focused == FocusedPane::Preview)
            .render(frame, content_chunks[2]);

        // Render omnibar
        OmnibarWidget::new(&self.omnibar_input, &self.theme)
            .active(self.omnibar_active)
            .suggestions(&self.omnibar_suggestions)
            .render(frame, main_chunks[1]);

        // Render enhanced status bar
        let connection_status = if self.daemon_connected {
            "Connected"
        } else {
            "Disconnected"
        };

        // Build provider status list from streams
        let provider_statuses: Vec<ProviderStatus> = self.get_unique_providers()
            .into_iter()
            .map(|provider_id| ProviderStatus {
                name: provider_id.clone(),
                sync_status: *self.provider_statuses.get(&provider_id).unwrap_or(&ProviderSyncStatus::Synced),
            })
            .collect();

        // Calculate total unread count
        let unread_count: u32 = self.streams
            .iter()
            .map(|s| s.unread_count.unwrap_or(0))
            .sum();

        StatusBarWidget::new(&self.status_message, connection_status, &self.theme)
            .provider_statuses(&provider_statuses)
            .unread_count(unread_count)
            .search_filter(self.active_search_filter.as_deref())
            .render(frame, main_chunks[2]);

        // Render toasts (overlay on top-right)
        if let Some(toast) = self.toasts.last() {
            let toast_area = self.calculate_toast_area(size);
            ToastWidget::new(toast, &self.theme).render(frame, toast_area);
        }
    }

    /// Get unique provider IDs from streams
    fn get_unique_providers(&self) -> Vec<String> {
        let mut providers: Vec<String> = self.streams
            .iter()
            .map(|s| s.provider_id.clone())
            .collect();
        providers.sort();
        providers.dedup();
        providers
    }

    /// Calculate the area for toast notification (top-right corner)
    fn calculate_toast_area(&self, screen_size: Rect) -> Rect {
        let width = 40.min(screen_size.width / 3);
        let height = 3;
        let x = screen_size.width.saturating_sub(width + 2);
        let y = 1;

        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Add a toast notification
    fn add_toast(&mut self, toast: Toast) {
        // Keep only the last 3 toasts
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(toast);
    }
}

impl AppState for App {
    fn handle_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Quit => {
                self.quit = true;
                return false;
            }
            AppEvent::Key(key) => {
                // Handle collection picker when active
                if self.collection_picker_active {
                    match key.code {
                        KeyCode::Esc => {
                            self.collection_picker_active = false;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            self.collection_state.select_next();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            self.collection_state.select_prev();
                        }
                        KeyCode::Enter => {
                            self.add_item_to_selected_collection();
                        }
                        _ => {}
                    }
                    return true;
                }

                // Handle omnibar input when active
                if self.omnibar_active {
                    match key.code {
                        KeyCode::Esc => {
                            self.omnibar_active = false;
                            self.omnibar_input.clear();
                            self.omnibar_suggestions.clear();
                        }
                        KeyCode::Enter => {
                            // Execute command or search
                            self.execute_omnibar_input();
                            self.omnibar_active = false;
                            self.omnibar_input.clear();
                            self.omnibar_suggestions.clear();
                        }
                        KeyCode::Backspace => {
                            self.omnibar_input.pop();
                            self.update_command_suggestions();
                        }
                        KeyCode::Char(c) => {
                            self.omnibar_input.push(c);
                            self.update_command_suggestions();
                        }
                        _ => {}
                    }
                    return true;
                }

                // Normal mode key handling
                match key.code {
                    KeyCode::Char('q') => {
                        self.quit = true;
                        return false;
                    }
                    KeyCode::Char('/') | KeyCode::Char(':') => {
                        self.omnibar_active = true;
                        if key.code == KeyCode::Char(':') {
                            self.omnibar_input.push(':');
                        }
                    }
                    KeyCode::Tab | KeyCode::Char('l') => {
                        self.focused = self.focused.next();
                    }
                    KeyCode::BackTab | KeyCode::Char('h') => {
                        self.focused = self.focused.prev();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.navigate_down();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.navigate_up();
                    }
                    KeyCode::Char('g') => {
                        self.navigate_first();
                    }
                    KeyCode::Char('G') => {
                        self.navigate_last();
                    }
                    KeyCode::Enter => {
                        // TODO: Open selected item
                        self.status_message = "Open item (not implemented)".to_string();
                    }
                    KeyCode::Char('s') => {
                        self.toggle_save_item();
                    }
                    KeyCode::Char('r') => {
                        self.toggle_read_status();
                    }
                    KeyCode::Char('e') => {
                        self.archive_selected_item();
                    }
                    KeyCode::Char('a') => {
                        self.show_collection_picker();
                    }
                    KeyCode::Char('d') => {
                        self.remove_item_from_current_collection();
                    }
                    KeyCode::Char('?') => {
                        self.status_message =
                            "h/l:panes j/k:nav /:search r:read/unread e:archive s:save a:add-to-collection d:remove-from-collection q:quit"
                                .to_string();
                    }
                    _ => {}
                }
            }
            AppEvent::Resize(_, _) => {
                // Ratatui handles resize automatically
            }
            AppEvent::Tick => {
                // TODO: Check for daemon updates
            }
        }
        true
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

impl App {
    fn navigate_down(&mut self) {
        match self.focused {
            FocusedPane::StreamList => {
                let old_selection = self.stream_state.selected;
                self.stream_state.select_next();
                // If stream changed, fetch items for new stream
                if old_selection != self.stream_state.selected {
                    self.fetch_items_for_selected_stream();
                }
            }
            FocusedPane::ItemList => {
                self.item_state.select_next();
                self.auto_mark_selected_as_read();
            }
            _ => {}
        }
    }

    fn navigate_up(&mut self) {
        match self.focused {
            FocusedPane::StreamList => {
                let old_selection = self.stream_state.selected;
                self.stream_state.select_prev();
                // If stream changed, fetch items for new stream
                if old_selection != self.stream_state.selected {
                    self.fetch_items_for_selected_stream();
                }
            }
            FocusedPane::ItemList => {
                self.item_state.select_prev();
                self.auto_mark_selected_as_read();
            }
            _ => {}
        }
    }

    fn navigate_first(&mut self) {
        match self.focused {
            FocusedPane::StreamList => {
                let old_selection = self.stream_state.selected;
                self.stream_state.select_first();
                if old_selection != self.stream_state.selected {
                    self.fetch_items_for_selected_stream();
                }
            }
            FocusedPane::ItemList => {
                self.item_state.select_first();
                self.auto_mark_selected_as_read();
            }
            _ => {}
        }
    }

    fn navigate_last(&mut self) {
        match self.focused {
            FocusedPane::StreamList => {
                let old_selection = self.stream_state.selected;
                self.stream_state.select_last();
                if old_selection != self.stream_state.selected {
                    self.fetch_items_for_selected_stream();
                }
            }
            FocusedPane::ItemList => {
                self.item_state.select_last();
                self.auto_mark_selected_as_read();
            }
            _ => {}
        }
    }

    fn fetch_items_for_selected_stream(&mut self) {
        if let Some(idx) = self.stream_state.selected {
            if let Some(stream) = self.streams.get(idx) {
                let _ = self
                    .cmd_tx
                    .send(DaemonCommand::FetchItems(stream.id.as_str().to_string()));
                self.status_message = format!("Loading items for {}...", stream.name);

                // Set provider to syncing status
                self.provider_statuses.insert(
                    stream.provider_id.clone(),
                    ProviderSyncStatus::Syncing,
                );
                self.add_toast(Toast::info(format!("Syncing {}...", stream.name)));
            }
        }
    }

    fn toggle_save_item(&mut self) {
        // Only toggle if we're focused on item list and have a selected item
        if self.focused != FocusedPane::ItemList {
            self.status_message = "Focus on item list to save/unsave".to_string();
            return;
        }

        if let Some(idx) = self.item_state.selected {
            if let Some(item) = self.items.get_mut(idx) {
                let item_id = item.id.as_str().to_string();
                let is_saved = item.is_saved;

                // Toggle saved state locally
                item.is_saved = !is_saved;

                // Send command to daemon
                if is_saved {
                    let _ = self.cmd_tx.send(DaemonCommand::UnsaveItem(item_id));
                    self.status_message = "Item unsaved".to_string();
                    self.add_toast(Toast::success("Unsaved"));
                } else {
                    let _ = self.cmd_tx.send(DaemonCommand::SaveItem(item_id));
                    self.status_message = "Item saved".to_string();
                    self.add_toast(Toast::success("Saved!"));
                }
            }
        }
    }

    fn toggle_read_status(&mut self) {
        if self.focused != FocusedPane::ItemList {
            self.status_message = "Focus on item list to mark read/unread".to_string();
            return;
        }

        if let Some(idx) = self.item_state.selected {
            if let Some(item) = self.items.get_mut(idx) {
                let new_read_status = !item.is_read;
                let item_id = item.id.as_str().to_string();

                // Update local state immediately for responsive UI
                item.is_read = new_read_status;

                // Send command to daemon
                let cmd = if new_read_status {
                    DaemonCommand::MarkItemRead(item_id)
                } else {
                    DaemonCommand::MarkItemUnread(item_id)
                };
                let _ = self.cmd_tx.send(cmd);

                self.status_message = format!(
                    "Marked as {}",
                    if new_read_status { "read" } else { "unread" }
                );
            }
        }
    }

    fn archive_selected_item(&mut self) {
        if self.focused != FocusedPane::ItemList {
            self.status_message = "Focus on item list to archive".to_string();
            return;
        }

        if let Some(idx) = self.item_state.selected {
            if let Some(item) = self.items.get(idx) {
                let item_id = item.id.as_str().to_string();
                let _ = self.cmd_tx.send(DaemonCommand::ArchiveItem(item_id));
                self.status_message = "Item archived".to_string();
                self.add_toast(Toast::success("Archived"));

                // Remove from current view
                self.items.remove(idx);
                self.item_state.update_len(self.items.len());
                // update_len will handle fixing the selection if idx is out of bounds
            }
        }
    }

    fn auto_mark_selected_as_read(&mut self) {
        if let Some(idx) = self.item_state.selected {
            if let Some(item) = self.items.get_mut(idx) {
                // Only mark as read if currently unread
                if !item.is_read {
                    let item_id = item.id.as_str().to_string();
                    item.is_read = true;
                    let _ = self.cmd_tx.send(DaemonCommand::MarkItemRead(item_id));
                }
            }
        }
    }

    fn show_collection_picker(&mut self) {
        if self.focused != FocusedPane::ItemList {
            self.status_message = "Focus on item list to add to collection".to_string();
            return;
        }

        if self.item_state.selected.is_none() {
            self.status_message = "No item selected".to_string();
            return;
        }

        // Fetch collections if not already loaded
        if self.collections.is_empty() {
            let _ = self.cmd_tx.send(DaemonCommand::FetchCollections);
            self.status_message = "Loading collections...".to_string();
        } else {
            self.collection_picker_active = true;
            self.status_message = "Select collection (j/k to navigate, Enter to add, Esc to cancel)".to_string();
        }
    }

    fn add_item_to_selected_collection(&mut self) {
        if let Some(item_idx) = self.item_state.selected {
            if let Some(collection_idx) = self.collection_state.selected {
                if let Some(item) = self.items.get(item_idx) {
                    if let Some(collection) = self.collections.get(collection_idx) {
                        let item_id = item.id.as_str().to_string();
                        let collection_id = collection.id.0.clone();
                        let _ = self.cmd_tx.send(DaemonCommand::AddToCollection {
                            collection_id,
                            item_id,
                        });
                        self.status_message = format!("Adding to collection: {}", collection.name);
                    }
                }
            }
        }
    }

    fn remove_item_from_current_collection(&mut self) {
        // This only works if we're viewing a collection stream
        // For now, show a message
        self.status_message = "Remove from collection not yet fully implemented".to_string();

        // TODO: Implement logic to detect if current stream is a collection
        // and remove the current item from it
    }

    /// Execute the current omnibar input as a command or search.
    fn execute_omnibar_input(&mut self) {
        use command::{parse_command, Command};

        let input = self.omnibar_input.trim();
        if input.is_empty() {
            return;
        }

        match parse_command(input) {
            Some(Command::Quit) => {
                self.quit = true;
            }
            Some(Command::Sync(provider)) => {
                match provider {
                    Some(ref p) => {
                        self.status_message = format!("Syncing provider: {}", p);
                        self.provider_statuses.insert(p.clone(), ProviderSyncStatus::Syncing);
                        self.add_toast(Toast::info(format!("Syncing {}...", p)));
                        // TODO: Send sync command to daemon when implemented
                        // For now, simulate completion
                        self.provider_statuses.insert(p.clone(), ProviderSyncStatus::Synced);
                        self.add_toast(Toast::success(format!("{} synced", p)));
                    }
                    None => {
                        self.status_message = "Syncing all providers...".to_string();
                        self.add_toast(Toast::info("Syncing all providers..."));
                        // Mark all providers as syncing
                        for provider_id in self.get_unique_providers() {
                            self.provider_statuses.insert(provider_id, ProviderSyncStatus::Syncing);
                        }
                        // TODO: Send sync command to daemon when implemented
                        // Simulate completion
                        for provider_id in self.get_unique_providers() {
                            self.provider_statuses.insert(provider_id, ProviderSyncStatus::Synced);
                        }
                        self.add_toast(Toast::success("Sync complete"));
                    }
                }
            }
            Some(Command::Refresh) => {
                self.status_message = "Refreshing...".to_string();
                self.add_toast(Toast::info("Refreshing..."));
                let _ = self.cmd_tx.send(DaemonCommand::FetchStreams);
            }
            Some(Command::Help) => {
                self.status_message = "Help: Type :h for commands, / for search".to_string();
                // TODO: Show help in a modal/preview pane
                // For now, just update status message with abbreviated help
            }
            Some(Command::Search(query)) => {
                self.execute_search(query);
            }
            Some(Command::Plugin(plugin_cmd)) => {
                self.handle_plugin_command(plugin_cmd);
            }
            Some(Command::Theme(theme_cmd)) => {
                self.handle_theme_command(theme_cmd);
            }
            None => {
                self.status_message = format!("Unknown command: {}", input);
                self.add_toast(Toast::warning(format!("Unknown command: {}", input)));
            }
        }
    }

    /// Handle plugin management commands.
    fn handle_plugin_command(&mut self, cmd: command::PluginCommand) {
        use command::PluginCommand;
        match cmd {
            PluginCommand::List => {
                self.status_message = "Plugin list: (fetching from daemon...)".to_string();
                // TODO: Send RPC to daemon to list plugins
                // For now show placeholder
            }
            PluginCommand::Enable(id) => {
                self.status_message = format!("Enabling plugin: {}", id);
                // TODO: Send RPC to daemon to enable plugin
            }
            PluginCommand::Disable(id) => {
                self.status_message = format!("Disabling plugin: {}", id);
                // TODO: Send RPC to daemon to disable plugin
            }
            PluginCommand::Info(id) => {
                self.status_message = format!("Plugin info for: {}", id);
                // TODO: Send RPC to daemon to get plugin info
            }
            PluginCommand::Reload => {
                self.status_message = "Reloading plugins...".to_string();
                // TODO: Send RPC to daemon to reload plugins
            }
        }
    }

    /// Handle theme management commands.
    fn handle_theme_command(&mut self, cmd: command::ThemeCommand) {
        use command::ThemeCommand;
        use fusabi_tui_widgets::theme::Theme;
        match cmd {
            ThemeCommand::List => {
                let themes = Theme::available_themes();
                self.status_message = format!("Available themes: {}", themes.join(", "));
                self.add_toast(Toast::info(format!("Themes: {}", themes.join(", "))));
            }
            ThemeCommand::Set(name) => {
                if let Some(new_theme) = Theme::by_name(&name) {
                    self.theme = new_theme;
                    self.status_message = format!("Theme changed to: {}", name);
                    self.add_toast(Toast::success(format!("Theme: {}", name)));
                } else {
                    self.status_message = format!("Unknown theme: {}. Use :theme list to see available themes", name);
                    self.add_toast(Toast::error(format!("Unknown theme: {}", name)));
                }
            }
        }
    }

    /// Execute a search query.
    fn execute_search(&mut self, query: search::SearchQuery) {
        if query.has_advanced_syntax {
            self.status_message = format!("Searching with filters: {}", query.text);
            self.active_search_filter = Some(query.text.clone());
            self.add_toast(Toast::info(format!("Searching: {}", query.text)));
            // TODO: Send search RPC to daemon
            // For now, just show the query in status
        } else {
            // Simple search: filter items locally
            self.status_message = format!("Search: {}", query.text);
            self.active_search_filter = Some(query.text.clone());
            self.add_toast(Toast::info(format!("Search: {}", query.text)));
            // TODO: Filter self.items based on query.text
            // For now, just show the search in status
        }
    }

    /// Update command suggestions based on current omnibar input.
    fn update_command_suggestions(&mut self) {
        use command::get_command_suggestions;

        if self.omnibar_input.starts_with(':') {
            self.omnibar_suggestions = get_command_suggestions(&self.omnibar_input);
        } else {
            self.omnibar_suggestions.clear();
        }
    }
}

// ============================================================================
// TODO: Module stubs for future implementation
// ============================================================================

// mod daemon_client {
//     //! Client for communicating with scryforge-daemon
// }

// mod views {
//     //! Unified views (all feeds, all saved, etc.)
// }

// mod keybindings {
//     //! Configurable keyboard shortcuts
// }

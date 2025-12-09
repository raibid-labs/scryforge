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
use fusabi_streams_core::{Item, Stream};
use fusabi_tui_core::prelude::*;
use fusabi_tui_widgets::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout};
use tokio::sync::mpsc;

mod daemon_client;

use daemon_client::{Command, Message, get_daemon_url, spawn_client_task};
use scryforge_tui::command::{self, parse_command};
use scryforge_tui::search::{self, parse_search_query};

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
    let _ = cmd_tx.send(Command::FetchStreams);

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
    let _ = cmd_tx.send(Command::Shutdown);

    // Cleanup handled by TerminalWrapper::Drop
    Ok(())
}

// ============================================================================
// Application State
// ============================================================================

struct App {
    streams: Vec<Stream>,
    items: Vec<Item>,
    stream_state: ListState,
    item_state: ListState,
    focused: FocusedPane,
    omnibar_input: String,
    omnibar_active: bool,
    omnibar_mode: OmnibarMode,
    quit: bool,
    theme: Theme,
    status_message: String,
    cmd_tx: mpsc::UnboundedSender<Command>,
    daemon_connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OmnibarMode {
    Search,
    Command,
}

impl App {
    fn new(cmd_tx: mpsc::UnboundedSender<Command>) -> Self {
        Self {
            streams: Vec::new(),
            items: Vec::new(),
            stream_state: ListState::new(0),
            item_state: ListState::new(0),
            focused: FocusedPane::StreamList,
            omnibar_input: String::new(),
            omnibar_active: false,
            omnibar_mode: OmnibarMode::Search,
            quit: false,
            theme: Theme::default(),
            status_message: "Connecting to daemon...".to_string(),
            cmd_tx,
            daemon_connected: false,
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
                        let _ = self.cmd_tx.send(Command::FetchItems(stream.id.as_str().to_string()));
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
                }
                self.status_message = format!("Loaded {} items", count);
            }
            Message::SearchResults(items) => {
                let count = items.len();
                self.items = items;
                self.item_state = ListState::new(count);
                if count > 0 {
                    self.item_state.select_first();
                }
                self.status_message = format!("Found {} items", count);
            }
            Message::SyncTriggered => {
                self.status_message = "Sync triggered successfully".to_string();
            }
            Message::Error(err) => {
                self.status_message = format!("Error: {}", err);
            }
            Message::Disconnected => {
                self.status_message = "Disconnected from daemon".to_string();
                self.daemon_connected = false;
            }
        }
    }


    fn render(&self, frame: &mut ratatui::Frame) {
        let size = frame.area();

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
            .render(frame, main_chunks[1]);

        // Render status bar
        let connection_status = if self.daemon_connected {
            "Connected"
        } else {
            "Disconnected"
        };
        StatusBarWidget::new(&self.status_message, connection_status, &self.theme)
            .render(frame, main_chunks[2]);
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
                // Handle omnibar input when active
                if self.omnibar_active {
                    match key.code {
                        KeyCode::Esc => {
                            self.omnibar_active = false;
                            self.omnibar_input.clear();
                        }
                        KeyCode::Enter => {
                            self.execute_omnibar_input();
                            self.omnibar_active = false;
                            self.omnibar_input.clear();
                        }
                        KeyCode::Backspace => {
                            self.omnibar_input.pop();
                        }
                        KeyCode::Char(c) => {
                            self.omnibar_input.push(c);
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
                    KeyCode::Char('/') => {
                        self.omnibar_active = true;
                        self.omnibar_mode = OmnibarMode::Search;
                    }
                    KeyCode::Char(':') => {
                        self.omnibar_active = true;
                        self.omnibar_mode = OmnibarMode::Command;
                        self.omnibar_input.push(':');
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
                    KeyCode::Char('?') => {
                        self.status_message = "h/l:panes j/k:nav /:search q:quit".to_string();
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
            FocusedPane::ItemList => self.item_state.select_next(),
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
            FocusedPane::ItemList => self.item_state.select_prev(),
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
            FocusedPane::ItemList => self.item_state.select_first(),
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
            FocusedPane::ItemList => self.item_state.select_last(),
            _ => {}
        }
    }

    fn fetch_items_for_selected_stream(&mut self) {
        if let Some(idx) = self.stream_state.selected {
            if let Some(stream) = self.streams.get(idx) {
                let _ = self.cmd_tx.send(Command::FetchItems(stream.id.as_str().to_string()));
                self.status_message = format!("Loading items for {}...", stream.name);
            }
        }
    }

    fn execute_omnibar_input(&mut self) {
        match self.omnibar_mode {
            OmnibarMode::Search => {
                self.execute_search();
            }
            OmnibarMode::Command => {
                self.execute_command();
            }
        }
    }

    fn execute_search(&mut self) {
        let query = parse_search_query(&self.omnibar_input);

        // Build filters JSON
        let mut filters = serde_json::Map::new();

        if let Some(ref stream_id) = query.stream_filter {
            filters.insert("stream_id".to_string(), serde_json::Value::String(stream_id.clone()));
        }

        if let Some(ref content_type) = query.type_filter {
            let type_str = match content_type {
                search::ContentTypeFilter::Article => "Article",
                search::ContentTypeFilter::Email => "Email",
                search::ContentTypeFilter::Video => "Video",
                search::ContentTypeFilter::Track => "Track",
                search::ContentTypeFilter::Task => "Task",
                search::ContentTypeFilter::Event => "Event",
                search::ContentTypeFilter::Bookmark => "Bookmark",
            };
            filters.insert("content_type".to_string(), serde_json::Value::String(type_str.to_string()));
        }

        for status_filter in &query.status_filters {
            match status_filter {
                search::StatusFilter::Read => {
                    filters.insert("is_read".to_string(), serde_json::Value::Bool(true));
                }
                search::StatusFilter::Unread => {
                    filters.insert("is_read".to_string(), serde_json::Value::Bool(false));
                }
                search::StatusFilter::Saved => {
                    filters.insert("is_saved".to_string(), serde_json::Value::Bool(true));
                }
            }
        }

        let filters_value = if filters.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(filters))
        };

        let _ = self.cmd_tx.send(Command::Search {
            query: query.text.clone(),
            filters: filters_value,
        });

        self.status_message = format!("Searching for: {}", self.omnibar_input);
    }

    fn execute_command(&mut self) {
        match parse_command(&self.omnibar_input) {
            Ok(cmd) => {
                match cmd {
                    command::Command::Quit => {
                        self.quit = true;
                    }
                    command::Command::Sync { provider_id } => {
                        let msg = if let Some(ref id) = provider_id {
                            format!("Triggering sync for provider: {}", id)
                        } else {
                            "Triggering sync for all providers".to_string()
                        };
                        let _ = self.cmd_tx.send(Command::TriggerSync { provider_id });
                        self.status_message = msg;
                    }
                    command::Command::Refresh => {
                        // Refresh current view by re-fetching items
                        if let Some(idx) = self.stream_state.selected {
                            if let Some(stream) = self.streams.get(idx) {
                                let _ = self.cmd_tx.send(Command::FetchItems(stream.id.as_str().to_string()));
                                self.status_message = "Refreshing...".to_string();
                            }
                        } else {
                            // Refresh streams list
                            let _ = self.cmd_tx.send(Command::FetchStreams);
                            self.status_message = "Refreshing streams...".to_string();
                        }
                    }
                    command::Command::Help => {
                        self.status_message = "Commands: :quit :sync [provider] :refresh :help | Search: /query is:unread type:article in:stream".to_string();
                    }
                }
            }
            Err(e) => {
                self.status_message = format!("Command error: {}", e);
            }
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

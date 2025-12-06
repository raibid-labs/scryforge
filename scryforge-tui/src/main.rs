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
use fusabi_streams_core::{Item, ItemContent, ItemId, Stream, StreamId, StreamType};
use fusabi_tui_core::prelude::*;
use fusabi_tui_widgets::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout};
use std::collections::HashMap;

fn main() -> Result<()> {
    // Initialize logging (to file, not stdout, since we're using the terminal)
    // TODO: Set up file-based logging
    // For now, just suppress output

    // Initialize terminal
    let mut terminal = TerminalWrapper::new()?;

    // Create app state with dummy data for now
    let mut app = App::new_with_dummy_data();

    // Main event loop
    loop {
        // Render
        terminal.terminal().draw(|frame| {
            app.render(frame);
        })?;

        // Handle events
        if let Some(event) = poll_event(std::time::Duration::from_millis(100))? {
            if !app.handle_event(event) {
                break;
            }
        }

        if app.should_quit() {
            break;
        }
    }

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
    quit: bool,
    theme: Theme,
    status_message: String,
}

impl App {
    fn new_with_dummy_data() -> Self {
        // Create dummy streams for demonstration
        let streams = vec![
            Stream {
                id: StreamId::new("email", "inbox", "gmail"),
                name: "Gmail Inbox".to_string(),
                provider_id: "email-imap".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("📧".to_string()),
                unread_count: Some(5),
                total_count: Some(150),
                last_updated: None,
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("rss", "feed", "hackernews"),
                name: "Hacker News".to_string(),
                provider_id: "rss".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("📰".to_string()),
                unread_count: Some(42),
                total_count: Some(100),
                last_updated: None,
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("reddit", "home", "default"),
                name: "Reddit Home".to_string(),
                provider_id: "reddit".to_string(),
                stream_type: StreamType::Feed,
                icon: Some("🔴".to_string()),
                unread_count: None,
                total_count: None,
                last_updated: None,
                metadata: HashMap::new(),
            },
            Stream {
                id: StreamId::new("spotify", "collection", "liked"),
                name: "Liked Songs".to_string(),
                provider_id: "spotify".to_string(),
                stream_type: StreamType::SavedItems,
                icon: Some("💚".to_string()),
                unread_count: None,
                total_count: Some(523),
                last_updated: None,
                metadata: HashMap::new(),
            },
        ];

        // Create dummy items
        let items = vec![
            Item {
                id: ItemId::new("email", "msg-001"),
                stream_id: StreamId::new("email", "inbox", "gmail"),
                title: "Meeting tomorrow at 10am".to_string(),
                content: ItemContent::Email {
                    subject: "Meeting tomorrow at 10am".to_string(),
                    body_text: Some(
                        "Hi,\n\nJust a reminder about our meeting tomorrow.\n\nBest,\nJohn"
                            .to_string(),
                    ),
                    body_html: None,
                    snippet: "Just a reminder about our meeting...".to_string(),
                },
                author: Some(fusabi_streams_core::Author {
                    name: "John Doe".to_string(),
                    email: Some("john@example.com".to_string()),
                    url: None,
                    avatar_url: None,
                }),
                published: Some(chrono::Utc::now()),
                updated: None,
                url: None,
                thumbnail_url: None,
                is_read: false,
                is_saved: false,
                tags: vec![],
                metadata: HashMap::new(),
            },
            Item {
                id: ItemId::new("email", "msg-002"),
                stream_id: StreamId::new("email", "inbox", "gmail"),
                title: "Your order has shipped".to_string(),
                content: ItemContent::Email {
                    subject: "Your order has shipped".to_string(),
                    body_text: Some(
                        "Your order #12345 has shipped and will arrive by Friday.".to_string(),
                    ),
                    body_html: None,
                    snippet: "Your order #12345 has shipped...".to_string(),
                },
                author: Some(fusabi_streams_core::Author {
                    name: "Shop Support".to_string(),
                    email: Some("support@shop.com".to_string()),
                    url: None,
                    avatar_url: None,
                }),
                published: Some(chrono::Utc::now()),
                updated: None,
                url: None,
                thumbnail_url: None,
                is_read: true,
                is_saved: false,
                tags: vec![],
                metadata: HashMap::new(),
            },
            Item {
                id: ItemId::new("rss", "article-001"),
                stream_id: StreamId::new("rss", "feed", "hackernews"),
                title: "Show HN: A new Rust TUI framework".to_string(),
                content: ItemContent::Article {
                    summary: Some(
                        "I've been working on a new TUI framework in Rust...".to_string(),
                    ),
                    full_content: None,
                },
                author: Some(fusabi_streams_core::Author {
                    name: "rustdev".to_string(),
                    email: None,
                    url: None,
                    avatar_url: None,
                }),
                published: Some(chrono::Utc::now()),
                updated: None,
                url: Some("https://news.ycombinator.com/item?id=123".to_string()),
                thumbnail_url: None,
                is_read: false,
                is_saved: false,
                tags: vec![],
                metadata: HashMap::new(),
            },
        ];

        let stream_count = streams.len();
        let item_count = items.len();

        Self {
            streams,
            items,
            stream_state: ListState::new(stream_count),
            item_state: ListState::new(item_count),
            focused: FocusedPane::StreamList,
            omnibar_input: String::new(),
            omnibar_active: false,
            quit: false,
            theme: Theme::default(),
            status_message: "Ready - Press ? for help".to_string(),
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
        StatusBarWidget::new(&self.status_message, "Dummy provider", &self.theme)
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
                            // TODO: Execute search/command
                            self.status_message = format!("Search: {}", self.omnibar_input);
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
            FocusedPane::StreamList => self.stream_state.select_next(),
            FocusedPane::ItemList => self.item_state.select_next(),
            _ => {}
        }
    }

    fn navigate_up(&mut self) {
        match self.focused {
            FocusedPane::StreamList => self.stream_state.select_prev(),
            FocusedPane::ItemList => self.item_state.select_prev(),
            _ => {}
        }
    }

    fn navigate_first(&mut self) {
        match self.focused {
            FocusedPane::StreamList => self.stream_state.select_first(),
            FocusedPane::ItemList => self.item_state.select_first(),
            _ => {}
        }
    }

    fn navigate_last(&mut self) {
        match self.focused {
            FocusedPane::StreamList => self.stream_state.select_last(),
            FocusedPane::ItemList => self.item_state.select_last(),
            _ => {}
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

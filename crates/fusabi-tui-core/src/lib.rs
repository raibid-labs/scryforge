//! # fusabi-tui-core
//!
//! Core TUI infrastructure for Ratatui-based Fusabi applications.
//!
//! This crate provides the foundational building blocks for terminal user interfaces
//! in the Fusabi ecosystem:
//!
//! - Event loop abstraction
//! - State management primitives
//! - Input handling framework
//! - Async command dispatch
//!
//! ## Usage
//!
//! This crate is designed to be used alongside `fusabi-tui-widgets` for building
//! complete TUI applications. The typical pattern is:
//!
//! 1. Define your application state implementing [`AppState`]
//! 2. Create a [`Terminal`] wrapper
//! 3. Run the event loop with [`run_app`]

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

// ============================================================================
// Terminal Management
// ============================================================================

/// Wrapper around the terminal with setup/teardown.
pub struct TerminalWrapper {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalWrapper {
    /// Initialize the terminal for TUI rendering.
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Get a mutable reference to the underlying terminal.
    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Restore terminal to normal state.
    pub fn restore(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TerminalWrapper {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

// ============================================================================
// Input Events
// ============================================================================

/// Application-level input events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    /// A key was pressed
    Key(KeyEvent),
    /// Terminal was resized
    Resize(u16, u16),
    /// Tick for periodic updates
    Tick,
    /// Quit the application
    Quit,
}

/// Poll for input events with a timeout.
pub fn poll_event(timeout: Duration) -> Result<Option<AppEvent>> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => {
                // Handle Ctrl+C as quit
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(Some(AppEvent::Quit));
                }
                Ok(Some(AppEvent::Key(key)))
            }
            Event::Resize(w, h) => Ok(Some(AppEvent::Resize(w, h))),
            _ => Ok(None),
        }
    } else {
        Ok(Some(AppEvent::Tick))
    }
}

// ============================================================================
// Application State Trait
// ============================================================================

/// Trait for application state management.
///
/// Implement this trait for your application's main state struct.
/// The event loop will call these methods to update and render your app.
pub trait AppState {
    /// Handle an input event and return whether the app should continue running.
    fn handle_event(&mut self, event: AppEvent) -> bool;

    /// Check if the application should quit.
    fn should_quit(&self) -> bool;
}

// ============================================================================
// Focus and Navigation
// ============================================================================

/// Represents which pane/component has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPane {
    #[default]
    StreamList,
    ItemList,
    Preview,
    Omnibar,
}

impl FocusedPane {
    /// Move focus to the next pane (left to right).
    pub fn next(self) -> Self {
        match self {
            Self::StreamList => Self::ItemList,
            Self::ItemList => Self::Preview,
            Self::Preview => Self::StreamList,
            Self::Omnibar => Self::StreamList,
        }
    }

    /// Move focus to the previous pane (right to left).
    pub fn prev(self) -> Self {
        match self {
            Self::StreamList => Self::Preview,
            Self::ItemList => Self::StreamList,
            Self::Preview => Self::ItemList,
            Self::Omnibar => Self::StreamList,
        }
    }
}

// ============================================================================
// List State Helper
// ============================================================================

/// Simple list state for tracking selection in lists.
#[derive(Debug, Clone, Default)]
pub struct ListState {
    /// Currently selected index
    pub selected: Option<usize>,
    /// Total number of items
    pub len: usize,
    /// Scroll offset for virtual scrolling
    pub offset: usize,
}

impl ListState {
    pub fn new(len: usize) -> Self {
        Self {
            selected: if len > 0 { Some(0) } else { None },
            len,
            offset: 0,
        }
    }

    pub fn select_next(&mut self) {
        if self.len == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) if i + 1 < self.len => i + 1,
            Some(_) => 0, // Wrap to beginning
            None => 0,
        });
    }

    pub fn select_prev(&mut self) {
        if self.len == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(0) => self.len.saturating_sub(1), // Wrap to end
            Some(i) => i - 1,
            None => 0,
        });
    }

    pub fn select_first(&mut self) {
        if self.len > 0 {
            self.selected = Some(0);
        }
    }

    pub fn select_last(&mut self) {
        if self.len > 0 {
            self.selected = Some(self.len.saturating_sub(1));
        }
    }

    pub fn update_len(&mut self, len: usize) {
        self.len = len;
        if let Some(i) = self.selected {
            if i >= len {
                self.selected = if len > 0 { Some(len - 1) } else { None };
            }
        } else if len > 0 {
            self.selected = Some(0);
        }
    }
}

// ============================================================================
// Command/Message System (TODO)
// ============================================================================

// TODO: Implement async command dispatch for daemon communication
// This will allow the TUI to send commands to the daemon and receive responses
// without blocking the UI thread.
//
// pub enum Command {
//     FetchStreams,
//     FetchItems(StreamId),
//     ExecuteAction(ItemId, ActionId),
//     Search(String),
// }
//
// pub enum Message {
//     StreamsLoaded(Vec<Stream>),
//     ItemsLoaded(StreamId, Vec<Item>),
//     ActionCompleted(ActionResult),
//     SearchResults(Vec<Item>),
//     Error(String),
// }

// ============================================================================
// Re-exports
// ============================================================================

pub mod prelude {
    pub use crate::{poll_event, AppEvent, AppState, FocusedPane, ListState, TerminalWrapper};
}

//! # fusabi-tui-widgets
//!
//! Reusable TUI widgets for Fusabi applications built on Ratatui.
//!
//! This crate provides pre-built widgets commonly needed for information browser
//! applications like Scryforge:
//!
//! - [`StreamListWidget`] - Sidebar showing available streams
//! - [`ItemListWidget`] - Scrollable list of items with filtering
//! - [`PreviewWidget`] - Rich preview of selected item
//! - [`StatusBarWidget`] - Connection status, sync state, notifications
//! - [`OmnibarWidget`] - Command palette / quick search
//! - [`ToastWidget`] - Notification toasts for async operations
//!
//! ## Design Philosophy
//!
//! These widgets are designed to be:
//! - Composable: Mix and match to build your layout
//! - Themeable: Colors and styles can be customized
//! - Accessible: Keyboard-first navigation
//! - Responsive: Adapt to terminal size

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use scryforge_provider_core::{Item, Stream};
use std::time::{Duration, Instant};

// ============================================================================
// Theme / Styling
// ============================================================================
pub mod theme;
pub use theme::Theme;


// ============================================================================
// Provider Status
// ============================================================================

/// Status of a provider's sync state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSyncStatus {
    /// Provider is currently syncing
    Syncing,
    /// Provider is synced and up-to-date
    Synced,
    /// Provider encountered an error
    Error,
    /// Provider status is unknown or not connected
    Unknown,
}

impl ProviderSyncStatus {
    /// Get the display symbol for this status
    pub fn symbol(&self) -> &'static str {
        match self {
            ProviderSyncStatus::Syncing => "⟳",
            ProviderSyncStatus::Synced => "✓",
            ProviderSyncStatus::Error => "✗",
            ProviderSyncStatus::Unknown => "?",
        }
    }

    /// Get the color for this status
    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            ProviderSyncStatus::Syncing => theme.warning,
            ProviderSyncStatus::Synced => theme.success,
            ProviderSyncStatus::Error => theme.error,
            ProviderSyncStatus::Unknown => theme.muted,
        }
    }
}

/// Information about a provider for status display
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub sync_status: ProviderSyncStatus,
}

// ============================================================================
// Stream List Widget
// ============================================================================

/// Get provider icon/symbol based on provider name or type
fn get_provider_icon(provider_id: &str) -> &'static str {
    // Map provider IDs to unicode symbols
    match provider_id.to_lowercase().as_str() {
        id if id.contains("email") || id.contains("gmail") || id.contains("imap") => "📧",
        id if id.contains("rss") || id.contains("feed") => "📰",
        id if id.contains("spotify") => "🎵",
        id if id.contains("youtube") || id.contains("video") => "📹",
        id if id.contains("reddit") => "📱",
        id if id.contains("twitter") || id.contains("x") => "🐦",
        id if id.contains("github") => "🐙",
        id if id.contains("calendar") => "📅",
        id if id.contains("task") || id.contains("todo") => "✓",
        id if id.contains("bookmark") => "🔖",
        _ => "📄",
    }
}

/// Widget displaying a list of streams in a sidebar.
pub struct StreamListWidget<'a> {
    streams: &'a [Stream],
    selected: Option<usize>,
    focused: bool,
    theme: &'a Theme,
}

impl<'a> StreamListWidget<'a> {
    pub fn new(streams: &'a [Stream], selected: Option<usize>, theme: &'a Theme) -> Self {
        Self {
            streams,
            selected,
            focused: false,
            theme,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Render the widget to a frame at the given area.
    pub fn render(self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Streams ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = self
            .streams
            .iter()
            .enumerate()
            .map(|(i, stream)| {
                let is_selected = self.selected == Some(i);
                let unread = stream.unread_count.unwrap_or(0);

                let mut line = vec![];

                // Provider icon
                let icon = get_provider_icon(&stream.provider_id);
                line.push(Span::raw(format!("{} ", icon)));

                // Stream name
                line.push(Span::raw(&stream.name));

                // Unread count badge
                if unread > 0 {
                    line.push(Span::styled(
                        format!(" [{}]", unread),
                        Style::default().fg(self.theme.unread).add_modifier(Modifier::BOLD),
                    ));
                }

                let style = if is_selected {
                    Style::default()
                        .bg(self.theme.selection_bg)
                        .fg(self.theme.selection_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(line)).style(style)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}

// ============================================================================
// Item List Widget
// ============================================================================

/// Widget displaying a list of items.
pub struct ItemListWidget<'a> {
    items: &'a [Item],
    selected: Option<usize>,
    focused: bool,
    theme: &'a Theme,
}

impl<'a> ItemListWidget<'a> {
    pub fn new(items: &'a [Item], selected: Option<usize>, theme: &'a Theme) -> Self {
        Self {
            items,
            selected,
            focused: false,
            theme,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Items ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = self.selected == Some(i);

                let mut spans = vec![];

                // Read/unread indicator with distinct symbols
                if !item.is_read {
                    spans.push(Span::styled("● ", Style::default().fg(self.theme.unread)));
                } else {
                    spans.push(Span::styled("○ ", Style::default().fg(self.theme.muted)));
                }

                // Saved/starred indicator
                if item.is_saved {
                    spans.push(Span::styled("★ ", Style::default().fg(self.theme.accent)));
                }

                // Title - bold if unread
                let title_style = if !item.is_read {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(&item.title, title_style));

                // Author if present
                if let Some(ref author) = item.author {
                    spans.push(Span::styled(
                        format!(" - {}", author.name),
                        Style::default().fg(self.theme.muted),
                    ));
                }

                let style = if is_selected {
                    Style::default()
                        .bg(self.theme.selection_bg)
                        .fg(self.theme.selection_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(spans)).style(style)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}

// ============================================================================
// Preview Widget
// ============================================================================

/// Widget displaying a preview of the selected item.
pub struct PreviewWidget<'a> {
    item: Option<&'a Item>,
    focused: bool,
    theme: &'a Theme,
}

impl<'a> PreviewWidget<'a> {
    pub fn new(item: Option<&'a Item>, theme: &'a Theme) -> Self {
        Self {
            item,
            focused: false,
            theme,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Preview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let content = match self.item {
            Some(item) => {
                let mut lines = vec![];

                // Title line with status indicators
                let mut title_spans = vec![];
                if item.is_saved {
                    title_spans.push(Span::styled("★ ", Style::default().fg(self.theme.accent)));
                }
                title_spans.push(Span::styled(
                    &item.title,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(title_spans));
                lines.push(Line::from(""));

                if let Some(ref author) = item.author {
                    lines.push(Line::from(Span::styled(
                        format!("By: {}", author.name),
                        Style::default().fg(self.theme.muted),
                    )));
                }

                if let Some(published) = item.published {
                    lines.push(Line::from(Span::styled(
                        format!("Date: {}", published.format("%Y-%m-%d %H:%M")),
                        Style::default().fg(self.theme.muted),
                    )));
                }

                lines.push(Line::from(""));

                // Extract text content based on item type
                let body = extract_preview_text(&item.content);
                for line in body.lines() {
                    lines.push(Line::from(line.to_string()));
                }

                lines
            }
            None => vec![Line::from(Span::styled(
                "No item selected",
                Style::default().fg(self.theme.muted),
            ))],
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

fn extract_preview_text(content: &scryforge_provider_core::ItemContent) -> String {
    use scryforge_provider_core::ItemContent::*;
    match content {
        Text(s) => s.clone(),
        Markdown(s) => s.clone(), // TODO: render markdown
        Html(s) => s.clone(),     // TODO: strip HTML tags
        Email {
            snippet, body_text, ..
        } => body_text.clone().unwrap_or_else(|| snippet.clone()),
        Article {
            summary,
            full_content,
        } => full_content
            .clone()
            .or_else(|| summary.clone())
            .unwrap_or_default(),
        Video { description, .. } => description.clone(),
        Track { album, artists, .. } => {
            let artist_str = artists.join(", ");
            match album {
                Some(a) => format!("{artist_str} - {a}"),
                None => artist_str,
            }
        }
        Task { body, .. } => body.clone().unwrap_or_default(),
        Event {
            description,
            location,
            ..
        } => {
            let desc = description.clone().unwrap_or_default();
            match location {
                Some(loc) => format!("{desc}\n\nLocation: {loc}"),
                None => desc,
            }
        }
        Bookmark { description } => description.clone().unwrap_or_default(),
        Generic { body } => body.clone().unwrap_or_default(),
    }
}

// ============================================================================
// Enhanced Status Bar Widget
// ============================================================================

/// Widget displaying enhanced status information at the bottom.
pub struct StatusBarWidget<'a> {
    message: &'a str,
    connection_status: &'a str,
    provider_statuses: &'a [ProviderStatus],
    unread_count: u32,
    search_filter: Option<&'a str>,
    theme: &'a Theme,
}

impl<'a> StatusBarWidget<'a> {
    pub fn new(message: &'a str, connection_status: &'a str, theme: &'a Theme) -> Self {
        Self {
            message,
            connection_status,
            provider_statuses: &[],
            unread_count: 0,
            search_filter: None,
            theme,
        }
    }

    /// Set provider sync statuses
    pub fn provider_statuses(mut self, statuses: &'a [ProviderStatus]) -> Self {
        self.provider_statuses = statuses;
        self
    }

    /// Set total unread count
    pub fn unread_count(mut self, count: u32) -> Self {
        self.unread_count = count;
        self
    }

    /// Set current search/filter text
    pub fn search_filter(mut self, filter: Option<&'a str>) -> Self {
        self.search_filter = filter;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let mut spans = vec![];

        // Message
        spans.push(Span::styled(self.message, Style::default().fg(self.theme.foreground)));
        spans.push(Span::raw(" | "));

        // Connection status
        let conn_color = if self.connection_status.contains("Connect") {
            self.theme.success
        } else {
            self.theme.error
        };
        spans.push(Span::styled(self.connection_status, Style::default().fg(conn_color)));

        // Provider statuses
        if !self.provider_statuses.is_empty() {
            spans.push(Span::raw(" | "));
            for (i, status) in self.provider_statuses.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                let color = status.sync_status.color(self.theme);
                spans.push(Span::styled(
                    format!("{} {}", status.sync_status.symbol(), status.name),
                    Style::default().fg(color),
                ));
            }
        }

        // Unread count
        if self.unread_count > 0 {
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                format!("{} unread", self.unread_count),
                Style::default().fg(self.theme.unread).add_modifier(Modifier::BOLD),
            ));
        }

        // Search/filter indicator
        if let Some(filter) = self.search_filter {
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                format!("Filter: {}", filter),
                Style::default().fg(self.theme.accent),
            ));
        }

        let paragraph =
            Paragraph::new(Line::from(spans)).style(Style::default().bg(self.theme.selection_bg));

        frame.render_widget(paragraph, area);
    }
}

// ============================================================================
// Omnibar Widget
// ============================================================================

/// Widget for command palette / quick search.
///
/// Supports:
/// - Text input for search/commands
/// - Autocomplete suggestions
/// - Command hints
pub struct OmnibarWidget<'a> {
    input: &'a str,
    placeholder: &'a str,
    active: bool,
    suggestions: &'a [String],
    theme: &'a Theme,
}

impl<'a> OmnibarWidget<'a> {
    pub fn new(input: &'a str, theme: &'a Theme) -> Self {
        Self {
            input,
            placeholder: "Type to search or press : for commands...",
            active: false,
            suggestions: &[],
            theme,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn suggestions(mut self, suggestions: &'a [String]) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let border_color = if self.active {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        // Build content lines
        let mut lines = Vec::new();

        // Input line
        let display_text = if self.input.is_empty() {
            Span::styled(self.placeholder, Style::default().fg(self.theme.muted))
        } else {
            Span::raw(self.input)
        };
        lines.push(Line::from(display_text));

        // Show suggestions if available and omnibar is active
        if self.active && !self.suggestions.is_empty() {
            // Take first 3 suggestions to fit in the space
            for suggestion in self.suggestions.iter().take(3) {
                lines.push(Line::from(Span::styled(
                    format!("  {}", suggestion),
                    Style::default().fg(self.theme.muted),
                )));
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

// ============================================================================
// Toast Notification System
// ============================================================================

/// Type of toast notification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastType {
    fn color(&self, theme: &Theme) -> Color {
        match self {
            ToastType::Info => theme.accent,
            ToastType::Success => theme.success,
            ToastType::Warning => theme.warning,
            ToastType::Error => theme.error,
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            ToastType::Info => "ℹ",
            ToastType::Success => "✓",
            ToastType::Warning => "⚠",
            ToastType::Error => "✗",
        }
    }
}

/// A toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Toast {
    /// Create a new info toast
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            toast_type: ToastType::Info,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Create a new success toast
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            toast_type: ToastType::Success,
            created_at: Instant::now(),
            duration: Duration::from_secs(2),
        }
    }

    /// Create a new warning toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            toast_type: ToastType::Warning,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Create a new error toast
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            toast_type: ToastType::Error,
            created_at: Instant::now(),
            duration: Duration::from_secs(4),
        }
    }

    /// Check if this toast has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

/// Widget for displaying toast notifications
pub struct ToastWidget<'a> {
    toast: &'a Toast,
    theme: &'a Theme,
}

impl<'a> ToastWidget<'a> {
    pub fn new(toast: &'a Toast, theme: &'a Theme) -> Self {
        Self { toast, theme }
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let color = self.toast.toast_type.color(self.theme);
        let symbol = self.toast.toast_type.symbol();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.background));

        let text = format!("{} {}", symbol, self.toast.message);
        let paragraph = Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(color));

        frame.render_widget(paragraph, area);
    }
}

// ============================================================================
// Re-exports
// ============================================================================

pub mod prelude {
    pub use crate::{
        ItemListWidget, OmnibarWidget, PreviewWidget, ProviderStatus, ProviderSyncStatus,
        StatusBarWidget, StreamListWidget, Theme, Toast, ToastType, ToastWidget,
    };
}

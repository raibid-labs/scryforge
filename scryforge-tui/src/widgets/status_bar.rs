//! Status bar widget with provider sync status and unread counts.

use crate::theme::Theme;
use fusabi_tui_core::{buffer::Buffer, layout::Rect, style::Style};
use fusabi_tui_widgets::{paragraph::Paragraph, text::{Line, Span}, widget::Widget};

/// Status of a provider's sync state.
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
    pub fn color(&self, theme: &Theme) -> fusabi_tui_core::style::Color {
        match self {
            ProviderSyncStatus::Syncing => theme.warning,
            ProviderSyncStatus::Synced => theme.success,
            ProviderSyncStatus::Error => theme.error,
            ProviderSyncStatus::Unknown => theme.muted,
        }
    }
}

/// Information about a provider for status display.
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub sync_status: ProviderSyncStatus,
}

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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut spans = vec![];

        // Message
        spans.push(Span::styled(
            self.message,
            Style::new().fg(self.theme.foreground),
        ));
        spans.push(Span::raw(" | "));

        // Connection status
        let conn_color = if self.connection_status.contains("Connect") {
            self.theme.success
        } else {
            self.theme.error
        };
        spans.push(Span::styled(
            self.connection_status,
            Style::new().fg(conn_color),
        ));

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
                    Style::new().fg(color),
                ));
            }
        }

        // Unread count
        if self.unread_count > 0 {
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                format!("{} unread", self.unread_count),
                Style::new()
                    .fg(self.theme.unread)
                    .add_modifier(fusabi_tui_core::style::Modifier::BOLD),
            ));
        }

        // Search/filter indicator
        if let Some(filter) = self.search_filter {
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(
                format!("Filter: {}", filter),
                Style::new().fg(self.theme.accent),
            ));
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .style(Style::new().bg(self.theme.selection_bg));

        paragraph.render(area, buffer);
    }
}

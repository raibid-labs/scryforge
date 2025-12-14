//! Omnibar widget for command palette and search.

use crate::theme::Theme;
use fusabi_tui_core::{buffer::Buffer, layout::Rect, style::Style};
use fusabi_tui_widgets::{
    block::Block, borders::Borders, paragraph::Paragraph, text::{Line, Span}, widget::Widget,
};

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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let border_color = if self.active {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(border_color));

        // Build content lines
        let mut lines = Vec::new();

        // Input line
        let display_text = if self.input.is_empty() {
            Span::styled(self.placeholder, Style::new().fg(self.theme.muted))
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
                    Style::new().fg(self.theme.muted),
                )));
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buffer);
    }
}

//! Stream list widget for sidebar.

use crate::theme::Theme;
use fusabi_tui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use fusabi_tui_widgets::{
    block::Block, borders::Borders, list::{List, ListItem, ListState as WidgetListState}, text::{Line, Span},
};
use scryforge_provider_core::Stream;

/// Get provider icon/symbol based on provider name or type.
fn get_provider_icon(provider_id: &str) -> &'static str {
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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Streams ")
            .borders(Borders::ALL)
            .border_style(Style::new().fg(border_color));

        let items: Vec<ListItem> = self
            .streams
            .iter()
            .enumerate()
            .map(|(i, stream)| {
                let is_selected = self.selected == Some(i);
                let unread = stream.unread_count.unwrap_or(0);

                let mut spans = vec![];

                // Provider icon
                let icon = get_provider_icon(&stream.provider_id);
                spans.push(Span::raw(format!("{} ", icon)));

                // Stream name
                spans.push(Span::raw(&stream.name));

                // Unread count badge
                if unread > 0 {
                    spans.push(Span::styled(
                        format!(" [{}]", unread),
                        Style::new()
                            .fg(self.theme.unread)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                let style = if is_selected {
                    Style::new()
                        .bg(self.theme.selection_bg)
                        .fg(self.theme.selection_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };

                ListItem::new(Line::from(spans)).style(style)
            })
            .collect();

        let list = List::new(items).block(block);
        let mut list_state = WidgetListState::default();
        if let Some(selected) = self.selected {
            list_state.select(Some(selected));
        }
        fusabi_tui_widgets::StatefulWidget::render(&list, area, buffer, &mut list_state);
    }
}

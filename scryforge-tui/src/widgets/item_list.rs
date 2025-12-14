//! Item list widget with YouTube metadata formatting.

use crate::{theme::Theme, time};
use fusabi_tui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use fusabi_tui_widgets::{
    block::Block, borders::Borders, list::{List, ListItem, ListState as WidgetListState}, text::{Line, Span},
};
use scryforge_provider_core::Item;

/// Format view count in a compact, human-readable format.
/// Examples: 1.2K, 45K, 1.5M, 3.2B
fn format_view_count(count: u64) -> String {
    if count < 1_000 {
        format!("{} views", count)
    } else if count < 1_000_000 {
        let formatted = format!("{:.1}", count as f64 / 1_000.0);
        let trimmed = formatted.trim_end_matches(".0");
        format!("{}K views", trimmed)
    } else if count < 1_000_000_000 {
        let formatted = format!("{:.1}", count as f64 / 1_000_000.0);
        let trimmed = formatted.trim_end_matches(".0");
        format!("{}M views", trimmed)
    } else {
        let formatted = format!("{:.1}", count as f64 / 1_000_000_000.0);
        let trimmed = formatted.trim_end_matches(".0");
        format!("{}B views", trimmed)
    }
}

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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Items ")
            .borders(Borders::ALL)
            .border_style(Style::new().fg(border_color));

        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .flat_map(|(i, item)| {
                let is_selected = self.selected == Some(i);

                let mut lines = vec![];

                // First line: indicator + title + duration (for videos)
                let mut title_spans = vec![];

                // Read/unread indicator with distinct symbols
                if !item.is_read {
                    title_spans.push(Span::styled("● ", Style::new().fg(self.theme.unread)));
                } else {
                    title_spans.push(Span::styled("○ ", Style::new().fg(self.theme.muted)));
                }

                // Saved/starred indicator
                if item.is_saved {
                    title_spans.push(Span::styled("★ ", Style::new().fg(self.theme.accent)));
                }

                // Title - bold if unread
                let title_style = if !item.is_read {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };
                title_spans.push(Span::styled(&item.title, title_style));

                // Duration for video items (color-coded)
                if let scryforge_provider_core::ItemContent::Video {
                    duration_seconds: Some(duration),
                    ..
                } = &item.content
                {
                    let duration_str = time::format_duration((*duration) as u64);
                    let duration_color = time::duration_color((*duration) as u64);
                    title_spans.push(Span::raw("  "));
                    title_spans.push(Span::styled(
                        duration_str,
                        Style::new()
                            .fg(duration_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                let title_style = if is_selected {
                    Style::new()
                        .bg(self.theme.selection_bg)
                        .fg(self.theme.selection_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };

                lines.push(ListItem::new(Line::from(title_spans)).style(title_style));

                // Second line: metadata (author, views, published date)
                let mut metadata_spans = vec![];
                metadata_spans.push(Span::raw("  ")); // Indent for visual hierarchy

                // Author/Channel name
                if let Some(ref author) = item.author {
                    metadata_spans.push(Span::styled(
                        &author.name,
                        Style::new().fg(self.theme.muted),
                    ));
                }

                // View count for videos
                if let scryforge_provider_core::ItemContent::Video {
                    view_count: Some(views),
                    ..
                } = &item.content
                {
                    if !metadata_spans.is_empty() && metadata_spans.len() > 1 {
                        metadata_spans.push(Span::styled(" · ", Style::new().fg(self.theme.muted)));
                    }
                    metadata_spans.push(Span::styled(
                        format_view_count(*views),
                        Style::new().fg(self.theme.muted),
                    ));
                }

                // Published date (relative time)
                if let Some(published) = item.published {
                    if !metadata_spans.is_empty() && metadata_spans.len() > 1 {
                        metadata_spans.push(Span::styled(" · ", Style::new().fg(self.theme.muted)));
                    }
                    metadata_spans.push(Span::styled(
                        time::format_relative_time(published),
                        Style::new().fg(self.theme.muted),
                    ));
                }

                let metadata_style = if is_selected {
                    Style::new()
                        .bg(self.theme.selection_bg)
                        .fg(self.theme.muted)
                } else {
                    Style::new()
                };

                if metadata_spans.len() > 1 {
                    lines.push(ListItem::new(Line::from(metadata_spans)).style(metadata_style));
                }

                lines
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

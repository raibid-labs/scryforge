//! Preview widget for item detail display.

use crate::theme::Theme;
use fusabi_tui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use fusabi_tui_widgets::{
    block::Block, borders::Borders, paragraph::Paragraph, text::{Line, Span}, widget::Widget,
};
use scryforge_provider_core::Item;

fn extract_preview_text(content: &scryforge_provider_core::ItemContent) -> String {
    use scryforge_provider_core::ItemContent::*;
    match content {
        Text(s) => s.clone(),
        Markdown(s) => s.clone(),
        Html(s) => s.clone(),
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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let border_color = if self.focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Preview ")
            .borders(Borders::ALL)
            .border_style(Style::new().fg(border_color));

        let content = match self.item {
            Some(item) => {
                let mut lines = vec![];

                // Title line with status indicators
                let mut title_spans = vec![];
                if item.is_saved {
                    title_spans.push(Span::styled("★ ", Style::new().fg(self.theme.accent)));
                }
                title_spans.push(Span::styled(
                    &item.title,
                    Style::new().add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(title_spans));
                lines.push(Line::from(""));

                if let Some(ref author) = item.author {
                    lines.push(Line::from(Span::styled(
                        format!("By: {}", author.name),
                        Style::new().fg(self.theme.muted),
                    )));
                }

                if let Some(published) = item.published {
                    lines.push(Line::from(Span::styled(
                        format!("Date: {}", published.format("%Y-%m-%d %H:%M")),
                        Style::new().fg(self.theme.muted),
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
                Style::new().fg(self.theme.muted),
            ))],
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(fusabi_tui_widgets::Wrap::Wrap);

        paragraph.render(area, buffer);
    }
}

//! Playlist picker modal widget.
//!
//! A modal dialog for selecting a playlist to add items to.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use scryforge_provider_core::Collection;

use crate::Theme;

/// State for the playlist picker.
#[derive(Debug, Default)]
pub struct PlaylistPickerState {
    /// Currently selected index
    pub selected: Option<usize>,
    /// Total number of items
    len: usize,
    /// Whether the picker is visible
    pub visible: bool,
}

impl PlaylistPickerState {
    pub fn new(len: usize) -> Self {
        Self {
            selected: if len > 0 { Some(0) } else { None },
            len,
            visible: false,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn select_next(&mut self) {
        if self.len == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) => (i + 1) % self.len,
            None => 0,
        });
    }

    pub fn select_prev(&mut self) {
        if self.len == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(0) => self.len - 1,
            Some(i) => i - 1,
            None => 0,
        });
    }

    pub fn update_len(&mut self, len: usize) {
        self.len = len;
        if let Some(selected) = self.selected {
            if selected >= len {
                self.selected = if len > 0 { Some(len - 1) } else { None };
            }
        }
    }
}

/// Playlist picker modal widget.
pub struct PlaylistPicker<'a> {
    playlists: &'a [Collection],
    state: &'a PlaylistPickerState,
    theme: &'a Theme,
    title: &'a str,
}

impl<'a> PlaylistPicker<'a> {
    pub fn new(
        playlists: &'a [Collection],
        state: &'a PlaylistPickerState,
        theme: &'a Theme,
    ) -> Self {
        Self {
            playlists,
            state,
            theme,
            title: "Add to Playlist",
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Calculate centered modal area.
    fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        if !self.state.visible {
            return;
        }

        // Calculate modal area (40% width, 50% height, centered)
        let modal_area = Self::centered_rect(40, 50, area);

        // Clear the background
        frame.render_widget(Clear, modal_area);

        // Create the modal block
        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border_focused))
            .style(Style::default().bg(self.theme.background));

        // Create list items
        let items: Vec<ListItem> = self
            .playlists
            .iter()
            .enumerate()
            .map(|(i, playlist)| {
                let is_selected = self.state.selected == Some(i);
                let icon = if is_selected { "▶ " } else { "  " };
                let count = format!(" ({} items)", playlist.item_count);

                let style = if is_selected {
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.foreground)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(icon, style),
                    Span::styled(&playlist.name, style),
                    Span::styled(count, Style::default().fg(self.theme.muted)),
                ]))
            })
            .collect();

        // Add "Create New" option
        let mut all_items = items;
        let create_new_selected = self.state.selected == Some(self.playlists.len());
        let create_style = if create_new_selected {
            Style::default()
                .fg(self.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.muted)
        };
        all_items.push(ListItem::new(Line::from(vec![
            Span::styled("─".repeat(20), Style::default().fg(self.theme.border)),
        ])));
        all_items.push(ListItem::new(Line::from(vec![
            Span::styled(if create_new_selected { "▶ " } else { "  " }, create_style),
            Span::styled("+ Create New Playlist...", create_style),
        ])));

        let list = List::new(all_items).block(block);

        frame.render_widget(list, modal_area);

        // Render help text at bottom
        let help_area = Rect {
            x: modal_area.x,
            y: modal_area.y + modal_area.height,
            width: modal_area.width,
            height: 1,
        };
        if help_area.y < area.height {
            let help = Paragraph::new(Line::from(vec![
                Span::styled("j/k", Style::default().fg(self.theme.accent)),
                Span::raw(": navigate  "),
                Span::styled("Enter", Style::default().fg(self.theme.accent)),
                Span::raw(": select  "),
                Span::styled("Esc", Style::default().fg(self.theme.accent)),
                Span::raw(": cancel"),
            ]))
            .style(Style::default().fg(self.theme.muted));
            frame.render_widget(help, help_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::CollectionId;

    #[test]
    fn test_picker_state_navigation() {
        let mut state = PlaylistPickerState::new(3);
        assert_eq!(state.selected, Some(0));

        state.select_next();
        assert_eq!(state.selected, Some(1));

        state.select_next();
        assert_eq!(state.selected, Some(2));

        state.select_next();
        assert_eq!(state.selected, Some(0)); // Wraps around

        state.select_prev();
        assert_eq!(state.selected, Some(2)); // Wraps around backward
    }

    #[test]
    fn test_picker_visibility() {
        let mut state = PlaylistPickerState::new(2);
        assert!(!state.visible);

        state.show();
        assert!(state.visible);

        state.hide();
        assert!(!state.visible);
    }

    #[test]
    fn test_empty_picker() {
        let state = PlaylistPickerState::new(0);
        assert_eq!(state.selected, None);
    }
}

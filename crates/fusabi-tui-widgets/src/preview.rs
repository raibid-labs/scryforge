//! # PreviewPane Widget
//!
//! Advanced preview pane with scrolling support and content-type-aware rendering.
//!
//! This widget displays the full content of a selected item, with different
//! rendering strategies based on the content type:
//! - Email: subject, from, date, body
//! - Article: title, author, summary/content
//! - Video: title, description, duration
//! - Task: title, due date, body, completion status
//! - Markdown: basic markdown rendering (bold, italic, headers, lists)

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use scryforge_provider_core::{Item, ItemContent};

use crate::Theme;

/// State for tracking scroll position within the preview pane.
#[derive(Debug, Clone, Default)]
pub struct PreviewState {
    /// Vertical scroll offset (number of lines scrolled down)
    pub scroll_offset: u16,
}

impl PreviewState {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Scroll up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down by a page (roughly).
    pub fn page_down(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(page_size);
    }

    /// Scroll up by a page (roughly).
    pub fn page_up(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    /// Reset scroll to the top.
    pub fn reset(&mut self) {
        self.scroll_offset = 0;
    }
}

/// Widget for displaying a detailed preview of an item with scrolling support.
pub struct PreviewPane<'a> {
    item: Option<&'a Item>,
    state: &'a PreviewState,
    focused: bool,
    theme: &'a Theme,
}

impl<'a> PreviewPane<'a> {
    pub fn new(item: Option<&'a Item>, state: &'a PreviewState, theme: &'a Theme) -> Self {
        Self {
            item,
            state,
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

        let title = if self.focused {
            " Preview (scroll: j/k) "
        } else {
            " Preview "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let content = match self.item {
            Some(item) => render_item_content(item, self.theme),
            None => vec![Line::from(Span::styled(
                "No item selected",
                Style::default().fg(self.theme.muted),
            ))],
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((self.state.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }
}

/// Render item content based on its type, with appropriate formatting.
fn render_item_content<'a>(item: &'a Item, theme: &'a Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Title (always shown, bold)
    lines.push(Line::from(Span::styled(
        item.title.clone(),
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(theme.accent),
    )));
    lines.push(Line::from(""));

    // Render content based on type
    match &item.content {
        ItemContent::Email {
            subject,
            body_text,
            body_html: _,
            snippet,
        } => render_email_content(subject, body_text, snippet, item, theme, &mut lines),

        ItemContent::Article {
            summary,
            full_content,
        } => render_article_content(summary, full_content, item, theme, &mut lines),

        ItemContent::Video {
            description,
            duration_seconds,
            view_count,
        } => render_video_content(description, *duration_seconds, *view_count, item, theme, &mut lines),

        ItemContent::Task {
            body,
            due_date,
            is_completed,
        } => render_task_content(body, due_date.as_ref(), *is_completed, item, theme, &mut lines),

        ItemContent::Track {
            album,
            duration_ms,
            artists,
        } => render_track_content(album, *duration_ms, artists, item, theme, &mut lines),

        ItemContent::Event {
            description,
            start,
            end,
            location,
            is_all_day,
        } => render_event_content(
            description,
            start,
            end,
            location,
            *is_all_day,
            item,
            theme,
            &mut lines,
        ),

        ItemContent::Bookmark { description } => {
            render_bookmark_content(description, item, theme, &mut lines)
        }

        ItemContent::Markdown(text) => render_markdown_content(text, item, theme, &mut lines),

        ItemContent::Html(html) => render_html_content(html, item, theme, &mut lines),

        ItemContent::Text(text) => render_text_content(text, item, theme, &mut lines),

        ItemContent::Generic { body } => render_generic_content(body, item, theme, &mut lines),
    }

    lines
}

fn render_email_content<'a>(
    subject: &'a str,
    body_text: &'a Option<String>,
    snippet: &'a str,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Subject
    lines.push(Line::from(vec![
        Span::styled("Subject: ", Style::default().fg(theme.muted)),
        Span::raw(subject),
    ]));

    // Author (from)
    if let Some(ref author) = item.author {
        lines.push(Line::from(vec![
            Span::styled("From: ", Style::default().fg(theme.muted)),
            Span::raw(&author.name),
        ]));
        if let Some(ref email) = author.email {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(format!("<{}>", email), Style::default().fg(theme.muted)),
            ]));
        }
    }

    // Date
    if let Some(published) = item.published {
        lines.push(Line::from(vec![
            Span::styled("Date: ", Style::default().fg(theme.muted)),
            Span::raw(published.format("%Y-%m-%d %H:%M").to_string()),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(""));

    // Body
    let body = body_text.as_deref().unwrap_or(snippet);
    for line in body.lines() {
        lines.push(Line::from(line.to_string()));
    }
}

fn render_article_content<'a>(
    summary: &'a Option<String>,
    full_content: &'a Option<String>,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Author
    if let Some(ref author) = item.author {
        lines.push(Line::from(vec![
            Span::styled("By: ", Style::default().fg(theme.muted)),
            Span::raw(&author.name),
        ]));
    }

    // Publication date
    if let Some(published) = item.published {
        lines.push(Line::from(vec![
            Span::styled("Published: ", Style::default().fg(theme.muted)),
            Span::raw(published.format("%Y-%m-%d").to_string()),
        ]));
    }

    // URL
    if let Some(ref url) = item.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(theme.muted)),
            Span::styled(url, Style::default().fg(theme.accent)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(""));

    // Content
    let content = full_content
        .as_ref()
        .or(summary.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("No content available");

    for line in content.lines() {
        lines.push(Line::from(line.to_string()));
    }
}

fn render_video_content<'a>(
    description: &'a str,
    duration_seconds: Option<u32>,
    view_count: Option<u64>,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Author/Channel
    if let Some(ref author) = item.author {
        lines.push(Line::from(vec![
            Span::styled("Channel: ", Style::default().fg(theme.muted)),
            Span::raw(&author.name),
        ]));
    }

    // Published date with relative time
    if let Some(published) = item.published {
        use crate::time::format_relative_time;
        lines.push(Line::from(vec![
            Span::styled("Published: ", Style::default().fg(theme.muted)),
            Span::raw(format_relative_time(published)),
        ]));
    }

    // Duration with color coding
    if let Some(seconds) = duration_seconds {
        use crate::time::duration_color;
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        let duration_str = if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, secs)
        } else {
            format!("{}:{:02}", minutes, secs)
        };
        let color = duration_color(seconds);
        lines.push(Line::from(vec![
            Span::styled("Duration: ", Style::default().fg(theme.muted)),
            Span::styled(duration_str, Style::default().fg(color)),
        ]));
    }

    // View count
    if let Some(views) = view_count {
        lines.push(Line::from(vec![
            Span::styled("Views: ", Style::default().fg(theme.muted)),
            Span::raw(format_number(views)),
        ]));
    }

    // Like count from metadata
    if let Some(like_count_str) = item.metadata.get("like_count") {
        if let Ok(likes) = like_count_str.parse::<u64>() {
            lines.push(Line::from(vec![
                Span::styled("Likes: ", Style::default().fg(theme.muted)),
                Span::raw(format_number(likes)),
            ]));
        }
    }

    // Comment count from metadata
    if let Some(comment_count_str) = item.metadata.get("comment_count") {
        if let Ok(comments) = comment_count_str.parse::<u64>() {
            lines.push(Line::from(vec![
                Span::styled("Comments: ", Style::default().fg(theme.muted)),
                Span::raw(format_number(comments)),
            ]));
        }
    }

    // URL
    if let Some(ref url) = item.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(theme.muted)),
            Span::styled(url, Style::default().fg(theme.accent)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(""));

    // Description
    for line in description.lines() {
        lines.push(Line::from(line.to_string()));
    }
}

fn render_task_content<'a>(
    body: &'a Option<String>,
    due_date: Option<&'a chrono::NaiveDate>,
    is_completed: bool,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Status
    let status = if is_completed {
        Span::styled("✓ Completed", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ Incomplete", Style::default().fg(theme.unread))
    };
    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(theme.muted)),
        status,
    ]));

    // Due date
    if let Some(date) = due_date {
        lines.push(Line::from(vec![
            Span::styled("Due: ", Style::default().fg(theme.muted)),
            Span::raw(date.format("%Y-%m-%d").to_string()),
        ]));
    }

    // Created date
    if let Some(created) = item.published {
        lines.push(Line::from(vec![
            Span::styled("Created: ", Style::default().fg(theme.muted)),
            Span::raw(created.format("%Y-%m-%d").to_string()),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(""));

    // Body
    if let Some(ref text) = body {
        for line in text.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "No description",
            Style::default().fg(theme.muted),
        )));
    }
}

fn render_track_content<'a>(
    album: &'a Option<String>,
    duration_ms: Option<u32>,
    artists: &'a [String],
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Artists
    if !artists.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Artists: ", Style::default().fg(theme.muted)),
            Span::raw(artists.join(", ")),
        ]));
    }

    // Album
    if let Some(ref album_name) = album {
        lines.push(Line::from(vec![
            Span::styled("Album: ", Style::default().fg(theme.muted)),
            Span::raw(album_name),
        ]));
    }

    // Duration
    if let Some(ms) = duration_ms {
        let seconds = ms / 1000;
        let minutes = seconds / 60;
        let secs = seconds % 60;
        lines.push(Line::from(vec![
            Span::styled("Duration: ", Style::default().fg(theme.muted)),
            Span::raw(format!("{}:{:02}", minutes, secs)),
        ]));
    }

    // URL
    if let Some(ref url) = item.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(theme.muted)),
            Span::styled(url, Style::default().fg(theme.accent)),
        ]));
    }
}

fn render_event_content<'a>(
    description: &'a Option<String>,
    start: &'a chrono::DateTime<chrono::Utc>,
    end: &'a chrono::DateTime<chrono::Utc>,
    location: &'a Option<String>,
    is_all_day: bool,
    _item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // Time
    if is_all_day {
        lines.push(Line::from(vec![
            Span::styled("When: ", Style::default().fg(theme.muted)),
            Span::raw(start.format("%Y-%m-%d").to_string()),
            Span::raw(" (All day)"),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Start: ", Style::default().fg(theme.muted)),
            Span::raw(start.format("%Y-%m-%d %H:%M").to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("End: ", Style::default().fg(theme.muted)),
            Span::raw(end.format("%Y-%m-%d %H:%M").to_string()),
        ]));
    }

    // Location
    if let Some(ref loc) = location {
        lines.push(Line::from(vec![
            Span::styled("Location: ", Style::default().fg(theme.muted)),
            Span::raw(loc),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(""));

    // Description
    if let Some(ref desc) = description {
        for line in desc.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }
}

fn render_bookmark_content<'a>(
    description: &'a Option<String>,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    // URL
    if let Some(ref url) = item.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(theme.muted)),
            Span::styled(url, Style::default().fg(theme.accent)),
        ]));
    }

    // Created date
    if let Some(created) = item.published {
        lines.push(Line::from(vec![
            Span::styled("Saved: ", Style::default().fg(theme.muted)),
            Span::raw(created.format("%Y-%m-%d").to_string()),
        ]));
    }

    // Tags
    if !item.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tags: ", Style::default().fg(theme.muted)),
            Span::raw(item.tags.join(", ")),
        ]));
    }

    lines.push(Line::from(""));

    // Description
    if let Some(ref desc) = description {
        for line in desc.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }
}

fn render_markdown_content<'a>(
    text: &'a str,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    render_metadata(item, theme, lines);
    lines.push(Line::from(""));

    // Basic markdown rendering
    for line in text.lines() {
        let rendered_line = render_markdown_line(line, theme);
        lines.push(rendered_line);
    }
}

fn render_html_content<'a>(
    html: &'a str,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    render_metadata(item, theme, lines);
    lines.push(Line::from(""));

    // For now, just strip HTML tags and render as text
    // TODO: Proper HTML to text conversion
    let text = strip_html_tags(html);
    for line in text.lines() {
        lines.push(Line::from(line.to_string()));
    }
}

fn render_text_content<'a>(
    text: &'a str,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    render_metadata(item, theme, lines);
    lines.push(Line::from(""));

    for line in text.lines() {
        lines.push(Line::from(line.to_string()));
    }
}

fn render_generic_content<'a>(
    body: &'a Option<String>,
    item: &'a Item,
    theme: &'a Theme,
    lines: &mut Vec<Line<'a>>,
) {
    render_metadata(item, theme, lines);
    lines.push(Line::from(""));

    if let Some(ref text) = body {
        for line in text.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "No content available",
            Style::default().fg(theme.muted),
        )));
    }
}

fn render_metadata<'a>(item: &'a Item, theme: &'a Theme, lines: &mut Vec<Line<'a>>) {
    if let Some(ref author) = item.author {
        lines.push(Line::from(vec![
            Span::styled("Author: ", Style::default().fg(theme.muted)),
            Span::raw(&author.name),
        ]));
    }

    if let Some(published) = item.published {
        lines.push(Line::from(vec![
            Span::styled("Published: ", Style::default().fg(theme.muted)),
            Span::raw(published.format("%Y-%m-%d %H:%M").to_string()),
        ]));
    }

    if let Some(ref url) = item.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(theme.muted)),
            Span::styled(url, Style::default().fg(theme.accent)),
        ]));
    }
}

/// Basic markdown line rendering with support for bold, italic, headers, and lists.
fn render_markdown_line<'a>(line: &'a str, theme: &'a Theme) -> Line<'a> {
    let trimmed = line.trim_start();

    // Headers
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        ));
    } else if let Some(rest) = trimmed.strip_prefix("## ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    } else if let Some(rest) = trimmed.strip_prefix("### ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    }

    // Lists
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent = line.len() - trimmed.len();
        return Line::from(format!("{}• {}", " ".repeat(indent), &trimmed[2..]));
    }

    // Numbered lists
    if let Some(idx) = trimmed.find(". ") {
        if trimmed[..idx].chars().all(|c| c.is_ascii_digit()) {
            return Line::from(line.to_string());
        }
    }

    // Parse inline styles (basic implementation)
    let spans = parse_inline_markdown(line, theme);
    Line::from(spans)
}

/// Parse inline markdown styles like **bold** and *italic*.
fn parse_inline_markdown<'a>(text: &'a str, theme: &'a Theme) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '*' {
            if chars.peek() == Some(&'*') {
                // Bold: **text**
                chars.next(); // consume second *
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }

                // Collect until closing **
                let mut bold_text = String::new();
                let mut found_closing = false;
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        found_closing = true;
                        break;
                    }
                    bold_text.push(c);
                }

                if found_closing {
                    spans.push(Span::styled(
                        bold_text,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // No closing **, treat as literal
                    current.push_str("**");
                    current.push_str(&bold_text);
                }
            } else {
                // Italic: *text*
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }

                // Collect until closing *
                let mut italic_text = String::new();
                let mut found_closing = false;
                while let Some(c) = chars.next() {
                    if c == '*' {
                        found_closing = true;
                        break;
                    }
                    italic_text.push(c);
                }

                if found_closing {
                    spans.push(Span::styled(
                        italic_text,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                } else {
                    // No closing *, treat as literal
                    current.push('*');
                    current.push_str(&italic_text);
                }
            }
        } else if ch == '`' {
            // Code: `text`
            if !current.is_empty() {
                spans.push(Span::raw(current.clone()));
                current.clear();
            }

            let mut code_text = String::new();
            let mut found_closing = false;
            while let Some(c) = chars.next() {
                if c == '`' {
                    found_closing = true;
                    break;
                }
                code_text.push(c);
            }

            if found_closing {
                spans.push(Span::styled(
                    code_text,
                    Style::default().fg(theme.accent),
                ));
            } else {
                current.push('`');
                current.push_str(&code_text);
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

/// Strip HTML tags from text (simple implementation).
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(ch);
                }
            }
        }
    }

    result
}

/// Format large numbers with commas.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for ch in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
        count += 1;
    }

    result.chars().rev().collect()
}

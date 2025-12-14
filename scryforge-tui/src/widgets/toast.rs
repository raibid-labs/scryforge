//! Toast notification widget.

use crate::theme::Theme;
use fusabi_tui_core::{buffer::Buffer, layout::Rect, style::{Modifier, Style}};
use fusabi_tui_widgets::{
    block::Block, borders::Borders, paragraph::Paragraph, widget::Widget,
};
use std::time::{Duration, Instant};

/// Type of toast notification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastType {
    fn color(&self, theme: &Theme) -> fusabi_tui_core::style::Color {
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

    pub fn render(self, area: Rect, buffer: &mut Buffer) {
        let color = self.toast.toast_type.color(self.theme);
        let symbol = self.toast.toast_type.symbol();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(color).add_modifier(Modifier::BOLD))
            .style(Style::new().bg(self.theme.background));

        let text = format!("{} {}", symbol, self.toast.message);
        let paragraph = Paragraph::new(text)
            .block(block)
            .style(Style::new().fg(color));

        paragraph.render(area, buffer);
    }
}

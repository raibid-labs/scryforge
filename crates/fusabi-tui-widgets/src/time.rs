//! Time formatting utilities for the TUI.

use chrono::{DateTime, Utc};

/// Format a datetime as relative time (e.g., "3 days ago").
pub fn format_relative_time(datetime: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(datetime);

    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();
    let weeks = duration.num_weeks();

    if minutes < 1 {
        "just now".to_string()
    } else if minutes < 60 {
        format!("{} min ago", minutes)
    } else if hours < 24 {
        if hours == 1 { "1 hour ago".to_string() } else { format!("{} hours ago", hours) }
    } else if days < 7 {
        if days == 1 { "1 day ago".to_string() } else { format!("{} days ago", days) }
    } else if weeks < 4 {
        if weeks == 1 { "1 week ago".to_string() } else { format!("{} weeks ago", weeks) }
    } else {
        datetime.format("%Y-%m-%d").to_string()
    }
}

/// Get color for video duration based on length.
pub fn duration_color(seconds: u32) -> ratatui::style::Color {
    use ratatui::style::Color;
    match seconds {
        0..=299 => Color::Green,           // < 5 min (short)
        300..=1199 => Color::Yellow,       // 5-20 min (standard)
        1200..=3599 => Color::Rgb(255, 165, 0), // 20-60 min (long, orange)
        _ => Color::Red,                   // > 60 min (extended)
    }
}

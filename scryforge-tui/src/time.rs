//! Time formatting utilities for YouTube video metadata.

use chrono::{DateTime, Utc};
use fusabi_tui_core::style::Color;

/// Format a duration in seconds to a human-readable string (HH:MM:SS or MM:SS).
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

/// Get color for duration based on length (green for short, red for long).
pub fn duration_color(seconds: u64) -> Color {
    match seconds {
        0..=300 => Color::Green,        // 0-5 min: green
        301..=1200 => Color::Yellow,    // 5-20 min: yellow
        1201..=3600 => Color::LightRed, // 20-60 min: orange-ish
        _ => Color::Red,                // 60+ min: red
    }
}

/// Format a timestamp as relative time (e.g., "3 days ago").
pub fn format_relative_time(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if duration.num_days() < 30 {
        let days = duration.num_days();
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    } else {
        let years = duration.num_days() / 365;
        format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
    }
}

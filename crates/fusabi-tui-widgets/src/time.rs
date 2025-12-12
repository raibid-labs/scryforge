//! # Time Utilities
//!
//! Utilities for formatting timestamps and durations in a human-readable way.
//! Used by TUI widgets to display relative times and color-code video durations.

use chrono::{DateTime, Utc};
use ratatui::style::Color;

/// Format a timestamp as relative time (e.g., "3 days ago", "just now").
///
/// # Arguments
///
/// * `datetime` - The datetime to format
///
/// # Returns
///
/// A human-readable relative time string
///
/// # Examples
///
/// ```
/// use chrono::{Utc, Duration};
/// use fusabi_tui_widgets::time::format_relative_time;
///
/// let now = Utc::now();
/// let three_days_ago = now - Duration::days(3);
/// assert_eq!(format_relative_time(three_days_ago), "3 days ago");
/// ```
pub fn format_relative_time(datetime: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(datetime);

    if duration.num_minutes() < 1 {
        "just now".to_string()
    } else if duration.num_hours() < 1 {
        let minutes = duration.num_minutes();
        if minutes == 1 {
            "1 min ago".to_string()
        } else {
            format!("{} mins ago", minutes)
        }
    } else if duration.num_days() < 1 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else if duration.num_weeks() < 1 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{} weeks ago", weeks)
        }
    } else {
        // For older dates, show absolute date
        datetime.format("%Y-%m-%d").to_string()
    }
}

/// Format duration in seconds to a human-readable string (e.g., "5:30", "1:23:45").
///
/// # Arguments
///
/// * `seconds` - Total duration in seconds
///
/// # Returns
///
/// A formatted duration string
///
/// # Examples
///
/// ```
/// use fusabi_tui_widgets::time::format_duration;
///
/// assert_eq!(format_duration(330), "5:30");
/// assert_eq!(format_duration(3665), "1:01:05");
/// assert_eq!(format_duration(45), "0:45");
/// ```
pub fn format_duration(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}

/// Get the color for a video duration based on length categories.
///
/// Color coding:
/// - Green (< 5 min) - Short clips
/// - Yellow (5-20 min) - Standard videos
/// - Orange (20-60 min) - Long-form content
/// - Red (> 60 min) - Extended content
///
/// # Arguments
///
/// * `duration_seconds` - Video duration in seconds
///
/// # Returns
///
/// A ratatui Color for the duration
///
/// # Examples
///
/// ```
/// use fusabi_tui_widgets::time::duration_color;
/// use ratatui::style::Color;
///
/// assert_eq!(duration_color(180), Color::Green);      // 3 minutes
/// assert_eq!(duration_color(600), Color::Yellow);     // 10 minutes
/// assert_eq!(duration_color(1800), Color::Rgb(255, 165, 0)); // 30 minutes (orange)
/// assert_eq!(duration_color(4000), Color::Red);       // 66 minutes
/// ```
pub fn duration_color(duration_seconds: u32) -> Color {
    let minutes = duration_seconds / 60;

    if minutes < 5 {
        Color::Green
    } else if minutes < 20 {
        Color::Yellow
    } else if minutes < 60 {
        // Orange - RGB(255, 165, 0)
        Color::Rgb(255, 165, 0)
    } else {
        Color::Red
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_format_relative_time_just_now() {
        let now = Utc::now();
        assert_eq!(format_relative_time(now), "just now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = Utc::now();
        let five_mins_ago = now - Duration::minutes(5);
        assert_eq!(format_relative_time(five_mins_ago), "5 mins ago");

        let one_min_ago = now - Duration::minutes(1);
        assert_eq!(format_relative_time(one_min_ago), "1 min ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let now = Utc::now();
        let two_hours_ago = now - Duration::hours(2);
        assert_eq!(format_relative_time(two_hours_ago), "2 hours ago");

        let one_hour_ago = now - Duration::hours(1);
        assert_eq!(format_relative_time(one_hour_ago), "1 hour ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let now = Utc::now();
        let three_days_ago = now - Duration::days(3);
        assert_eq!(format_relative_time(three_days_ago), "3 days ago");

        let one_day_ago = now - Duration::days(1);
        assert_eq!(format_relative_time(one_day_ago), "1 day ago");
    }

    #[test]
    fn test_format_relative_time_weeks() {
        let now = Utc::now();
        let two_weeks_ago = now - Duration::weeks(2);
        assert_eq!(format_relative_time(two_weeks_ago), "2 weeks ago");

        let one_week_ago = now - Duration::weeks(1);
        assert_eq!(format_relative_time(one_week_ago), "1 week ago");
    }

    #[test]
    fn test_format_relative_time_old_dates() {
        let now = Utc::now();
        let old_date = now - Duration::weeks(8);
        let result = format_relative_time(old_date);
        // Should be in format YYYY-MM-DD
        assert!(result.contains('-'));
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn test_format_duration_seconds_only() {
        assert_eq!(format_duration(45), "0:45");
        assert_eq!(format_duration(5), "0:05");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(330), "5:30");
        assert_eq!(format_duration(600), "10:00");
        assert_eq!(format_duration(61), "1:01");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3665), "1:01:05");
        assert_eq!(format_duration(7200), "2:00:00");
        assert_eq!(format_duration(5415), "1:30:15");
    }

    #[test]
    fn test_duration_color_short() {
        assert_eq!(duration_color(60), Color::Green); // 1 minute
        assert_eq!(duration_color(180), Color::Green); // 3 minutes
        assert_eq!(duration_color(299), Color::Green); // 4:59
    }

    #[test]
    fn test_duration_color_standard() {
        assert_eq!(duration_color(300), Color::Yellow); // 5 minutes
        assert_eq!(duration_color(600), Color::Yellow); // 10 minutes
        assert_eq!(duration_color(1199), Color::Yellow); // 19:59
    }

    #[test]
    fn test_duration_color_long() {
        assert_eq!(duration_color(1200), Color::Rgb(255, 165, 0)); // 20 minutes
        assert_eq!(duration_color(1800), Color::Rgb(255, 165, 0)); // 30 minutes
        assert_eq!(duration_color(3599), Color::Rgb(255, 165, 0)); // 59:59
    }

    #[test]
    fn test_duration_color_extended() {
        assert_eq!(duration_color(3600), Color::Red); // 1 hour
        assert_eq!(duration_color(7200), Color::Red); // 2 hours
        assert_eq!(duration_color(10800), Color::Red); // 3 hours
    }
}

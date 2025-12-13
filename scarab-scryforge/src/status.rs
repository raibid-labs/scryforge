//! Status bar rendering for the Scryforge plugin.

use chrono::{DateTime, Utc};
use scarab_plugin_api::status_bar::{Color, RenderItem};

/// Build status bar items showing unread count.
///
/// # Arguments
///
/// * `unread_count` - Number of unread items
/// * `last_sync` - Optional timestamp of last sync
/// * `is_healthy` - Whether the daemon connection is healthy
///
/// # Returns
///
/// A vector of `RenderItem`s to display in the status bar.
pub fn build_status_items(
    unread_count: usize,
    last_sync: Option<DateTime<Utc>>,
    is_healthy: bool,
) -> Vec<RenderItem> {
    let mut items = Vec::new();

    // Icon and color based on health status
    if is_healthy {
        // Healthy green color (catppuccin mocha green)
        items.push(RenderItem::Foreground(Color::Hex("#a6e3a1".to_string())));
    } else {
        // Warning yellow for unhealthy
        items.push(RenderItem::Foreground(Color::Hex("#f9e2af".to_string())));
    }

    // Mailbox emoji
    items.push(RenderItem::Text("📬".to_string()));
    items.push(RenderItem::Text(" ".to_string()));

    // Reset color for count
    items.push(RenderItem::ResetForeground);

    // Unread count with appropriate styling
    if unread_count > 0 {
        items.push(RenderItem::Bold);
        items.push(RenderItem::Foreground(Color::Hex("#89b4fa".to_string()))); // catppuccin blue
        items.push(RenderItem::Text(format!("{}", unread_count)));
        items.push(RenderItem::ResetAttributes);
        items.push(RenderItem::Text(" unread".to_string()));
    } else {
        items.push(RenderItem::Text("0 unread".to_string()));
    }

    // Add sync time if available
    if let Some(sync_time) = last_sync {
        items.push(RenderItem::Separator(" | ".to_string()));

        // Format relative time
        let now = Utc::now();
        let duration = now.signed_duration_since(sync_time);

        let time_str = if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        };

        items.push(RenderItem::Foreground(Color::Hex("#6c7086".to_string()))); // catppuccin overlay0
        items.push(RenderItem::Text(time_str));
        items.push(RenderItem::ResetForeground);
    }

    // Add warning indicator if unhealthy
    if !is_healthy {
        items.push(RenderItem::Separator(" ".to_string()));
        items.push(RenderItem::Foreground(Color::Hex("#f38ba8".to_string()))); // catppuccin red
        items.push(RenderItem::Text("⚠".to_string()));
        items.push(RenderItem::ResetForeground);
    }

    items
}

/// Build compact status bar items (just icon and count).
pub fn build_compact_status(unread_count: usize, is_healthy: bool) -> Vec<RenderItem> {
    let mut items = Vec::new();

    // Color based on health
    if is_healthy {
        items.push(RenderItem::Foreground(Color::Hex("#a6e3a1".to_string())));
    } else {
        items.push(RenderItem::Foreground(Color::Hex("#f9e2af".to_string())));
    }

    items.push(RenderItem::Text("📬".to_string()));
    items.push(RenderItem::ResetForeground);

    if unread_count > 0 {
        items.push(RenderItem::Text(" ".to_string()));
        items.push(RenderItem::Bold);
        items.push(RenderItem::Foreground(Color::Hex("#89b4fa".to_string())));
        items.push(RenderItem::Text(format!("{}", unread_count)));
        items.push(RenderItem::ResetAttributes);
    }

    items
}

/// Build detailed status bar items with provider breakdown.
pub fn build_detailed_status(
    unread_count: usize,
    provider_counts: &[(String, usize)],
    is_healthy: bool,
) -> Vec<RenderItem> {
    let mut items = Vec::new();

    // Start with basic status
    if is_healthy {
        items.push(RenderItem::Foreground(Color::Hex("#a6e3a1".to_string())));
    } else {
        items.push(RenderItem::Foreground(Color::Hex("#f9e2af".to_string())));
    }

    items.push(RenderItem::Text("📬".to_string()));
    items.push(RenderItem::Text(" ".to_string()));
    items.push(RenderItem::ResetForeground);

    // Total count
    if unread_count > 0 {
        items.push(RenderItem::Bold);
        items.push(RenderItem::Foreground(Color::Hex("#89b4fa".to_string())));
        items.push(RenderItem::Text(format!("{}", unread_count)));
        items.push(RenderItem::ResetAttributes);
    } else {
        items.push(RenderItem::Text("0".to_string()));
    }

    // Provider breakdown (show up to 3)
    if !provider_counts.is_empty() {
        items.push(RenderItem::Separator(" (".to_string()));

        for (i, (provider, count)) in provider_counts.iter().take(3).enumerate() {
            if i > 0 {
                items.push(RenderItem::Text(", ".to_string()));
            }

            items.push(RenderItem::Text(format!("{}: ", provider)));
            items.push(RenderItem::Foreground(Color::Hex("#cba6f7".to_string()))); // catppuccin mauve
            items.push(RenderItem::Text(format!("{}", count)));
            items.push(RenderItem::ResetForeground);
        }

        if provider_counts.len() > 3 {
            items.push(RenderItem::Text(format!(", +{} more", provider_counts.len() - 3)));
        }

        items.push(RenderItem::Text(")".to_string()));
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_build_status_items_no_unread() {
        let items = build_status_items(0, None, true);
        assert!(!items.is_empty());

        // Should contain "0 unread"
        let has_zero = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("0 unread"))
        });
        assert!(has_zero);
    }

    #[test]
    fn test_build_status_items_with_unread() {
        let items = build_status_items(5, None, true);

        // Should contain the count
        let has_count = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("5"))
        });
        assert!(has_count);

        // Should have bold formatting
        let has_bold = items.iter().any(|item| matches!(item, RenderItem::Bold));
        assert!(has_bold);
    }

    #[test]
    fn test_build_status_items_with_sync_time() {
        let sync_time = Utc::now() - Duration::minutes(30);
        let items = build_status_items(3, Some(sync_time), true);

        // Should contain time reference
        let has_time = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("ago"))
        });
        assert!(has_time);
    }

    #[test]
    fn test_build_status_unhealthy() {
        let items = build_status_items(0, None, false);

        // Should contain warning indicator
        let has_warning = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("⚠"))
        });
        assert!(has_warning);
    }

    #[test]
    fn test_build_compact_status() {
        let items = build_compact_status(10, true);
        assert!(!items.is_empty());

        // Should contain mailbox emoji
        let has_mailbox = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("📬"))
        });
        assert!(has_mailbox);
    }

    #[test]
    fn test_build_detailed_status_with_providers() {
        let providers = vec![
            ("reddit".to_string(), 5),
            ("email".to_string(), 3),
            ("rss".to_string(), 2),
        ];

        let items = build_detailed_status(10, &providers, true);

        // Should contain provider names
        let has_reddit = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("reddit"))
        });
        assert!(has_reddit);
    }

    #[test]
    fn test_build_detailed_status_many_providers() {
        let providers = vec![
            ("reddit".to_string(), 5),
            ("email".to_string(), 3),
            ("rss".to_string(), 2),
            ("youtube".to_string(), 1),
            ("spotify".to_string(), 1),
        ];

        let items = build_detailed_status(12, &providers, true);

        // Should show "+2 more" indicator
        let has_more = items.iter().any(|item| {
            matches!(item, RenderItem::Text(s) if s.contains("more"))
        });
        assert!(has_more);
    }
}

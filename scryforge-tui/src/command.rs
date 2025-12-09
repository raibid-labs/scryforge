//! Command parsing for the TUI omnibar.
//!
//! This module handles parsing and executing commands prefixed with `:`.
//! Commands provide quick access to common operations like quitting,
//! syncing providers, refreshing views, and getting help.
//!
//! # Supported Commands
//!
//! - `:quit` or `:q` - Exit the application
//! - `:sync` - Sync all providers
//! - `:sync <provider>` - Sync a specific provider
//! - `:refresh` or `:r` - Refresh the current view
//! - `:help` or `:h` - Show help information
//! - Any text without `:` prefix is treated as a search query

use crate::search::{parse_search_query, SearchQuery};

/// Commands that can be executed from the omnibar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Exit the application
    Quit,
    /// Sync providers (optionally for a specific provider)
    Sync(Option<String>),
    /// Refresh the current view
    Refresh,
    /// Show help information
    Help,
    /// Execute a search query
    Search(SearchQuery),
}

/// Parse a command or search query from omnibar input.
///
/// If the input starts with `:`, it's parsed as a command.
/// Otherwise, it's treated as a search query.
///
/// # Examples
///
/// ```
/// use scryforge_tui::command::{parse_command, Command};
///
/// // Commands
/// assert_eq!(parse_command(":quit"), Some(Command::Quit));
/// assert_eq!(parse_command(":q"), Some(Command::Quit));
/// assert_eq!(parse_command(":sync"), Some(Command::Sync(None)));
/// assert_eq!(parse_command(":sync reddit"), Some(Command::Sync(Some("reddit".to_string()))));
/// assert_eq!(parse_command(":refresh"), Some(Command::Refresh));
/// assert_eq!(parse_command(":r"), Some(Command::Refresh));
/// assert_eq!(parse_command(":help"), Some(Command::Help));
/// assert_eq!(parse_command(":h"), Some(Command::Help));
///
/// // Search queries
/// let cmd = parse_command("rust programming").unwrap();
/// match cmd {
///     Command::Search(query) => assert_eq!(query.text, "rust programming"),
///     _ => panic!("Expected Search command"),
/// }
///
/// // Unknown commands return None
/// assert_eq!(parse_command(":unknown"), None);
/// ```
pub fn parse_command(input: &str) -> Option<Command> {
    let trimmed = input.trim();

    // Empty input
    if trimmed.is_empty() {
        return None;
    }

    // Check if it's a command (starts with :)
    if let Some(cmd_str) = trimmed.strip_prefix(':') {
        parse_command_string(cmd_str.trim())
    } else {
        // Treat as search query
        Some(Command::Search(parse_search_query(trimmed)))
    }
}

/// Parse a command string (without the leading :).
fn parse_command_string(cmd_str: &str) -> Option<Command> {
    if cmd_str.is_empty() {
        return None;
    }

    // Split command and arguments
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    let cmd = parts[0];
    let args = &parts[1..];

    match cmd.to_lowercase().as_str() {
        "quit" | "q" | "exit" => Some(Command::Quit),
        "sync" | "s" => {
            if args.is_empty() {
                Some(Command::Sync(None))
            } else {
                // Join all args as provider name (supports multi-word provider names)
                Some(Command::Sync(Some(args.join(" "))))
            }
        }
        "refresh" | "r" | "reload" => Some(Command::Refresh),
        "help" | "h" => Some(Command::Help),
        _ => None, // Unknown command
    }
}

/// Get help text for available commands.
pub fn get_help_text() -> &'static str {
    "Available Commands:\n\
     :quit, :q           - Exit the application\n\
     :sync [provider]    - Sync all providers or a specific provider\n\
     :refresh, :r        - Refresh the current view\n\
     :help, :h           - Show this help\n\
     \n\
     Search Syntax:\n\
     \"exact phrase\"      - Search for exact phrase\n\
     title:keyword       - Search in title field\n\
     content:keyword     - Search in content field\n\
     provider:name       - Filter by provider\n\
     -provider:name      - Exclude provider\n\
     in:stream_id        - Filter by stream\n\
     type:article        - Filter by content type\n\
     is:unread           - Filter by read status\n\
     is:saved            - Filter by saved status\n\
     since:30d           - Items from last 30 days\n\
     \n\
     Navigation:\n\
     h/l, Tab            - Move between panes\n\
     j/k, ↑/↓            - Navigate lists\n\
     g/G                 - Jump to first/last\n\
     Enter               - Open item\n\
     /                   - Search\n\
     :                   - Commands\n\
     q                   - Quit"
}

/// Get command suggestions based on partial input.
///
/// Returns a list of command suggestions that match the partial input.
/// Useful for implementing autocomplete in the omnibar.
pub fn get_command_suggestions(partial: &str) -> Vec<String> {
    // Don't return suggestions for empty input
    if partial.is_empty() {
        return Vec::new();
    }

    let partial_lower = partial.to_lowercase();
    let mut suggestions = Vec::new();

    let commands = [
        (":quit", "Exit the application"),
        (":q", "Exit (short)"),
        (":sync", "Sync all providers"),
        (":sync <provider>", "Sync specific provider"),
        (":refresh", "Refresh current view"),
        (":r", "Refresh (short)"),
        (":help", "Show help"),
        (":h", "Help (short)"),
    ];

    for (cmd, desc) in &commands {
        if cmd.starts_with(&partial_lower) {
            suggestions.push(format!("{} - {}", cmd, desc));
        }
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quit_commands() {
        assert_eq!(parse_command(":quit"), Some(Command::Quit));
        assert_eq!(parse_command(":q"), Some(Command::Quit));
        assert_eq!(parse_command(":exit"), Some(Command::Quit));
        assert_eq!(parse_command(":QUIT"), Some(Command::Quit));
        assert_eq!(parse_command(":Q"), Some(Command::Quit));
    }

    #[test]
    fn test_parse_sync_commands() {
        assert_eq!(parse_command(":sync"), Some(Command::Sync(None)));
        assert_eq!(parse_command(":s"), Some(Command::Sync(None)));
        assert_eq!(
            parse_command(":sync reddit"),
            Some(Command::Sync(Some("reddit".to_string())))
        );
        assert_eq!(
            parse_command(":sync my provider"),
            Some(Command::Sync(Some("my provider".to_string())))
        );
    }

    #[test]
    fn test_parse_refresh_commands() {
        assert_eq!(parse_command(":refresh"), Some(Command::Refresh));
        assert_eq!(parse_command(":r"), Some(Command::Refresh));
        assert_eq!(parse_command(":reload"), Some(Command::Refresh));
    }

    #[test]
    fn test_parse_help_commands() {
        assert_eq!(parse_command(":help"), Some(Command::Help));
        assert_eq!(parse_command(":h"), Some(Command::Help));
    }

    #[test]
    fn test_parse_unknown_command() {
        assert_eq!(parse_command(":unknown"), None);
        assert_eq!(parse_command(":foo"), None);
        assert_eq!(parse_command(":"), None);
    }

    #[test]
    fn test_parse_search_query() {
        let cmd = parse_command("rust programming").unwrap();
        match cmd {
            Command::Search(query) => {
                assert_eq!(query.text, "rust programming");
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_parse_search_query_with_filters() {
        let cmd = parse_command("title:kubernetes is:unread").unwrap();
        match cmd {
            Command::Search(query) => {
                assert_eq!(query.text, "title:kubernetes is:unread");
                assert!(query.has_advanced_syntax);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_parse_empty_input() {
        assert_eq!(parse_command(""), None);
        assert_eq!(parse_command("   "), None);
    }

    #[test]
    fn test_parse_whitespace_handling() {
        assert_eq!(parse_command("  :quit  "), Some(Command::Quit));
        assert_eq!(
            parse_command("  :sync  reddit  "),
            Some(Command::Sync(Some("reddit".to_string())))
        );
    }

    #[test]
    fn test_get_command_suggestions() {
        let suggestions = get_command_suggestions(":q");
        assert!(suggestions.len() >= 2); // :quit and :q
        assert!(suggestions.iter().any(|s| s.contains(":quit")));
        assert!(suggestions.iter().any(|s| s.contains(":q ")));

        let suggestions = get_command_suggestions(":s");
        assert!(suggestions.iter().any(|s| s.contains(":sync")));

        let suggestions = get_command_suggestions(":h");
        assert!(suggestions.iter().any(|s| s.contains(":help")));
        assert!(suggestions.iter().any(|s| s.contains(":h ")));
    }

    #[test]
    fn test_get_command_suggestions_empty() {
        let suggestions = get_command_suggestions("");
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_get_command_suggestions_no_match() {
        let suggestions = get_command_suggestions(":xyz");
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_help_text_not_empty() {
        let help = get_help_text();
        assert!(!help.is_empty());
        assert!(help.contains("quit"));
        assert!(help.contains("sync"));
        assert!(help.contains("refresh"));
        assert!(help.contains("help"));
    }
}

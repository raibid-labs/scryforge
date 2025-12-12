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
//! - `:theme <name>` - Switch to a named theme
//! - `:theme list` - List available themes
//! - `:plugin list` - List all loaded plugins
//! - `:plugin enable <id>` - Enable a plugin
//! - `:plugin disable <id>` - Disable a plugin
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
    /// Plugin management commands
    Plugin(PluginCommand),
    /// Theme management commands
    Theme(ThemeCommand),
}

/// Plugin management subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCommand {
    /// List all loaded plugins
    List,
    /// Enable a plugin by ID
    Enable(String),
    /// Disable a plugin by ID
    Disable(String),
    /// Show info about a specific plugin
    Info(String),
    /// Reload plugins from disk
    Reload,
}

/// Theme management subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeCommand {
    /// List available themes
    List,
    /// Set a specific theme by name
    Set(String),
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
        "refresh" | "r" => Some(Command::Refresh),
        "help" | "h" => Some(Command::Help),
        "plugin" | "plugins" => parse_plugin_command(args),
        "theme" | "themes" => parse_theme_command(args),
        _ => None, // Unknown command
    }
}

/// Parse plugin subcommands.
fn parse_plugin_command(args: &[&str]) -> Option<Command> {
    if args.is_empty() {
        // Default to listing plugins
        return Some(Command::Plugin(PluginCommand::List));
    }

    let subcommand = args[0].to_lowercase();
    let subargs = &args[1..];

    match subcommand.as_str() {
        "list" | "ls" => Some(Command::Plugin(PluginCommand::List)),
        "enable" | "on" => {
            if subargs.is_empty() {
                None // Requires plugin ID
            } else {
                Some(Command::Plugin(PluginCommand::Enable(subargs.join(" "))))
            }
        }
        "disable" | "off" => {
            if subargs.is_empty() {
                None // Requires plugin ID
            } else {
                Some(Command::Plugin(PluginCommand::Disable(subargs.join(" "))))
            }
        }
        "info" | "show" => {
            if subargs.is_empty() {
                None // Requires plugin ID
            } else {
                Some(Command::Plugin(PluginCommand::Info(subargs.join(" "))))
            }
        }
        "reload" | "refresh" => Some(Command::Plugin(PluginCommand::Reload)),
        _ => None,
    }
}

/// Parse theme subcommands.
fn parse_theme_command(args: &[&str]) -> Option<Command> {
    if args.is_empty() {
        // Default to listing themes
        return Some(Command::Theme(ThemeCommand::List));
    }

    let subcommand = args[0].to_lowercase();
    let _subargs = &args[1..];

    match subcommand.as_str() {
        "list" | "ls" => Some(Command::Theme(ThemeCommand::List)),
        _ => {
            // If not "list", treat the first arg as a theme name
            Some(Command::Theme(ThemeCommand::Set(args.join(" "))))
        }
    }
}

/// Get help text for available commands.
pub fn get_help_text() -> &'static str {
    "Available Commands:\n\
     :quit, :q           - Exit the application\n\
     :sync [provider]    - Sync all providers or a specific provider\n\
     :refresh, :r        - Refresh the current view\n\
     :help, :h           - Show this help\n\
     :theme <name>       - Switch to a theme (default, light, dracula, gruvbox, nord, solarized-dark, monokai)\n\
     :theme list         - List available themes\n\
     \n\
     Plugin Commands:\n\
     :plugin list        - List all loaded plugins\n\
     :plugin enable <id> - Enable a plugin\n\
     :plugin disable <id> - Disable a plugin\n\
     :plugin info <id>   - Show plugin details\n\
     :plugin reload      - Reload plugins from disk\n\
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
        (":theme list", "List available themes"),
        (":theme default", "Switch to default theme"),
        (":theme light", "Switch to light theme"),
        (":theme dracula", "Switch to Dracula theme"),
        (":theme gruvbox", "Switch to Gruvbox theme"),
        (":theme nord", "Switch to Nord theme"),
        (":theme solarized-dark", "Switch to Solarized Dark theme"),
        (":theme monokai", "Switch to Monokai theme"),
        (":plugin list", "List loaded plugins"),
        (":plugin enable <id>", "Enable a plugin"),
        (":plugin disable <id>", "Disable a plugin"),
        (":plugin info <id>", "Show plugin details"),
        (":plugin reload", "Reload plugins"),
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
    }

    #[test]
    fn test_parse_help_commands() {
        assert_eq!(parse_command(":help"), Some(Command::Help));
        assert_eq!(parse_command(":h"), Some(Command::Help));
    }

    #[test]
    fn test_parse_theme_commands() {
        assert_eq!(
            parse_command(":theme list"),
            Some(Command::Theme(ThemeCommand::List))
        );
        assert_eq!(
            parse_command(":theme"),
            Some(Command::Theme(ThemeCommand::List))
        );
        assert_eq!(
            parse_command(":theme dracula"),
            Some(Command::Theme(ThemeCommand::Set("dracula".to_string())))
        );
        assert_eq!(
            parse_command(":theme gruvbox"),
            Some(Command::Theme(ThemeCommand::Set("gruvbox".to_string())))
        );
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

        let suggestions = get_command_suggestions(":theme");
        assert!(suggestions.iter().any(|s| s.contains(":theme list")));
        assert!(suggestions.iter().any(|s| s.contains(":theme dracula")));
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
        assert!(help.contains("theme"));
        assert!(help.contains("plugin"));
    }

    #[test]
    fn test_parse_plugin_commands() {
        // Default to list
        assert_eq!(
            parse_command(":plugin"),
            Some(Command::Plugin(PluginCommand::List))
        );
        assert_eq!(
            parse_command(":plugin list"),
            Some(Command::Plugin(PluginCommand::List))
        );
        assert_eq!(
            parse_command(":plugin ls"),
            Some(Command::Plugin(PluginCommand::List))
        );
    }

    #[test]
    fn test_parse_plugin_enable_disable() {
        assert_eq!(
            parse_command(":plugin enable my-plugin"),
            Some(Command::Plugin(PluginCommand::Enable(
                "my-plugin".to_string()
            )))
        );
        assert_eq!(
            parse_command(":plugin disable my-plugin"),
            Some(Command::Plugin(PluginCommand::Disable(
                "my-plugin".to_string()
            )))
        );
        assert_eq!(
            parse_command(":plugin on test"),
            Some(Command::Plugin(PluginCommand::Enable("test".to_string())))
        );
        assert_eq!(
            parse_command(":plugin off test"),
            Some(Command::Plugin(PluginCommand::Disable("test".to_string())))
        );
    }

    #[test]
    fn test_parse_plugin_info_reload() {
        assert_eq!(
            parse_command(":plugin info my-plugin"),
            Some(Command::Plugin(PluginCommand::Info(
                "my-plugin".to_string()
            )))
        );
        assert_eq!(
            parse_command(":plugin reload"),
            Some(Command::Plugin(PluginCommand::Reload))
        );
    }

    #[test]
    fn test_parse_plugin_missing_args() {
        // These require arguments
        assert_eq!(parse_command(":plugin enable"), None);
        assert_eq!(parse_command(":plugin disable"), None);
        assert_eq!(parse_command(":plugin info"), None);
    }
}

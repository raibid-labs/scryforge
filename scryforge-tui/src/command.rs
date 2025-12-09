//! Command parsing for the omnibar.
//!
//! This module handles parsing and execution of commands entered in the omnibar.
//! Commands start with `:` and support various operations like sync, refresh, and quit.

use anyhow::{anyhow, Result};

/// Represents a parsed command from the omnibar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Trigger sync for a specific provider or all providers
    Sync { provider_id: Option<String> },
    /// Refresh the current view
    Refresh,
    /// Quit the application
    Quit,
    /// Show help information
    Help,
}

/// Parse a command string from the omnibar.
///
/// Commands must start with `:`. The leading `:` should be included in the input.
///
/// # Examples
///
/// ```
/// use scryforge_tui::command::{parse_command, Command};
///
/// let cmd = parse_command(":quit").unwrap();
/// assert_eq!(cmd, Command::Quit);
///
/// let cmd = parse_command(":sync email").unwrap();
/// assert_eq!(cmd, Command::Sync { provider_id: Some("email".to_string()) });
/// ```
pub fn parse_command(input: &str) -> Result<Command> {
    let input = input.trim();

    if !input.starts_with(':') {
        return Err(anyhow!("Commands must start with ':'"));
    }

    let input = &input[1..]; // Remove leading ':'
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return Err(anyhow!("Empty command"));
    }

    match parts[0].to_lowercase().as_str() {
        "q" | "quit" => {
            if parts.len() > 1 {
                return Err(anyhow!("quit command takes no arguments"));
            }
            Ok(Command::Quit)
        }
        "sync" => {
            let provider_id = if parts.len() > 1 {
                Some(parts[1].to_string())
            } else {
                None
            };
            Ok(Command::Sync { provider_id })
        }
        "refresh" => {
            if parts.len() > 1 {
                return Err(anyhow!("refresh command takes no arguments"));
            }
            Ok(Command::Refresh)
        }
        "help" | "h" => {
            if parts.len() > 1 {
                return Err(anyhow!("help command takes no arguments"));
            }
            Ok(Command::Help)
        }
        other => Err(anyhow!("Unknown command: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quit() {
        assert_eq!(parse_command(":quit").unwrap(), Command::Quit);
        assert_eq!(parse_command(":q").unwrap(), Command::Quit);
        assert_eq!(parse_command(":QUIT").unwrap(), Command::Quit);
        assert_eq!(parse_command(":Q").unwrap(), Command::Quit);
    }

    #[test]
    fn test_parse_quit_with_args_fails() {
        assert!(parse_command(":quit now").is_err());
    }

    #[test]
    fn test_parse_sync() {
        assert_eq!(
            parse_command(":sync").unwrap(),
            Command::Sync { provider_id: None }
        );
        assert_eq!(
            parse_command(":sync email").unwrap(),
            Command::Sync {
                provider_id: Some("email".to_string())
            }
        );
        assert_eq!(
            parse_command(":SYNC rss").unwrap(),
            Command::Sync {
                provider_id: Some("rss".to_string())
            }
        );
    }

    #[test]
    fn test_parse_refresh() {
        assert_eq!(parse_command(":refresh").unwrap(), Command::Refresh);
        assert_eq!(parse_command(":REFRESH").unwrap(), Command::Refresh);
    }

    #[test]
    fn test_parse_refresh_with_args_fails() {
        assert!(parse_command(":refresh now").is_err());
    }

    #[test]
    fn test_parse_help() {
        assert_eq!(parse_command(":help").unwrap(), Command::Help);
        assert_eq!(parse_command(":h").unwrap(), Command::Help);
        assert_eq!(parse_command(":HELP").unwrap(), Command::Help);
    }

    #[test]
    fn test_parse_help_with_args_fails() {
        assert!(parse_command(":help me").is_err());
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(parse_command(":unknown").is_err());
        assert!(parse_command(":foobar").is_err());
    }

    #[test]
    fn test_parse_missing_colon() {
        assert!(parse_command("quit").is_err());
        assert!(parse_command("sync").is_err());
    }

    #[test]
    fn test_parse_empty_command() {
        assert!(parse_command(":").is_err());
        assert!(parse_command(": ").is_err());
    }

    #[test]
    fn test_parse_with_whitespace() {
        assert_eq!(parse_command("  :quit  ").unwrap(), Command::Quit);
        assert_eq!(
            parse_command("  :sync  email  ").unwrap(),
            Command::Sync {
                provider_id: Some("email".to_string())
            }
        );
    }
}

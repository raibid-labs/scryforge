//! Search query parsing for the omnibar.
//!
//! This module handles parsing search queries with advanced filter support.
//! The parsed query is then sent to the daemon via JSON-RPC.
//!
//! # Supported Syntax
//!
//! - `"exact phrase"` - Search for exact phrase
//! - `title:keyword` - Search only in title field
//! - `content:keyword` - Search only in content field
//! - `provider:reddit` - Filter by provider
//! - `-provider:reddit` - Exclude provider (negation)
//! - `in:stream_id` or `stream:stream_id` - Filter by stream ID
//! - `type:article` - Filter by content type
//! - `is:read` or `is:unread` - Filter by read status
//! - `is:saved` or `is:starred` - Filter by saved status
//! - `since:30d` - Items from last 30 days
//! - `since:7d` - Items from last 7 days
//! - `date:2024-01-01` - Items from specific date
//! - `date:2024-01-01..2024-06-01` - Items in date range

use serde::{Deserialize, Serialize};

/// Represents a parsed search query with filters.
///
/// The TUI parser extracts basic filters and passes the entire query string
/// to the daemon, which performs advanced FTS5 parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The full query string (passed to daemon as-is for FTS5 parsing)
    pub text: String,
    /// Filter for specific stream(s) (extracted for UI hints)
    pub stream_filter: Option<String>,
    /// Filter for provider (may include negation with !, extracted for UI hints)
    pub provider_filter: Option<String>,
    /// Filter for content type (extracted for UI hints)
    pub type_filter: Option<ContentTypeFilter>,
    /// Filter for read/unread/saved status (extracted for UI hints)
    pub status_filters: Vec<StatusFilter>,
    /// Date filter specification (extracted for UI hints)
    pub date_filter: Option<String>,
    /// Whether the query contains advanced FTS syntax
    pub has_advanced_syntax: bool,
}

/// Content type filters for search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentTypeFilter {
    Article,
    Email,
    Video,
    Track,
    Task,
    Event,
    Bookmark,
}

impl ContentTypeFilter {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "article" => Some(Self::Article),
            "email" => Some(Self::Email),
            "video" => Some(Self::Video),
            "track" | "song" | "music" => Some(Self::Track),
            "task" | "todo" => Some(Self::Task),
            "event" | "calendar" => Some(Self::Event),
            "bookmark" => Some(Self::Bookmark),
            _ => None,
        }
    }
}

/// Status filters for search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    Read,
    Unread,
    Saved,
}

impl StatusFilter {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "read" => Some(Self::Read),
            "unread" => Some(Self::Unread),
            "saved" | "starred" | "favorite" => Some(Self::Saved),
            _ => None,
        }
    }
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            stream_filter: None,
            provider_filter: None,
            type_filter: None,
            status_filters: Vec::new(),
            date_filter: None,
            has_advanced_syntax: false,
        }
    }
}

/// Parse a search query from the omnibar.
///
/// This function extracts basic filters for UI hints but preserves the full
/// query string to pass to the daemon for advanced FTS5 parsing.
///
/// # Examples
///
/// ```
/// use scryforge_tui::search::{parse_search_query, StatusFilter};
///
/// let query = parse_search_query("rust programming");
/// assert_eq!(query.text, "rust programming");
///
/// let query = parse_search_query("in:email important meeting");
/// assert_eq!(query.stream_filter, Some("email".to_string()));
/// assert_eq!(query.text, "in:email important meeting");
///
/// let query = parse_search_query("title:kubernetes -provider:reddit is:unread");
/// assert_eq!(query.provider_filter, Some("!reddit".to_string()));
/// assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
/// assert!(query.has_advanced_syntax);
/// ```
pub fn parse_search_query(input: &str) -> SearchQuery {
    let mut query = SearchQuery {
        text: input.to_string(),
        ..Default::default()
    };

    // Check for advanced syntax indicators
    query.has_advanced_syntax = input.contains("title:")
        || input.contains("content:")
        || input.contains("provider:")
        || input.contains("-provider:")
        || input.contains("since:")
        || input.contains("date:")
        || input.contains('"');

    // Extract basic filters for UI hints (without modifying the original query)
    for word in input.split_whitespace() {
        // Provider filter (with negation support)
        if let Some(provider) = word.strip_prefix("-provider:") {
            query.provider_filter = Some(format!("!{}", provider));
        } else if let Some(provider) = word.strip_prefix("provider:") {
            query.provider_filter = Some(provider.to_string());
        }

        // Stream filter
        if let Some(stream) = word.strip_prefix("in:") {
            query.stream_filter = Some(stream.to_string());
        } else if let Some(stream) = word.strip_prefix("stream:") {
            query.stream_filter = Some(stream.to_string());
        }

        // Type filter
        if let Some(content_type) = word.strip_prefix("type:") {
            if let Some(ctype) = ContentTypeFilter::from_str(content_type) {
                query.type_filter = Some(ctype);
            }
        }

        // Status filters
        if let Some(status) = word.strip_prefix("is:") {
            if let Some(status_filter) = StatusFilter::from_str(status) {
                if !query.status_filters.contains(&status_filter) {
                    query.status_filters.push(status_filter);
                }
            }
        }

        // Date filters
        if let Some(date_spec) = word.strip_prefix("since:") {
            query.date_filter = Some(format!("since:{}", date_spec));
        } else if let Some(date_spec) = word.strip_prefix("date:") {
            query.date_filter = Some(format!("date:{}", date_spec));
        }
    }

    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_query() {
        let query = parse_search_query("hello world");
        assert_eq!(query.text, "hello world");
        assert_eq!(query.stream_filter, None);
        assert_eq!(query.type_filter, None);
        assert!(query.status_filters.is_empty());
        assert!(!query.has_advanced_syntax);
    }

    #[test]
    fn test_parse_stream_filter() {
        let query = parse_search_query("in:email meeting");
        assert_eq!(query.text, "in:email meeting");
        assert_eq!(query.stream_filter, Some("email".to_string()));
        assert!(!query.has_advanced_syntax); // 'in:' alone doesn't indicate advanced syntax
    }

    #[test]
    fn test_parse_provider_filter() {
        let query = parse_search_query("provider:reddit rust");
        assert_eq!(query.provider_filter, Some("reddit".to_string()));
        assert!(query.has_advanced_syntax);
    }

    #[test]
    fn test_parse_negated_provider() {
        let query = parse_search_query("-provider:reddit rust");
        assert_eq!(query.provider_filter, Some("!reddit".to_string()));
        assert!(query.has_advanced_syntax);
    }

    #[test]
    fn test_parse_type_filter() {
        let query = parse_search_query("type:article rust");
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Article));
    }

    #[test]
    fn test_parse_type_filter_aliases() {
        let query = parse_search_query("type:song");
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Track));

        let query = parse_search_query("type:todo");
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Task));
    }

    #[test]
    fn test_parse_status_filter() {
        let query = parse_search_query("is:unread");
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);

        let query = parse_search_query("is:read");
        assert_eq!(query.status_filters, vec![StatusFilter::Read]);

        let query = parse_search_query("is:saved");
        assert_eq!(query.status_filters, vec![StatusFilter::Saved]);
    }

    #[test]
    fn test_parse_status_filter_aliases() {
        let query = parse_search_query("is:starred");
        assert_eq!(query.status_filters, vec![StatusFilter::Saved]);

        let query = parse_search_query("is:favorite");
        assert_eq!(query.status_filters, vec![StatusFilter::Saved]);
    }

    #[test]
    fn test_parse_multiple_filters() {
        let query = parse_search_query("in:email type:email is:unread important");
        assert_eq!(query.stream_filter, Some("email".to_string()));
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Email));
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
    }

    #[test]
    fn test_parse_multiple_status_filters() {
        let query = parse_search_query("is:unread is:saved");
        assert_eq!(query.status_filters.len(), 2);
        assert!(query.status_filters.contains(&StatusFilter::Unread));
        assert!(query.status_filters.contains(&StatusFilter::Saved));
    }

    #[test]
    fn test_parse_duplicate_status_filters() {
        let query = parse_search_query("is:unread is:unread");
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
    }

    #[test]
    fn test_parse_title_search() {
        let query = parse_search_query("title:kubernetes");
        assert!(query.has_advanced_syntax);
        assert_eq!(query.text, "title:kubernetes");
    }

    #[test]
    fn test_parse_exact_phrase() {
        let query = parse_search_query(r#""getting started" guide"#);
        assert!(query.has_advanced_syntax);
        assert_eq!(query.text, r#""getting started" guide"#);
    }

    #[test]
    fn test_parse_since_filter() {
        let query = parse_search_query("since:30d rust");
        assert_eq!(query.date_filter, Some("since:30d".to_string()));
        assert!(query.has_advanced_syntax);
    }

    #[test]
    fn test_parse_date_range() {
        let query = parse_search_query("date:2024-01-01..2024-06-01 rust");
        assert_eq!(
            query.date_filter,
            Some("date:2024-01-01..2024-06-01".to_string())
        );
        assert!(query.has_advanced_syntax);
    }

    #[test]
    fn test_parse_complex_query() {
        let query = parse_search_query(r#"title:kubernetes -provider:reddit is:unread since:7d"#);

        assert!(query.has_advanced_syntax);
        assert_eq!(query.provider_filter, Some("!reddit".to_string()));
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
        assert_eq!(query.date_filter, Some("since:7d".to_string()));
        assert_eq!(
            query.text,
            r#"title:kubernetes -provider:reddit is:unread since:7d"#
        );
    }

    #[test]
    fn test_preserves_full_query() {
        // The TUI parser should preserve the full query for the daemon to parse
        let input = "title:rust AND (content:async OR content:await) -provider:reddit";
        let query = parse_search_query(input);
        assert_eq!(query.text, input);
        assert!(query.has_advanced_syntax);
    }
}

//! Search query parsing for the omnibar.
//!
//! This module handles parsing search queries with filter support.
//! Filters include `in:stream`, `type:article`, and `is:unread`.

use serde::{Deserialize, Serialize};

/// Represents a parsed search query with filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The search text (non-filter terms)
    pub text: String,
    /// Filter for specific stream(s)
    pub stream_filter: Option<String>,
    /// Filter for content type
    pub type_filter: Option<ContentTypeFilter>,
    /// Filter for read/unread/saved status
    pub status_filters: Vec<StatusFilter>,
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

/// Parse a search query from the omnibar.
///
/// The query can include filters:
/// - `in:stream_id` - Filter by stream
/// - `type:article` - Filter by content type
/// - `is:unread` - Filter by status (read/unread/saved)
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
/// assert_eq!(query.text, "important meeting");
///
/// let query = parse_search_query("type:article is:unread rust");
/// assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
/// assert_eq!(query.text, "rust");
/// ```
pub fn parse_search_query(input: &str) -> SearchQuery {
    let mut text_parts = Vec::new();
    let mut stream_filter = None;
    let mut type_filter = None;
    let mut status_filters = Vec::new();

    for word in input.split_whitespace() {
        if let Some(filter_value) = word.strip_prefix("in:") {
            stream_filter = Some(filter_value.to_string());
        } else if let Some(filter_value) = word.strip_prefix("type:") {
            if let Some(content_type) = ContentTypeFilter::from_str(filter_value) {
                type_filter = Some(content_type);
            } else {
                // If unknown type, treat as regular text
                text_parts.push(word);
            }
        } else if let Some(filter_value) = word.strip_prefix("is:") {
            if let Some(status) = StatusFilter::from_str(filter_value) {
                if !status_filters.contains(&status) {
                    status_filters.push(status);
                }
            } else {
                // If unknown status, treat as regular text
                text_parts.push(word);
            }
        } else {
            text_parts.push(word);
        }
    }

    SearchQuery {
        text: text_parts.join(" "),
        stream_filter,
        type_filter,
        status_filters,
    }
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
    }

    #[test]
    fn test_parse_stream_filter() {
        let query = parse_search_query("in:email meeting");
        assert_eq!(query.text, "meeting");
        assert_eq!(query.stream_filter, Some("email".to_string()));
    }

    #[test]
    fn test_parse_type_filter() {
        let query = parse_search_query("type:article rust");
        assert_eq!(query.text, "rust");
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
        assert_eq!(query.text, "important");
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
    fn test_parse_unknown_filter_values() {
        let query = parse_search_query("type:unknown is:invalid");
        assert_eq!(query.text, "type:unknown is:invalid");
    }

    #[test]
    fn test_parse_empty_query() {
        let query = parse_search_query("");
        assert_eq!(query.text, "");
        assert_eq!(query.stream_filter, None);
        assert_eq!(query.type_filter, None);
        assert!(query.status_filters.is_empty());
    }

    #[test]
    fn test_parse_only_filters() {
        let query = parse_search_query("in:email type:email is:unread");
        assert_eq!(query.text, "");
        assert_eq!(query.stream_filter, Some("email".to_string()));
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Email));
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
    }

    #[test]
    fn test_parse_mixed_order() {
        let query = parse_search_query("hello in:email world is:unread type:article");
        assert_eq!(query.text, "hello world");
        assert_eq!(query.stream_filter, Some("email".to_string()));
        assert_eq!(query.type_filter, Some(ContentTypeFilter::Article));
        assert_eq!(query.status_filters, vec![StatusFilter::Unread]);
    }
}

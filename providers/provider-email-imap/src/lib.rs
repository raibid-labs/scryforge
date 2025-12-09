//! # provider-email-imap
//!
//! IMAP email provider for Scryforge.
//!
//! This provider connects to IMAP servers to fetch emails, converting them into
//! unified `Item` structs. It supports both feeds (mailboxes) and collections (folders),
//! handles multipart messages, encoded headers, and common IMAP edge cases.
//!
//! ## Features
//!
//! - TLS/SSL connection support (enabled by default)
//! - Password authentication (OAuth can be added later)
//! - Mailbox synchronization as feeds
//! - Folder listing as collections
//! - Email parsing with proper encoding handling
//! - Multipart message support (plain text preferred over HTML)
//! - Attachment metadata extraction

use async_imap::Session;
use async_native_tls::{TlsConnector, TlsStream};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use fusabi_streams_core::prelude::*;
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum ImapError {
    #[error("IMAP connection error: {0}")]
    Connection(String),

    #[error("IMAP authentication error: {0}")]
    Authentication(String),

    #[error("IMAP command error: {0}")]
    Command(String),

    #[error("Email parse error: {0}")]
    Parse(String),

    #[error("Mailbox not found: {0}")]
    MailboxNotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ImapError> for StreamError {
    fn from(err: ImapError) -> Self {
        match err {
            ImapError::Connection(e) => StreamError::Network(format!("IMAP connection: {e}")),
            ImapError::Authentication(e) => StreamError::AuthRequired(e),
            ImapError::Command(e) => StreamError::Provider(format!("IMAP command: {e}")),
            ImapError::Parse(e) => StreamError::Provider(format!("Email parse: {e}")),
            ImapError::MailboxNotFound(e) => StreamError::StreamNotFound(e),
            ImapError::FolderNotFound(e) => StreamError::StreamNotFound(e),
            ImapError::Tls(e) => StreamError::Network(format!("TLS: {e}")),
            ImapError::Io(e) => StreamError::Internal(format!("IO: {e}")),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for an IMAP mailbox to sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapMailboxConfig {
    /// Unique identifier for this mailbox
    pub id: String,
    /// Display name for the mailbox
    pub name: String,
    /// IMAP mailbox name (e.g., "INBOX", "Sent", "Archive")
    pub mailbox_name: String,
    /// Optional description
    pub description: Option<String>,
    /// Optional icon/emoji
    pub icon: Option<String>,
}

/// Configuration for the IMAP provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    /// IMAP server hostname
    pub server: String,
    /// IMAP server port (typically 993 for TLS, 143 for STARTTLS)
    pub port: u16,
    /// Username for authentication
    pub username: String,
    /// Password for authentication
    pub password: String,
    /// Use TLS/SSL (default: true)
    #[serde(default = "default_use_tls")]
    pub use_tls: bool,
    /// List of mailboxes to sync as feeds
    pub mailboxes: Vec<ImapMailboxConfig>,
}

fn default_use_tls() -> bool {
    true
}

impl Default for ImapConfig {
    fn default() -> Self {
        Self {
            server: String::new(),
            port: 993,
            username: String::new(),
            password: String::new(),
            use_tls: true,
            mailboxes: Vec::new(),
        }
    }
}

// ============================================================================
// IMAP Provider
// ============================================================================

/// IMAP email provider.
pub struct ImapProvider {
    config: Arc<ImapConfig>,
}

impl ImapProvider {
    /// Create a new IMAP provider with the given configuration.
    pub fn new(config: ImapConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create an IMAP session.
    async fn create_session(&self) -> std::result::Result<Session<TlsStream<async_std::net::TcpStream>>, ImapError> {
        let server = &self.config.server;
        let port = self.config.port;

        debug!("Connecting to IMAP server {}:{}", server, port);

        // Connect to the server
        let tcp_stream = async_std::net::TcpStream::connect((server.as_str(), port))
            .await
            .map_err(|e| ImapError::Connection(format!("TCP connection failed: {e}")))?;

        // Wrap in TLS
        let tls = TlsConnector::new();
        let tls_stream = tls
            .connect(server, tcp_stream)
            .await
            .map_err(|e| ImapError::Tls(format!("TLS handshake failed: {e}")))?;

        // Create IMAP client
        let client = async_imap::Client::new(tls_stream);

        debug!("IMAP client connected, attempting login");

        // Authenticate
        let session = client
            .login(&self.config.username, &self.config.password)
            .await
            .map_err(|e| ImapError::Authentication(format!("Login failed: {:?}", e.0)))?;

        info!("Successfully authenticated to IMAP server");

        Ok(session)
    }

    /// Fetch emails from a mailbox.
    async fn fetch_emails(
        &self,
        mailbox_name: &str,
        limit: Option<u32>,
        since: Option<DateTime<Utc>>,
    ) -> std::result::Result<Vec<ImapEmail>, ImapError> {
        let mut session = self.create_session().await?;

        // Select the mailbox
        session
            .select(mailbox_name)
            .await
            .map_err(|e| ImapError::Command(format!("SELECT failed: {e}")))?;

        // Build search criteria
        let search_query = if let Some(since_date) = since {
            // Format date as "DD-Mon-YYYY" (IMAP date format)
            let date_str = since_date.format("%d-%b-%Y").to_string();
            format!("SINCE {}", date_str)
        } else {
            "ALL".to_string()
        };

        debug!("Searching with query: {}", search_query);

        // Search for messages
        let message_ids = session
            .search(&search_query)
            .await
            .map_err(|e| ImapError::Command(format!("SEARCH failed: {e}")))?;

        debug!("Found {} messages", message_ids.len());

        // Convert HashSet to Vec and sort (higher UIDs are newer)
        let mut message_ids: Vec<u32> = message_ids.into_iter().collect();
        message_ids.sort_unstable();

        // Apply limit by taking the most recent messages
        let message_ids: Vec<u32> = if let Some(limit) = limit {
            message_ids
                .into_iter()
                .rev()
                .take(limit as usize)
                .collect()
        } else {
            message_ids
        };

        if message_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch message data
        let sequence_set = message_ids
            .iter()
            .map(|id: &u32| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let mut emails = Vec::new();

        {
            let mut fetch_stream = session
                .fetch(&sequence_set, "(UID FLAGS INTERNALDATE BODY.PEEK[])")
                .await
                .map_err(|e| ImapError::Command(format!("FETCH failed: {e}")))?;

            while let Some(fetch_result) = fetch_stream.next().await {
                let fetch = match fetch_result {
                    Ok(f) => f,
                    Err(e) => {
                        error!("Error fetching message: {}", e);
                        continue;
                    }
                };
                let uid = fetch.uid.unwrap_or(0);
                let body = match fetch.body() {
                    Some(b) => b,
                    None => {
                        warn!("No body found for message UID {}", uid);
                        continue;
                    }
                };

                let internal_date = fetch.internal_date().map(|dt| dt.to_rfc2822());
                let flags = fetch.flags();
                let flags_vec: Vec<_> = flags.collect();
                let is_seen = flags_vec.iter().any(|f| matches!(f, async_imap::types::Flag::Seen));

                emails.push(ImapEmail {
                    uid,
                    body: body.to_vec(),
                    internal_date,
                    is_seen,
                });
            }
        }

        // Logout
        session
            .logout()
            .await
            .map_err(|e| ImapError::Command(format!("LOGOUT failed: {e}")))?;

        Ok(emails)
    }

    /// List all folders in the IMAP account.
    async fn list_folders(&self) -> std::result::Result<Vec<ImapFolder>, ImapError> {
        let mut session = self.create_session().await?;

        // List all mailboxes
        let mailboxes_stream = session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| ImapError::Command(format!("LIST failed: {e}")))?;

        let mut folders = Vec::new();

        {
            let mut mailboxes = mailboxes_stream;
            while let Some(mailbox_result) = mailboxes.next().await {
                let mailbox = match mailbox_result {
                    Ok(m) => m,
                    Err(e) => {
                        error!("Error listing mailbox: {}", e);
                        continue;
                    }
                };
                let name = mailbox.name();
                let attributes = mailbox.attributes();

                // Skip special system folders that shouldn't be displayed
                if attributes.iter().any(|attr| {
                    matches!(
                        attr,
                        async_imap::types::NameAttribute::NoSelect
                            | async_imap::types::NameAttribute::NoInferiors
                    )
                }) {
                    continue;
                }

                folders.push(ImapFolder {
                    name: name.to_string(),
                    attributes: format!("{:?}", attributes),
                });
            }
        }

        // Logout
        session
            .logout()
            .await
            .map_err(|e| ImapError::Command(format!("LOGOUT failed: {e}")))?;

        Ok(folders)
    }

    /// Convert an IMAP email to our Item struct.
    fn email_to_item(
        &self,
        email: &ImapEmail,
        mailbox_config: &ImapMailboxConfig,
    ) -> std::result::Result<Item, ImapError> {
        let stream_id = StreamId::new("email-imap", "mailbox", &mailbox_config.id);

        // Parse the email
        let parser = MessageParser::default();
        let message = parser
            .parse(&email.body)
            .ok_or_else(|| ImapError::Parse("Failed to parse email".to_string()))?;

        // Generate item ID from UID
        let item_id = ItemId::new("email-imap", &format!("uid-{}", email.uid));

        // Extract subject
        let title = message
            .subject()
            .unwrap_or("(No Subject)")
            .to_string();

        // Extract body (prefer plain text over HTML)
        let body_text = message
            .body_text(0)
            .map(|t| t.to_string());

        let body_html = message
            .body_html(0)
            .map(|h| h.to_string());

        // Create a snippet from the body
        let snippet = if let Some(text) = &body_text {
            truncate_text(text, 200)
        } else if let Some(html) = &body_html {
            truncate_text(&strip_html_tags(html), 200)
        } else {
            String::new()
        };

        let content = ItemContent::Email {
            subject: title.clone(),
            body_text: body_text.clone(),
            body_html: body_html.clone(),
            snippet,
        };

        // Extract author from "From" header
        let author = message.from().and_then(|from| {
            from.first().map(|addr| Author {
                name: addr.name().map(|n| n.to_string()).unwrap_or_else(|| {
                    addr.address()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                }),
                email: addr.address().map(|a| a.to_string()),
                url: None,
                avatar_url: None,
            })
        });

        // Extract date
        let published = message
            .date()
            .and_then(|&ts| DateTime::from_timestamp(ts.into(), 0))
            .or_else(|| {
                email.internal_date.as_ref().and_then(|date_str| {
                    chrono::DateTime::parse_from_rfc2822(date_str)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                })
            })
            .or_else(|| Some(Utc::now()));

        // Build metadata
        let mut metadata = HashMap::new();

        // Add To recipients
        if let Some(to) = message.to() {
            let to_addrs: Vec<String> = to
                .iter()
                .filter_map(|addr| addr.address().map(|a| a.to_string()))
                .collect();
            if !to_addrs.is_empty() {
                metadata.insert("to".to_string(), to_addrs.join(", "));
            }
        }

        // Add Cc recipients
        if let Some(cc) = message.cc() {
            let cc_addrs: Vec<String> = cc
                .iter()
                .filter_map(|addr| addr.address().map(|a| a.to_string()))
                .collect();
            if !cc_addrs.is_empty() {
                metadata.insert("cc".to_string(), cc_addrs.join(", "));
            }
        }

        // Add attachment information
        let attachments: Vec<String> = message
            .attachments()
            .filter_map(|att| {
                att.attachment_name()
                    .map(|name: &str| name.to_string())
            })
            .collect();

        if !attachments.is_empty() {
            metadata.insert("attachments".to_string(), attachments.join(", "));
            metadata.insert("attachment_count".to_string(), attachments.len().to_string());
        }

        // Add message ID
        if let Some(msg_id) = message.message_id() {
            metadata.insert("message_id".to_string(), msg_id.to_string());
        }

        Ok(Item {
            id: item_id,
            stream_id,
            title,
            content,
            author,
            published,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: email.is_seen,
            is_saved: false,
            tags: vec![],
            metadata,
        })
    }

    /// Find a mailbox configuration by ID.
    fn find_mailbox(&self, feed_id: &FeedId) -> Option<&ImapMailboxConfig> {
        self.config.mailboxes.iter().find(|m| m.id == feed_id.0)
    }
}

// ============================================================================
// Helper Structs
// ============================================================================

/// Represents a fetched IMAP email.
struct ImapEmail {
    uid: u32,
    body: Vec<u8>,
    internal_date: Option<String>,
    is_seen: bool,
}

/// Represents an IMAP folder.
struct ImapFolder {
    name: String,
    attributes: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Truncate text to a maximum length, adding ellipsis if truncated.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut truncated = text.chars().take(max_len).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

/// Strip HTML tags from a string (basic implementation).
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result
}

// ============================================================================
// Provider Trait Implementation
// ============================================================================

#[async_trait]
impl Provider for ImapProvider {
    fn id(&self) -> &'static str {
        "email-imap"
    }

    fn name(&self) -> &'static str {
        "IMAP Email"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to create a session to check connectivity
        match self.create_session().await {
            Ok(mut session) => {
                // Try to logout gracefully
                let _ = session.logout().await;

                Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some("Successfully connected to IMAP server".to_string()),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                })
            }
            Err(e) => {
                warn!("IMAP health check failed: {}", e);
                Ok(ProviderHealth {
                    is_healthy: false,
                    message: Some(format!("Failed to connect: {}", e)),
                    last_sync: None,
                    error_count: 1,
                })
            }
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();
        let mut items_added = 0;
        let mut errors = Vec::new();

        info!("Syncing {} IMAP mailboxes", self.config.mailboxes.len());

        for mailbox_config in &self.config.mailboxes {
            match self
                .fetch_emails(&mailbox_config.mailbox_name, Some(100), None)
                .await
            {
                Ok(emails) => {
                    items_added += emails.len() as u32;
                    debug!(
                        "Fetched {} emails from {}",
                        emails.len(),
                        mailbox_config.name
                    );
                }
                Err(e) => {
                    error!("Failed to fetch mailbox {}: {}", mailbox_config.name, e);
                    errors.push(format!("{}: {}", mailbox_config.name, e));
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SyncResult {
            success: errors.is_empty(),
            items_added,
            items_updated: 0,
            items_removed: 0,
            errors,
            duration_ms,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: true,
            has_saved_items: false,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        let actions = vec![
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show email preview".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "mark_read".to_string(),
                name: "Mark as Read".to_string(),
                description: "Mark email as read".to_string(),
                kind: ActionKind::MarkRead,
                keyboard_shortcut: Some("r".to_string()),
            },
            Action {
                id: "mark_unread".to_string(),
                name: "Mark as Unread".to_string(),
                description: "Mark email as unread".to_string(),
                kind: ActionKind::MarkUnread,
                keyboard_shortcut: Some("u".to_string()),
            },
            Action {
                id: "archive".to_string(),
                name: "Archive".to_string(),
                description: "Archive email".to_string(),
                kind: ActionKind::Archive,
                keyboard_shortcut: Some("a".to_string()),
            },
            Action {
                id: "delete".to_string(),
                name: "Delete".to_string(),
                description: "Delete email".to_string(),
                kind: ActionKind::Delete,
                keyboard_shortcut: Some("d".to_string()),
            },
        ];

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        // For now, these are placeholder implementations
        // In a full implementation, these would interact with the IMAP server
        match action.kind {
            ActionKind::MarkRead | ActionKind::MarkUnread | ActionKind::Archive | ActionKind::Delete => {
                Ok(ActionResult {
                    success: true,
                    message: Some(format!(
                        "Action '{}' would be executed on email: {}",
                        action.name, item.title
                    )),
                    data: Some(serde_json::json!({
                        "item_id": item.id.as_str(),
                        "action": &action.id,
                    })),
                })
            }
            _ => Ok(ActionResult {
                success: true,
                message: Some(format!("Executed action: {}", action.name)),
                data: None,
            }),
        }
    }
}

// ============================================================================
// HasFeeds Trait Implementation
// ============================================================================

#[async_trait]
impl HasFeeds for ImapProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        Ok(self
            .config
            .mailboxes
            .iter()
            .map(|mc| Feed {
                id: FeedId(mc.id.clone()),
                name: mc.name.clone(),
                description: mc.description.clone(),
                icon: mc.icon.clone(),
                unread_count: None, // Would require checking UNSEEN count
                total_count: None,  // Would require checking EXISTS count
            })
            .collect())
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let mailbox_config = self
            .find_mailbox(feed_id)
            .ok_or_else(|| StreamError::StreamNotFound(feed_id.0.clone()))?;

        let emails = self
            .fetch_emails(
                &mailbox_config.mailbox_name,
                options.limit,
                options.since,
            )
            .await
            .map_err(StreamError::from)?;

        let mut items: Vec<Item> = emails
            .iter()
            .filter_map(|email| {
                match self.email_to_item(email, mailbox_config) {
                    Ok(item) => Some(item),
                    Err(e) => {
                        error!("Failed to convert email to item: {}", e);
                        None
                    }
                }
            })
            .collect();

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Sort by published date (newest first)
        items.sort_by(|a, b| {
            let a_date = a.published.unwrap_or(DateTime::<Utc>::MIN_UTC);
            let b_date = b.published.unwrap_or(DateTime::<Utc>::MIN_UTC);
            b_date.cmp(&a_date)
        });

        // Apply offset
        let offset = options.offset.unwrap_or(0) as usize;
        let items: Vec<Item> = items.into_iter().skip(offset).collect();

        Ok(items)
    }
}

// ============================================================================
// HasCollections Trait Implementation
// ============================================================================

#[async_trait]
impl HasCollections for ImapProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let folders = self.list_folders().await.map_err(StreamError::from)?;

        Ok(folders
            .iter()
            .map(|folder| Collection {
                id: CollectionId(folder.name.clone()),
                name: folder.name.clone(),
                description: Some(format!("IMAP folder: {}", folder.name)),
                icon: get_folder_icon(&folder.name),
                item_count: 0, // Would require examining the folder
                is_editable: false,
                owner: Some(self.config.username.clone()),
            })
            .collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        // Fetch emails from the folder
        let emails = self
            .fetch_emails(&collection_id.0, Some(100), None)
            .await
            .map_err(StreamError::from)?;

        // Create a temporary mailbox config for this folder
        let temp_mailbox = ImapMailboxConfig {
            id: collection_id.0.clone(),
            name: collection_id.0.clone(),
            mailbox_name: collection_id.0.clone(),
            description: None,
            icon: None,
        };

        let items: Vec<Item> = emails
            .iter()
            .filter_map(|email| {
                match self.email_to_item(email, &temp_mailbox) {
                    Ok(item) => Some(item),
                    Err(e) => {
                        error!("Failed to convert email to item: {}", e);
                        None
                    }
                }
            })
            .collect();

        Ok(items)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get an appropriate icon/emoji for a folder based on its name.
fn get_folder_icon(folder_name: &str) -> Option<String> {
    let name_lower = folder_name.to_lowercase();

    if name_lower.contains("inbox") {
        Some("📥".to_string())
    } else if name_lower.contains("sent") {
        Some("📤".to_string())
    } else if name_lower.contains("draft") {
        Some("📝".to_string())
    } else if name_lower.contains("trash") || name_lower.contains("deleted") {
        Some("🗑️".to_string())
    } else if name_lower.contains("spam") || name_lower.contains("junk") {
        Some("🚫".to_string())
    } else if name_lower.contains("archive") {
        Some("📦".to_string())
    } else if name_lower.contains("important") || name_lower.contains("starred") {
        Some("⭐".to_string())
    } else {
        Some("📁".to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imap_provider_creation() {
        let config = ImapConfig {
            server: "imap.example.com".to_string(),
            port: 993,
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            use_tls: true,
            mailboxes: vec![ImapMailboxConfig {
                id: "inbox".to_string(),
                name: "Inbox".to_string(),
                mailbox_name: "INBOX".to_string(),
                description: Some("Main inbox".to_string()),
                icon: Some("📥".to_string()),
            }],
        };

        let provider = ImapProvider::new(config);

        assert_eq!(provider.id(), "email-imap");
        assert_eq!(provider.name(), "IMAP Email");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_collections);
        assert!(!caps.has_saved_items);
    }

    #[test]
    fn test_default_config() {
        let config = ImapConfig::default();
        assert_eq!(config.port, 993);
        assert!(config.use_tls);
        assert!(config.mailboxes.is_empty());
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let config = ImapConfig {
            server: "imap.example.com".to_string(),
            port: 993,
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            use_tls: true,
            mailboxes: vec![
                ImapMailboxConfig {
                    id: "inbox".to_string(),
                    name: "Inbox".to_string(),
                    mailbox_name: "INBOX".to_string(),
                    description: Some("Main inbox".to_string()),
                    icon: Some("📥".to_string()),
                },
                ImapMailboxConfig {
                    id: "sent".to_string(),
                    name: "Sent".to_string(),
                    mailbox_name: "Sent".to_string(),
                    description: None,
                    icon: Some("📤".to_string()),
                },
            ],
        };

        let provider = ImapProvider::new(config);
        let feeds = provider.list_feeds().await.unwrap();

        assert_eq!(feeds.len(), 2);
        assert_eq!(feeds[0].id.0, "inbox");
        assert_eq!(feeds[0].name, "Inbox");
        assert_eq!(feeds[1].id.0, "sent");
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("Hello", 10), "Hello");
        assert_eq!(truncate_text("Hello, World!", 5), "Hello...");
        assert_eq!(truncate_text("Test", 4), "Test");
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(
            strip_html_tags("<div><strong>Bold</strong> text</div>"),
            "Bold text"
        );
        assert_eq!(strip_html_tags("No tags here"), "No tags here");
    }

    #[test]
    fn test_get_folder_icon() {
        assert_eq!(get_folder_icon("INBOX"), Some("📥".to_string()));
        assert_eq!(get_folder_icon("Sent"), Some("📤".to_string()));
        assert_eq!(get_folder_icon("Drafts"), Some("📝".to_string()));
        assert_eq!(get_folder_icon("Trash"), Some("🗑️".to_string()));
        assert_eq!(get_folder_icon("Spam"), Some("🚫".to_string()));
        assert_eq!(get_folder_icon("Archive"), Some("📦".to_string()));
        assert_eq!(get_folder_icon("Important"), Some("⭐".to_string()));
        assert_eq!(get_folder_icon("Custom Folder"), Some("📁".to_string()));
    }

    #[tokio::test]
    async fn test_available_actions() {
        let provider = ImapProvider::new(ImapConfig::default());
        let item = Item {
            id: ItemId::new("email-imap", "test"),
            stream_id: StreamId::new("email-imap", "mailbox", "inbox"),
            title: "Test Email".to_string(),
            content: ItemContent::Email {
                subject: "Test".to_string(),
                body_text: Some("Body".to_string()),
                body_html: None,
                snippet: "Body".to_string(),
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Preview, Mark Read, Mark Unread, Archive, Delete
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0].kind, ActionKind::Preview);
        assert_eq!(actions[1].kind, ActionKind::MarkRead);
        assert_eq!(actions[2].kind, ActionKind::MarkUnread);
        assert_eq!(actions[3].kind, ActionKind::Archive);
        assert_eq!(actions[4].kind, ActionKind::Delete);
    }

    #[tokio::test]
    async fn test_execute_action() {
        let provider = ImapProvider::new(ImapConfig::default());
        let item = Item {
            id: ItemId::new("email-imap", "test"),
            stream_id: StreamId::new("email-imap", "mailbox", "inbox"),
            title: "Test Email".to_string(),
            content: ItemContent::Email {
                subject: "Test".to_string(),
                body_text: Some("Body".to_string()),
                body_html: None,
                snippet: "Body".to_string(),
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let action = Action {
            id: "mark_read".to_string(),
            name: "Mark as Read".to_string(),
            description: "Mark email as read".to_string(),
            kind: ActionKind::MarkRead,
            keyboard_shortcut: Some("r".to_string()),
        };

        let result = provider.execute_action(&item, &action).await.unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_email_parsing() {
        // This would require a more complex test setup with a real email message
        // For now, we'll just verify that the mail-parser crate is available
        let parser = MessageParser::default();
        let sample_email = b"From: sender@example.com\r\n\
                            To: recipient@example.com\r\n\
                            Subject: Test\r\n\
                            \r\n\
                            This is a test email.";

        let message = parser.parse(sample_email);
        assert!(message.is_some());

        let message = message.unwrap();
        assert_eq!(message.subject(), Some("Test"));
    }
}

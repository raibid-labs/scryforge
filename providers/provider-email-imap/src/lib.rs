//! # provider-email-imap
//!
//! IMAP email provider for Scryforge.
//!
//! This provider connects to IMAP email servers to fetch emails and folders.
//! It implements the `Provider` and `HasFeeds` traits, where each IMAP folder
//! (INBOX, Sent, etc.) is represented as a feed.
//!
//! ## Authentication
//!
//! Passwords are fetched via the `TokenFetcher` trait from sigilforge. The provider
//! expects the password to be stored with the provider ID "email-imap" and the
//! account name as the alias.
//!
//! ## Configuration
//!
//! ```rust
//! use provider_email_imap::{ImapProvider, ImapConfig};
//! use scryforge_provider_core::auth::MockTokenFetcher;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! let config = ImapConfig {
//!     server: "imap.gmail.com".to_string(),
//!     port: 993,
//!     username: "user@gmail.com".to_string(),
//!     account_name: "personal".to_string(),
//!     use_tls: true,
//! };
//!
//! let mut tokens = HashMap::new();
//! tokens.insert(
//!     ("email-imap".to_string(), "personal".to_string()),
//!     "password123".to_string(),
//! );
//! let token_fetcher = Arc::new(MockTokenFetcher::new(tokens));
//! let provider = ImapProvider::new(config, token_fetcher);
//! ```

use async_imap::Session;
use async_native_tls::{TlsConnector, TlsStream};
use async_std::net::TcpStream;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use mailparse::{parse_mail, MailHeaderMap};
use scryforge_provider_core::auth::TokenFetcher;
use scryforge_provider_core::prelude::*;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the IMAP provider.
#[derive(Debug, Clone)]
pub struct ImapConfig {
    /// IMAP server hostname (e.g., "imap.gmail.com")
    pub server: String,
    /// IMAP server port (typically 993 for TLS, 143 for non-TLS)
    pub port: u16,
    /// Username/email for authentication
    pub username: String,
    /// Account name for credential lookup in sigilforge
    pub account_name: String,
    /// Whether to use TLS (recommended: true)
    pub use_tls: bool,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// IMAP email provider.
pub struct ImapProvider {
    config: ImapConfig,
    token_fetcher: Arc<dyn TokenFetcher>,
}

impl ImapProvider {
    /// Create a new IMAP provider instance.
    pub fn new(config: ImapConfig, token_fetcher: Arc<dyn TokenFetcher>) -> Self {
        Self {
            config,
            token_fetcher,
        }
    }

    /// Connect to the IMAP server and authenticate.
    async fn connect(&self) -> Result<Session<TlsStream<TcpStream>>> {
        // Fetch password from sigilforge
        let password = self
            .token_fetcher
            .fetch_token("email-imap", &self.config.account_name)
            .await
            .map_err(|e| StreamError::AuthRequired(format!("Failed to fetch password: {}", e)))?;

        // Connect to server
        let addr = format!("{}:{}", self.config.server, self.config.port);
        let tcp_stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| StreamError::Network(format!("Failed to connect to {}: {}", addr, e)))?;

        // Wrap in TLS if configured
        let tls_stream = if self.config.use_tls {
            let connector = TlsConnector::new();
            connector
                .connect(&self.config.server, tcp_stream)
                .await
                .map_err(|e| StreamError::Network(format!("TLS connection failed: {}", e)))?
        } else {
            return Err(StreamError::Provider(
                "Non-TLS connections are not currently supported".to_string(),
            ));
        };

        // Create IMAP client
        let client = async_imap::Client::new(tls_stream);

        // Login
        let session = client
            .login(&self.config.username, &password)
            .await
            .map_err(|e| {
                StreamError::AuthRequired(format!(
                    "IMAP login failed for {}: {}",
                    self.config.username, e.0
                ))
            })?;

        Ok(session)
    }

    /// Convert IMAP mailbox name to a feed.
    fn mailbox_to_feed(&self, name: String, exists: u32, unseen: u32) -> Feed {
        let id = FeedId(format!("imap:{}", name));
        let icon = match name.to_uppercase().as_str() {
            "INBOX" => Some("📥".to_string()),
            "SENT" => Some("📤".to_string()),
            "DRAFTS" => Some("📝".to_string()),
            "TRASH" => Some("🗑️".to_string()),
            "SPAM" | "JUNK" => Some("🚫".to_string()),
            "ARCHIVE" => Some("📦".to_string()),
            _ => Some("📁".to_string()),
        };

        Feed {
            id,
            name,
            description: None,
            icon,
            unread_count: Some(unseen),
            total_count: Some(exists),
        }
    }

    /// Parse an email message into an Item.
    fn parse_email(&self, feed_id: &FeedId, uid: u32, data: &[u8]) -> Result<Item> {
        let parsed = parse_mail(data)
            .map_err(|e| StreamError::Provider(format!("Failed to parse email: {}", e)))?;

        // Extract headers
        let subject = parsed
            .headers
            .get_first_value("Subject")
            .unwrap_or_else(|| "(No Subject)".to_string());

        let from = parsed.headers.get_first_value("From");
        let date_str = parsed.headers.get_first_value("Date");
        let message_id = parsed.headers.get_first_value("Message-ID");

        // Parse date
        let published = date_str.and_then(|d| {
            mailparse::dateparse(&d)
                .ok()
                .and_then(|ts| DateTime::from_timestamp(ts, 0))
        });

        // Extract body parts
        let mut body_text: Option<String> = None;
        let mut body_html: Option<String> = None;

        // Handle multipart messages
        if parsed.subparts.is_empty() {
            // Single part message
            let content_type = parsed.ctype.mimetype.to_lowercase();

            if let Ok(body) = parsed.get_body() {
                if content_type.contains("html") {
                    body_html = Some(body);
                } else {
                    body_text = Some(body);
                }
            }
        } else {
            // Multipart message - extract text and HTML alternatives
            for part in &parsed.subparts {
                let content_type = part.ctype.mimetype.to_lowercase();

                if let Ok(body) = part.get_body() {
                    if content_type.contains("html") && body_html.is_none() {
                        body_html = Some(body);
                    } else if content_type.contains("text") && body_text.is_none() {
                        body_text = Some(body);
                    }
                }
            }
        }

        // Create snippet (first 200 chars of text or HTML)
        let snippet = body_text
            .as_ref()
            .or(body_html.as_ref())
            .map(|s| {
                let s = s.trim();
                if s.len() > 200 {
                    format!("{}...", &s[..200])
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "(No content)".to_string());

        // Parse author from "From" header
        let author = from.map(|from_str| {
            // Simple parsing: "Name <email@example.com>" or "email@example.com"
            let (name, email) = if from_str.contains('<') {
                let parts: Vec<&str> = from_str.splitn(2, '<').collect();
                let name = parts[0].trim().trim_matches('"').to_string();
                let email = parts
                    .get(1)
                    .map(|e| e.trim_end_matches('>').trim().to_string());
                (name, email)
            } else {
                (from_str.clone(), Some(from_str))
            };

            Author {
                name,
                email,
                url: None,
                avatar_url: None,
            }
        });

        // Create item ID using message ID if available, otherwise use UID
        let item_local_id = message_id.unwrap_or_else(|| format!("uid-{}", uid));
        let item_id = ItemId::new("email-imap", &item_local_id);
        let stream_id = StreamId::new("email-imap", "feed", &feed_id.0);

        Ok(Item {
            id: item_id,
            stream_id,
            title: subject.clone(),
            content: ItemContent::Email {
                subject,
                body_text,
                body_html,
                snippet,
            },
            author,
            published,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false, // TODO: Check IMAP flags for \Seen
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for ImapProvider {
    fn id(&self) -> &'static str {
        "email-imap"
    }

    fn name(&self) -> &'static str {
        "IMAP Email"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        match self.connect().await {
            Ok(mut session) => {
                // Logout cleanly
                let _ = session.logout().await;

                Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some(format!(
                        "Connected to {} as {}",
                        self.config.server, self.config.username
                    )),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                })
            }
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Connection failed: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        // For IMAP, sync is just checking that we can connect
        match self.connect().await {
            Ok(mut session) => {
                let _ = session.logout().await;
                Ok(SyncResult {
                    success: true,
                    items_added: 0,
                    items_updated: 0,
                    items_removed: 0,
                    errors: vec![],
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            Err(e) => Ok(SyncResult {
                success: false,
                items_added: 0,
                items_updated: 0,
                items_removed: 0,
                errors: vec![format!("Sync failed: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: true,
            has_collections: false,
            has_saved_items: false,
            has_communities: false,
        }
    }

    async fn available_actions(&self, _item: &Item) -> Result<Vec<Action>> {
        Ok(vec![
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
        ])
    }

    async fn execute_action(&self, _item: &Item, action: &Action) -> Result<ActionResult> {
        // TODO: Implement actual IMAP operations (STORE flags, MOVE, etc.)
        Ok(ActionResult {
            success: false,
            message: Some(format!(
                "Action '{}' not yet implemented for IMAP provider",
                action.name
            )),
            data: None,
        })
    }
}

#[async_trait]
impl HasFeeds for ImapProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        let mut session = self.connect().await?;

        // List all mailboxes and collect names first (to release the borrow on session)
        let mut mailbox_stream = session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| StreamError::Provider(format!("Failed to list mailboxes: {}", e)))?;

        let mut mailbox_names = Vec::new();
        while let Some(mailbox_result) = mailbox_stream.next().await {
            match mailbox_result {
                Ok(mailbox) => {
                    mailbox_names.push(mailbox.name().to_string());
                }
                Err(e) => {
                    eprintln!("Failed to list mailbox: {}", e);
                }
            }
        }

        // Drop the stream to release the borrow on session
        drop(mailbox_stream);

        let mut feeds = Vec::new();

        // Now we can use session again to select each mailbox
        for name in mailbox_names {
            match session.select(&name).await {
                Ok(mailbox_status) => {
                    let exists = mailbox_status.exists;
                    let unseen = mailbox_status.unseen.unwrap_or(0);
                    feeds.push(self.mailbox_to_feed(name, exists, unseen));
                }
                Err(e) => {
                    // Log error but continue with other mailboxes
                    eprintln!("Failed to select mailbox '{}': {}", name, e);
                }
            }
        }

        // Logout
        session
            .logout()
            .await
            .map_err(|e| StreamError::Provider(format!("Failed to logout: {}", e)))?;

        Ok(feeds)
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let mut session = self.connect().await?;

        // Extract mailbox name from feed_id (format: "imap:INBOX")
        let mailbox_name = feed_id.0.strip_prefix("imap:").ok_or_else(|| {
            StreamError::StreamNotFound(format!("Invalid feed ID: {}", feed_id.0))
        })?;

        // Select the mailbox
        session
            .select(mailbox_name)
            .await
            .map_err(|e| StreamError::StreamNotFound(format!("Mailbox not found: {}", e)))?;

        // Build search criteria
        let search_query = if options.include_read {
            "ALL"
        } else {
            "UNSEEN"
        };

        // Search for messages
        let message_uids = session
            .uid_search(search_query)
            .await
            .map_err(|e| StreamError::Provider(format!("Search failed: {}", e)))?;

        // Convert HashSet to sorted Vec for consistent ordering
        let mut uids_vec: Vec<u32> = message_uids.into_iter().collect();
        uids_vec.sort_unstable();

        // Apply limit and offset
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.unwrap_or(50) as usize; // Default limit of 50

        let uids_to_fetch: Vec<u32> = uids_vec
            .into_iter()
            .rev() // Most recent first
            .skip(offset)
            .take(limit)
            .collect();

        let mut items = Vec::new();

        // Fetch messages
        for uid in uids_to_fetch {
            let mut fetch_stream = session
                .uid_fetch(uid.to_string(), "RFC822")
                .await
                .map_err(|e| StreamError::Provider(format!("Fetch failed: {}", e)))?;

            // Process the fetch stream
            while let Some(fetch_result) = fetch_stream.next().await {
                match fetch_result {
                    Ok(msg) => {
                        if let Some(body) = msg.body() {
                            match self.parse_email(feed_id, uid, body) {
                                Ok(item) => items.push(item),
                                Err(e) => {
                                    eprintln!("Failed to parse email UID {}: {}", uid, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch message UID {}: {}", uid, e);
                    }
                }
            }
        }

        // Logout
        session
            .logout()
            .await
            .map_err(|e| StreamError::Provider(format!("Failed to logout: {}", e)))?;

        Ok(items)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::auth::MockTokenFetcher;

    fn create_test_config() -> ImapConfig {
        ImapConfig {
            server: "imap.example.com".to_string(),
            port: 993,
            username: "test@example.com".to_string(),
            account_name: "test-account".to_string(),
            use_tls: true,
        }
    }

    fn create_test_token_fetcher() -> Arc<dyn TokenFetcher> {
        let mut tokens = HashMap::new();
        tokens.insert(
            ("email-imap".to_string(), "test-account".to_string()),
            "test-password".to_string(),
        );
        Arc::new(MockTokenFetcher::new(tokens))
    }

    #[test]
    fn test_provider_metadata() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        assert_eq!(provider.id(), "email-imap");
        assert_eq!(provider.name(), "IMAP Email");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(!caps.has_collections);
        assert!(!caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[test]
    fn test_mailbox_to_feed() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        let feed = provider.mailbox_to_feed("INBOX".to_string(), 100, 5);
        assert_eq!(feed.id.0, "imap:INBOX");
        assert_eq!(feed.name, "INBOX");
        assert_eq!(feed.icon, Some("📥".to_string()));
        assert_eq!(feed.unread_count, Some(5));
        assert_eq!(feed.total_count, Some(100));
    }

    #[test]
    fn test_parse_simple_email() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        let email_data = b"From: sender@example.com\r\n\
                          To: recipient@example.com\r\n\
                          Subject: Test Email\r\n\
                          Date: Mon, 1 Jan 2024 12:00:00 +0000\r\n\
                          Message-ID: <test@example.com>\r\n\
                          Content-Type: text/plain\r\n\
                          \r\n\
                          This is a test email body.";

        let feed_id = FeedId("imap:INBOX".to_string());
        let item = provider.parse_email(&feed_id, 123, email_data).unwrap();

        assert_eq!(item.title, "Test Email");
        assert_eq!(item.id.0, "email-imap:<test@example.com>");

        if let ItemContent::Email {
            subject,
            body_text,
            body_html,
            snippet,
        } = item.content
        {
            assert_eq!(subject, "Test Email");
            assert_eq!(body_text, Some("This is a test email body.".to_string()));
            assert_eq!(body_html, None);
            assert_eq!(snippet, "This is a test email body.");
        } else {
            panic!("Expected Email content");
        }

        assert!(item.author.is_some());
        let author = item.author.unwrap();
        assert_eq!(author.email, Some("sender@example.com".to_string()));
    }

    #[test]
    fn test_parse_multipart_email() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        let email_data = b"From: sender@example.com\r\n\
                          Subject: Multipart Test\r\n\
                          Date: Mon, 1 Jan 2024 12:00:00 +0000\r\n\
                          MIME-Version: 1.0\r\n\
                          Content-Type: multipart/alternative; boundary=\"boundary\"\r\n\
                          \r\n\
                          --boundary\r\n\
                          Content-Type: text/plain\r\n\
                          \r\n\
                          Plain text version\r\n\
                          --boundary\r\n\
                          Content-Type: text/html\r\n\
                          \r\n\
                          <html><body>HTML version</body></html>\r\n\
                          --boundary--";

        let feed_id = FeedId("imap:INBOX".to_string());
        let item = provider.parse_email(&feed_id, 456, email_data).unwrap();

        if let ItemContent::Email {
            body_text,
            body_html,
            ..
        } = item.content
        {
            // mailparse may include trailing whitespace, so trim for comparison
            assert_eq!(
                body_text.map(|s| s.trim().to_string()),
                Some("Plain text version".to_string())
            );
            assert_eq!(
                body_html.map(|s| s.trim().to_string()),
                Some("<html><body>HTML version</body></html>".to_string())
            );
        } else {
            panic!("Expected Email content");
        }
    }

    #[tokio::test]
    async fn test_available_actions() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        let item = Item {
            id: ItemId::new("email-imap", "test"),
            stream_id: StreamId::new("email-imap", "feed", "test"),
            title: "Test".to_string(),
            content: ItemContent::Email {
                subject: "Test".to_string(),
                body_text: Some("Test body".to_string()),
                body_html: None,
                snippet: "Test".to_string(),
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
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0].kind, ActionKind::Preview);
        assert_eq!(actions[1].kind, ActionKind::MarkRead);
        assert_eq!(actions[2].kind, ActionKind::MarkUnread);
        assert_eq!(actions[3].kind, ActionKind::Archive);
    }

    #[test]
    fn test_feed_icon_mapping() {
        let config = create_test_config();
        let token_fetcher = create_test_token_fetcher();
        let provider = ImapProvider::new(config, token_fetcher);

        let inbox = provider.mailbox_to_feed("INBOX".to_string(), 0, 0);
        assert_eq!(inbox.icon, Some("📥".to_string()));

        let sent = provider.mailbox_to_feed("Sent".to_string(), 0, 0);
        assert_eq!(sent.icon, Some("📤".to_string()));

        let drafts = provider.mailbox_to_feed("Drafts".to_string(), 0, 0);
        assert_eq!(drafts.icon, Some("📝".to_string()));

        let trash = provider.mailbox_to_feed("Trash".to_string(), 0, 0);
        assert_eq!(trash.icon, Some("🗑️".to_string()));

        let custom = provider.mailbox_to_feed("CustomFolder".to_string(), 0, 0);
        assert_eq!(custom.icon, Some("📁".to_string()));
    }
}

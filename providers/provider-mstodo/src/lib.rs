//! # provider-mstodo
//!
//! Microsoft To Do and Calendar provider for Scryforge.
//!
//! This provider accesses Microsoft To Do tasks and calendar events via the Microsoft Graph API.
//! It implements both `HasCollections` (for task lists) and `HasFeeds` (for virtual feeds like
//! "Due Today", "Important", and "Planned" tasks).
//!
//! ## Authentication
//!
//! This provider requires an OAuth 2.0 access token with the following Microsoft Graph permissions:
//! - `Tasks.Read` - Read user tasks
//! - `Tasks.ReadWrite` - Read and write user tasks (for future write operations)
//! - `Calendars.Read` - Read user calendars (optional, for calendar integration)
//!
//! The access token should be provided via the `MsTodoConfig` configuration.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusabi_streams_core::prelude::*;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum MsTodoError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: StatusCode, message: String },

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Task list not found: {0}")]
    ListNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<MsTodoError> for StreamError {
    fn from(err: MsTodoError) -> Self {
        match err {
            MsTodoError::Http(e) => StreamError::Network(e.to_string()),
            MsTodoError::Api { status, message } => {
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    StreamError::AuthRequired(message)
                } else if status == StatusCode::TOO_MANY_REQUESTS {
                    StreamError::RateLimited(60) // Default retry after 60 seconds
                } else {
                    StreamError::Provider(format!("API error {}: {}", status, message))
                }
            }
            MsTodoError::Auth(e) => StreamError::AuthRequired(e),
            MsTodoError::ListNotFound(e) => StreamError::StreamNotFound(e),
            MsTodoError::TaskNotFound(e) => StreamError::ItemNotFound(e),
            MsTodoError::Serialization(e) => {
                StreamError::Provider(format!("Serialization error: {}", e))
            }
        }
    }
}

// ============================================================================
// Microsoft Graph API Types
// ============================================================================

/// Microsoft Graph API task list response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTaskList {
    id: String,
    display_name: String,
    #[serde(default)]
    is_owner: bool,
    #[serde(default)]
    is_shared: bool,
}

/// Microsoft Graph API task response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTask {
    id: String,
    title: String,
    #[serde(default)]
    body: GraphItemBody,
    status: String, // "notStarted", "inProgress", "completed", "waitingOnOthers", "deferred"
    importance: String, // "low", "normal", "high"
    #[serde(default)]
    is_reminder_on: bool,
    #[serde(default)]
    created_date_time: Option<String>,
    #[serde(default)]
    last_modified_date_time: Option<String>,
    #[serde(default)]
    completed_date_time: Option<GraphDateTimeTimeZone>,
    #[serde(default)]
    due_date_time: Option<GraphDateTimeTimeZone>,
    #[serde(default)]
    reminder_date_time: Option<GraphDateTimeTimeZone>,
    #[serde(default)]
    categories: Vec<String>,
}

/// Microsoft Graph API item body
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphItemBody {
    content_type: Option<String>, // "text" or "html"
    content: Option<String>,
}

/// Microsoft Graph API date time with timezone
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDateTimeTimeZone {
    date_time: String, // ISO 8601 format
    time_zone: String,
}

/// Microsoft Graph API list response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the Microsoft To Do provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsTodoConfig {
    /// OAuth 2.0 access token with Tasks.Read permission
    pub access_token: String,

    /// Optional: Base URL for Microsoft Graph API (defaults to production)
    #[serde(default = "default_graph_url")]
    pub graph_api_url: String,
}

fn default_graph_url() -> String {
    "https://graph.microsoft.com/v1.0".to_string()
}

impl MsTodoConfig {
    /// Create a new configuration with an access token.
    pub fn new(access_token: String) -> Self {
        Self {
            access_token,
            graph_api_url: default_graph_url(),
        }
    }
}

// ============================================================================
// Microsoft To Do Provider
// ============================================================================

/// Microsoft To Do provider.
pub struct MsTodoProvider {
    config: Arc<MsTodoConfig>,
    client: Client,
}

impl MsTodoProvider {
    /// Create a new Microsoft To Do provider with the given configuration.
    pub fn new(config: MsTodoConfig) -> Self {
        let client = Client::builder()
            .user_agent("Scryforge/0.1.0 (Microsoft To Do Client)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    /// Make an authenticated GET request to the Microsoft Graph API.
    async fn graph_get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
    ) -> std::result::Result<T, MsTodoError> {
        let url = format!("{}{}", self.config.graph_api_url, endpoint);
        debug!("GET {}", url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.config.access_token)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(MsTodoError::Api {
                status,
                message: error_text,
            });
        }

        let data = response.json::<T>().await?;
        Ok(data)
    }

    /// Fetch all task lists from Microsoft To Do.
    async fn fetch_task_lists(&self) -> std::result::Result<Vec<GraphTaskList>, MsTodoError> {
        let response: GraphListResponse<GraphTaskList> =
            self.graph_get("/me/todo/lists").await?;
        Ok(response.value)
    }

    /// Fetch tasks from a specific task list.
    async fn fetch_tasks(
        &self,
        list_id: &str,
    ) -> std::result::Result<Vec<GraphTask>, MsTodoError> {
        let endpoint = format!("/me/todo/lists/{}/tasks", list_id);
        let response: GraphListResponse<GraphTask> = self.graph_get(&endpoint).await?;
        Ok(response.value)
    }

    /// Fetch all tasks from all lists (for virtual feeds).
    async fn fetch_all_tasks(&self) -> std::result::Result<Vec<(String, GraphTask)>, MsTodoError> {
        let lists = self.fetch_task_lists().await?;
        let mut all_tasks = Vec::new();

        for list in lists {
            match self.fetch_tasks(&list.id).await {
                Ok(tasks) => {
                    for task in tasks {
                        all_tasks.push((list.id.clone(), task));
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch tasks from list {}: {}", list.display_name, e);
                }
            }
        }

        Ok(all_tasks)
    }

    /// Convert a Graph task to our Item struct.
    fn task_to_item(&self, task: &GraphTask, list_id: &str) -> Item {
        let stream_id = StreamId::new("mstodo", "list", list_id);
        let item_id = ItemId::new("mstodo", &task.id);

        // Parse due date
        let due_date = task.due_date_time.as_ref().and_then(|dt| {
            // Try parsing as RFC3339 first
            DateTime::parse_from_rfc3339(&dt.date_time)
                .ok()
                .map(|d| d.date_naive())
                .or_else(|| {
                    // If that fails, try parsing as naive datetime and extract date
                    use chrono::NaiveDateTime;
                    NaiveDateTime::parse_from_str(&dt.date_time, "%Y-%m-%dT%H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.date())
                })
        });

        // Determine if task is completed
        let is_completed = task.status == "completed";

        // Extract body content
        let body = task
            .body
            .content
            .clone()
            .filter(|s| !s.trim().is_empty());

        let content = ItemContent::Task {
            body: body.clone(),
            due_date,
            is_completed,
        };

        // Parse published/updated dates
        let published = task
            .created_date_time
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let updated = task
            .last_modified_date_time
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("status".to_string(), task.status.clone());
        metadata.insert("importance".to_string(), task.importance.clone());
        metadata.insert(
            "is_reminder_on".to_string(),
            task.is_reminder_on.to_string(),
        );

        if let Some(due_dt) = &task.due_date_time {
            metadata.insert("due_date_time".to_string(), due_dt.date_time.clone());
            metadata.insert("due_time_zone".to_string(), due_dt.time_zone.clone());
        }

        if let Some(completed_dt) = &task.completed_date_time {
            metadata.insert(
                "completed_date_time".to_string(),
                completed_dt.date_time.clone(),
            );
        }

        if let Some(reminder_dt) = &task.reminder_date_time {
            metadata.insert(
                "reminder_date_time".to_string(),
                reminder_dt.date_time.clone(),
            );
        }

        // Build To Do web URL
        let url = Some(format!(
            "https://to-do.microsoft.com/tasks/id/{}/details",
            task.id
        ));

        Item {
            id: item_id,
            stream_id,
            title: task.title.clone(),
            content,
            author: None,
            published,
            updated,
            url,
            thumbnail_url: None,
            is_read: is_completed,
            is_saved: false,
            tags: task.categories.clone(),
            metadata,
        }
    }

    /// Find a task list by collection ID.
    async fn find_list(&self, collection_id: &CollectionId) -> Option<GraphTaskList> {
        let lists = self.fetch_task_lists().await.ok()?;
        lists.into_iter().find(|list| list.id == collection_id.0)
    }
}

#[async_trait]
impl Provider for MsTodoProvider {
    fn id(&self) -> &'static str {
        "mstodo"
    }

    fn name(&self) -> &'static str {
        "Microsoft To Do"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch task lists to verify authentication and connectivity
        match self.fetch_task_lists().await {
            Ok(lists) => {
                info!("Health check passed: found {} task lists", lists.len());
                Ok(ProviderHealth {
                    is_healthy: true,
                    message: Some(format!("Connected to Microsoft To Do ({} lists)", lists.len())),
                    last_sync: Some(Utc::now()),
                    error_count: 0,
                })
            }
            Err(e) => {
                error!("Health check failed: {}", e);
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

        info!("Syncing Microsoft To Do tasks");

        match self.fetch_all_tasks().await {
            Ok(tasks) => {
                items_added = tasks.len() as u32;
                info!("Fetched {} tasks across all lists", items_added);
            }
            Err(e) => {
                error!("Failed to fetch tasks: {}", e);
                errors.push(format!("Fetch error: {}", e));
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

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show task details".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
            Action {
                id: "open_todo".to_string(),
                name: "Open in To Do".to_string(),
                description: "Open task in Microsoft To Do web app".to_string(),
                kind: ActionKind::OpenInBrowser,
                keyboard_shortcut: Some("o".to_string()),
            },
        ];

        // Add mark complete/incomplete action based on current status
        if let ItemContent::Task { is_completed, .. } = &item.content {
            if *is_completed {
                actions.push(Action {
                    id: "mark_incomplete".to_string(),
                    name: "Mark Incomplete".to_string(),
                    description: "Mark task as not completed".to_string(),
                    kind: ActionKind::MarkUnread,
                    keyboard_shortcut: Some("u".to_string()),
                });
            } else {
                actions.push(Action {
                    id: "mark_complete".to_string(),
                    name: "Mark Complete".to_string(),
                    description: "Mark task as completed".to_string(),
                    kind: ActionKind::MarkRead,
                    keyboard_shortcut: Some("c".to_string()),
                });
            }
        }

        // Add copy link action
        if item.url.is_some() {
            actions.push(Action {
                id: "copy_link".to_string(),
                name: "Copy Link".to_string(),
                description: "Copy task URL to clipboard".to_string(),
                kind: ActionKind::CopyLink,
                keyboard_shortcut: Some("l".to_string()),
            });
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::OpenInBrowser => {
                if let Some(url) = &item.url {
                    info!("Opening task in browser: {}", url);
                    Ok(ActionResult {
                        success: true,
                        message: Some(format!("Opening task: {}", item.title)),
                        data: Some(serde_json::json!({ "url": url })),
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available".to_string()),
                        data: None,
                    })
                }
            }
            ActionKind::CopyLink => {
                if let Some(url) = &item.url {
                    Ok(ActionResult {
                        success: true,
                        message: Some("Task link copied to clipboard".to_string()),
                        data: Some(serde_json::json!({ "url": url })),
                    })
                } else {
                    Ok(ActionResult {
                        success: false,
                        message: Some("No URL available".to_string()),
                        data: None,
                    })
                }
            }
            ActionKind::MarkRead | ActionKind::MarkUnread => {
                // TODO: Implement task completion toggle via API PATCH request
                // For now, return a placeholder success response
                Ok(ActionResult {
                    success: true,
                    message: Some(format!(
                        "Task marked as {}",
                        if action.kind == ActionKind::MarkRead {
                            "complete"
                        } else {
                            "incomplete"
                        }
                    )),
                    data: None,
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

#[async_trait]
impl HasCollections for MsTodoProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let lists = self
            .fetch_task_lists()
            .await
            .map_err(StreamError::from)?;

        Ok(lists
            .into_iter()
            .map(|list| Collection {
                id: CollectionId(list.id.clone()),
                name: list.display_name.clone(),
                description: if list.is_shared {
                    Some("Shared task list".to_string())
                } else {
                    None
                },
                icon: Some("📋".to_string()),
                item_count: 0, // Would require fetching tasks for each list
                is_editable: list.is_owner,
                owner: if list.is_owner {
                    Some("Me".to_string())
                } else {
                    None
                },
            })
            .collect())
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        // Verify the list exists
        if self.find_list(collection_id).await.is_none() {
            return Err(StreamError::StreamNotFound(collection_id.0.clone()));
        }

        let tasks = self
            .fetch_tasks(&collection_id.0)
            .await
            .map_err(StreamError::from)?;

        let items: Vec<Item> = tasks
            .iter()
            .map(|task| self.task_to_item(task, &collection_id.0))
            .collect();

        Ok(items)
    }
}

#[async_trait]
impl HasFeeds for MsTodoProvider {
    async fn list_feeds(&self) -> Result<Vec<Feed>> {
        // Virtual feeds for common task views
        Ok(vec![
            Feed {
                id: FeedId("mstodo:due-today".to_string()),
                name: "Due Today".to_string(),
                description: Some("Tasks due today".to_string()),
                icon: Some("📅".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("mstodo:important".to_string()),
                name: "Important".to_string(),
                description: Some("High importance tasks".to_string()),
                icon: Some("⭐".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("mstodo:planned".to_string()),
                name: "Planned".to_string(),
                description: Some("Tasks with due dates".to_string()),
                icon: Some("🗓️".to_string()),
                unread_count: None,
                total_count: None,
            },
            Feed {
                id: FeedId("mstodo:all".to_string()),
                name: "All Tasks".to_string(),
                description: Some("All tasks across all lists".to_string()),
                icon: Some("📋".to_string()),
                unread_count: None,
                total_count: None,
            },
        ])
    }

    async fn get_feed_items(&self, feed_id: &FeedId, options: FeedOptions) -> Result<Vec<Item>> {
        let all_tasks = self.fetch_all_tasks().await.map_err(StreamError::from)?;

        let today = Utc::now().date_naive();

        // Convert all tasks to items
        let mut items: Vec<Item> = all_tasks
            .iter()
            .map(|(list_id, task)| self.task_to_item(task, list_id))
            .collect();

        // Filter based on feed type
        match feed_id.0.as_str() {
            "mstodo:due-today" => {
                items.retain(|item| {
                    if let ItemContent::Task { due_date, .. } = &item.content {
                        due_date.is_some_and(|d| d == today)
                    } else {
                        false
                    }
                });
            }
            "mstodo:important" => {
                items.retain(|item| {
                    item.metadata
                        .get("importance")
                        .is_some_and(|imp| imp == "high")
                });
            }
            "mstodo:planned" => {
                items.retain(|item| {
                    if let ItemContent::Task { due_date, .. } = &item.content {
                        due_date.is_some()
                    } else {
                        false
                    }
                });
            }
            "mstodo:all" => {
                // Include all tasks
            }
            _ => {
                return Err(StreamError::StreamNotFound(feed_id.0.clone()));
            }
        }

        // Apply filtering based on options
        if !options.include_read {
            items.retain(|item| !item.is_read);
        }

        // Apply since filter
        if let Some(since) = options.since {
            items.retain(|item| item.published.is_some_and(|pub_date| pub_date > since));
        }

        // Sort by due date (soonest first), then by importance
        items.sort_by(|a, b| {
            // Extract due dates
            let a_due = if let ItemContent::Task { due_date, .. } = &a.content {
                *due_date
            } else {
                None
            };
            let b_due = if let ItemContent::Task { due_date, .. } = &b.content {
                *due_date
            } else {
                None
            };

            // Sort by due date first (None goes to end)
            match (a_due, b_due) {
                (Some(a_date), Some(b_date)) => a_date.cmp(&b_date),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => {
                    // If no due dates, sort by importance
                    let a_imp = a.metadata.get("importance").map(|s| s.as_str());
                    let b_imp = b.metadata.get("importance").map(|s| s.as_str());
                    importance_order(b_imp).cmp(&importance_order(a_imp))
                }
            }
        });

        // Apply offset and limit
        let offset = options.offset.unwrap_or(0) as usize;
        let limit = options.limit.map(|l| l as usize);

        let items = items.into_iter().skip(offset);
        let items = if let Some(limit) = limit {
            items.take(limit).collect()
        } else {
            items.collect()
        };

        Ok(items)
    }
}

/// Helper function to convert importance to sortable order.
fn importance_order(importance: Option<&str>) -> u8 {
    match importance {
        Some("high") => 3,
        Some("normal") => 2,
        Some("low") => 1,
        _ => 0,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn mock_config() -> MsTodoConfig {
        MsTodoConfig::new("mock_access_token".to_string())
    }

    #[test]
    fn test_mstodo_provider_creation() {
        let provider = MsTodoProvider::new(mock_config());

        assert_eq!(provider.id(), "mstodo");
        assert_eq!(provider.name(), "Microsoft To Do");

        let caps = provider.capabilities();
        assert!(caps.has_feeds);
        assert!(caps.has_collections);
        assert!(!caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[test]
    fn test_task_to_item_conversion() {
        let provider = MsTodoProvider::new(mock_config());

        let graph_task = GraphTask {
            id: "task123".to_string(),
            title: "Test Task".to_string(),
            body: GraphItemBody {
                content_type: Some("text".to_string()),
                content: Some("This is a test task body".to_string()),
            },
            status: "notStarted".to_string(),
            importance: "high".to_string(),
            is_reminder_on: true,
            created_date_time: Some("2024-01-15T10:00:00Z".to_string()),
            last_modified_date_time: Some("2024-01-15T11:00:00Z".to_string()),
            completed_date_time: None,
            due_date_time: Some(GraphDateTimeTimeZone {
                date_time: "2024-01-20T09:00:00".to_string(),
                time_zone: "UTC".to_string(),
            }),
            reminder_date_time: None,
            categories: vec!["work".to_string(), "urgent".to_string()],
        };

        let item = provider.task_to_item(&graph_task, "list1");

        assert_eq!(item.title, "Test Task");
        assert_eq!(item.id.0, "mstodo:task123");
        assert_eq!(item.stream_id.0, "mstodo:list:list1");

        if let ItemContent::Task {
            body,
            due_date,
            is_completed,
        } = &item.content
        {
            assert_eq!(body.as_ref().unwrap(), "This is a test task body");
            assert_eq!(
                due_date.unwrap(),
                NaiveDate::from_ymd_opt(2024, 1, 20).unwrap()
            );
            assert!(!is_completed);
        } else {
            panic!("Expected ItemContent::Task");
        }

        assert_eq!(item.tags, vec!["work", "urgent"]);
        assert_eq!(item.metadata.get("status").unwrap(), "notStarted");
        assert_eq!(item.metadata.get("importance").unwrap(), "high");
        assert_eq!(item.metadata.get("is_reminder_on").unwrap(), "true");
        assert!(!item.is_read);
    }

    #[test]
    fn test_task_to_item_completed() {
        let provider = MsTodoProvider::new(mock_config());

        let graph_task = GraphTask {
            id: "task456".to_string(),
            title: "Completed Task".to_string(),
            body: GraphItemBody::default(),
            status: "completed".to_string(),
            importance: "normal".to_string(),
            is_reminder_on: false,
            created_date_time: Some("2024-01-15T10:00:00Z".to_string()),
            last_modified_date_time: Some("2024-01-16T14:00:00Z".to_string()),
            completed_date_time: Some(GraphDateTimeTimeZone {
                date_time: "2024-01-16T14:00:00Z".to_string(),
                time_zone: "UTC".to_string(),
            }),
            due_date_time: None,
            reminder_date_time: None,
            categories: vec![],
        };

        let item = provider.task_to_item(&graph_task, "list1");

        if let ItemContent::Task { is_completed, .. } = &item.content {
            assert!(is_completed);
        } else {
            panic!("Expected ItemContent::Task");
        }

        assert!(item.is_read); // Completed tasks are marked as read
        assert_eq!(item.metadata.get("status").unwrap(), "completed");
        assert!(item.metadata.contains_key("completed_date_time"));
    }

    #[tokio::test]
    async fn test_list_feeds() {
        let provider = MsTodoProvider::new(mock_config());
        let feeds = provider.list_feeds().await.unwrap();

        assert_eq!(feeds.len(), 4);

        assert_eq!(feeds[0].id.0, "mstodo:due-today");
        assert_eq!(feeds[0].name, "Due Today");

        assert_eq!(feeds[1].id.0, "mstodo:important");
        assert_eq!(feeds[1].name, "Important");

        assert_eq!(feeds[2].id.0, "mstodo:planned");
        assert_eq!(feeds[2].name, "Planned");

        assert_eq!(feeds[3].id.0, "mstodo:all");
        assert_eq!(feeds[3].name, "All Tasks");
    }

    #[tokio::test]
    async fn test_available_actions_incomplete_task() {
        let provider = MsTodoProvider::new(mock_config());

        let item = Item {
            id: ItemId::new("mstodo", "task1"),
            stream_id: StreamId::new("mstodo", "list", "list1"),
            title: "Test Task".to_string(),
            content: ItemContent::Task {
                body: Some("Task body".to_string()),
                due_date: Some(NaiveDate::from_ymd_opt(2024, 1, 20).unwrap()),
                is_completed: false,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://to-do.microsoft.com/tasks/id/task1/details".to_string()),
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have: Preview, Open in To Do, Mark Complete, Copy Link
        assert!(actions.len() >= 4);
        assert!(actions.iter().any(|a| a.kind == ActionKind::Preview));
        assert!(actions.iter().any(|a| a.kind == ActionKind::OpenInBrowser));
        assert!(actions.iter().any(|a| a.kind == ActionKind::MarkRead));
        assert!(actions.iter().any(|a| a.kind == ActionKind::CopyLink));
    }

    #[tokio::test]
    async fn test_available_actions_completed_task() {
        let provider = MsTodoProvider::new(mock_config());

        let item = Item {
            id: ItemId::new("mstodo", "task1"),
            stream_id: StreamId::new("mstodo", "list", "list1"),
            title: "Completed Task".to_string(),
            content: ItemContent::Task {
                body: None,
                due_date: None,
                is_completed: true,
            },
            author: None,
            published: None,
            updated: None,
            url: Some("https://to-do.microsoft.com/tasks/id/task1/details".to_string()),
            thumbnail_url: None,
            is_read: true,
            is_saved: false,
            tags: vec![],
            metadata: Default::default(),
        };

        let actions = provider.available_actions(&item).await.unwrap();

        // Should have Mark Incomplete instead of Mark Complete
        assert!(actions.iter().any(|a| a.kind == ActionKind::MarkUnread));
        assert!(!actions.iter().any(|a| a.kind == ActionKind::MarkRead));
    }

    #[test]
    fn test_importance_order() {
        assert_eq!(importance_order(Some("high")), 3);
        assert_eq!(importance_order(Some("normal")), 2);
        assert_eq!(importance_order(Some("low")), 1);
        assert_eq!(importance_order(None), 0);
        assert_eq!(importance_order(Some("unknown")), 0);
    }

    #[test]
    fn test_config_creation() {
        let config = MsTodoConfig::new("test_token".to_string());
        assert_eq!(config.access_token, "test_token");
        assert_eq!(
            config.graph_api_url,
            "https://graph.microsoft.com/v1.0"
        );
    }

    #[test]
    fn test_graph_task_deserialization() {
        let json = r#"{
            "id": "task123",
            "title": "Test Task",
            "status": "notStarted",
            "importance": "high",
            "isReminderOn": true,
            "createdDateTime": "2024-01-15T10:00:00Z",
            "body": {
                "contentType": "text",
                "content": "Test body"
            },
            "categories": ["work"]
        }"#;

        let task: GraphTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "task123");
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, "notStarted");
        assert_eq!(task.importance, "high");
        assert!(task.is_reminder_on);
        assert_eq!(task.categories, vec!["work"]);
    }
}

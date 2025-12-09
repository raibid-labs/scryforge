//! # provider-mstodo
//!
//! Microsoft To Do provider for Scryforge.
//!
//! This provider integrates with Microsoft To Do using the Microsoft Graph API
//! to fetch task lists and tasks. It supports:
//! - Listing task lists (collections)
//! - Fetching tasks from lists
//! - Marking tasks as complete/incomplete
//!
//! ## Authentication
//!
//! This provider requires OAuth tokens via the Sigilforge daemon.
//! The service identifier is "mstodo".
//!
//! ## Example
//!
//! ```no_run
//! use provider_mstodo::MsTodoProvider;
//! use scryforge_provider_core::auth::{MockTokenFetcher, TokenFetcher};
//! use scryforge_provider_core::prelude::*;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<()> {
//! let token_fetcher = Arc::new(MockTokenFetcher::empty()
//!     .with_token("mstodo".to_string(), "personal".to_string(), "token123".to_string()));
//! let provider = MsTodoProvider::new(token_fetcher, "personal".to_string());
//!
//! // List all task lists
//! let collections = provider.list_collections().await?;
//! for collection in collections {
//!     println!("Task list: {}", collection.name);
//! }
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use scryforge_provider_core::auth::TokenFetcher;
use scryforge_provider_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Error)]
pub enum MsTodoError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("API request failed: {0}")]
    ApiRequest(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl From<MsTodoError> for StreamError {
    fn from(err: MsTodoError) -> Self {
        match err {
            MsTodoError::Auth(msg) => StreamError::AuthRequired(msg),
            MsTodoError::Http(e) => StreamError::Network(e.to_string()),
            MsTodoError::ApiRequest(msg) => StreamError::Provider(msg),
            MsTodoError::Json(e) => StreamError::Internal(e.to_string()),
            MsTodoError::InvalidResponse(msg) => StreamError::Internal(msg),
        }
    }
}

// ============================================================================
// Microsoft Graph API Response Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
struct TaskList {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "isOwner")]
    is_owner: bool,
    #[serde(rename = "isShared")]
    is_shared: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskListsResponse {
    value: Vec<TaskList>,
}

#[derive(Debug, Clone, Deserialize)]
struct TodoTask {
    id: String,
    title: String,
    #[serde(default)]
    body: TaskBody,
    status: TaskStatus,
    #[serde(rename = "createdDateTime")]
    created_date_time: String,
    #[serde(rename = "lastModifiedDateTime")]
    last_modified_date_time: String,
    #[serde(rename = "dueDateTime")]
    due_date_time: Option<DateTimeTimeZone>,
    #[serde(rename = "completedDateTime")]
    #[allow(dead_code)]
    completed_date_time: Option<DateTimeTimeZone>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TaskBody {
    #[serde(default)]
    content: String,
    #[serde(rename = "contentType", default)]
    #[allow(dead_code)]
    content_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TaskStatus {
    NotStarted,
    InProgress,
    Completed,
    WaitingOnOthers,
    Deferred,
}

#[derive(Debug, Clone, Deserialize)]
struct DateTimeTimeZone {
    #[serde(rename = "dateTime")]
    date_time: String,
    #[serde(rename = "timeZone")]
    #[allow(dead_code)]
    time_zone: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TasksResponse {
    value: Vec<TodoTask>,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateTaskRequest {
    status: String,
}

// ============================================================================
// Provider Implementation
// ============================================================================

/// Microsoft To Do provider.
///
/// Connects to Microsoft Graph API to fetch task lists and tasks.
/// Requires OAuth authentication via Sigilforge.
pub struct MsTodoProvider {
    token_fetcher: Arc<dyn TokenFetcher>,
    account: String,
    client: Client,
    base_url: String,
}

impl MsTodoProvider {
    const SERVICE_ID: &'static str = "mstodo";
    const GRAPH_BASE_URL: &'static str = "https://graph.microsoft.com/v1.0";

    /// Create a new Microsoft To Do provider.
    ///
    /// # Arguments
    ///
    /// * `token_fetcher` - Token fetcher for OAuth authentication
    /// * `account` - Account identifier for token lookup (e.g., "personal", "work")
    pub fn new(token_fetcher: Arc<dyn TokenFetcher>, account: String) -> Self {
        Self {
            token_fetcher,
            account,
            client: Client::new(),
            base_url: Self::GRAPH_BASE_URL.to_string(),
        }
    }

    /// Create a provider with a custom base URL (useful for testing).
    #[cfg(test)]
    pub fn with_base_url(
        token_fetcher: Arc<dyn TokenFetcher>,
        account: String,
        base_url: String,
    ) -> Self {
        Self {
            token_fetcher,
            account,
            client: Client::new(),
            base_url,
        }
    }

    /// Fetch a fresh access token.
    async fn get_token(&self) -> std::result::Result<String, MsTodoError> {
        self.token_fetcher
            .fetch_token(Self::SERVICE_ID, &self.account)
            .await
            .map_err(|e| MsTodoError::Auth(e.to_string()))
    }

    /// Fetch all task lists from Microsoft To Do.
    async fn fetch_task_lists(&self) -> std::result::Result<Vec<TaskList>, MsTodoError> {
        let token = self.get_token().await?;
        let url = format!("{}/me/todo/lists", self.base_url);

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MsTodoError::ApiRequest(format!(
                "Failed to fetch task lists: {} - {}",
                status, body
            )));
        }

        let lists_response: TaskListsResponse = response.json().await?;
        Ok(lists_response.value)
    }

    /// Fetch tasks from a specific task list.
    async fn fetch_tasks(&self, list_id: &str) -> std::result::Result<Vec<TodoTask>, MsTodoError> {
        let token = self.get_token().await?;
        let url = format!("{}/me/todo/lists/{}/tasks", self.base_url, list_id);

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MsTodoError::ApiRequest(format!(
                "Failed to fetch tasks from list {}: {} - {}",
                list_id, status, body
            )));
        }

        let tasks_response: TasksResponse = response.json().await?;
        Ok(tasks_response.value)
    }

    /// Convert a Microsoft To Do task to a Scryforge Item.
    fn task_to_item(&self, task: TodoTask, list_id: &str) -> Item {
        let stream_id = StreamId::new(Self::SERVICE_ID, "collection", list_id);
        let item_id = ItemId::new(Self::SERVICE_ID, &task.id);

        // Parse due date if present
        let due_date = task
            .due_date_time
            .and_then(|dt| NaiveDate::parse_from_str(&dt.date_time[..10], "%Y-%m-%d").ok());

        // Determine if completed
        let is_completed = matches!(task.status, TaskStatus::Completed);

        // Parse timestamps
        let published = DateTime::parse_from_rfc3339(&task.created_date_time)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        let updated = DateTime::parse_from_rfc3339(&task.last_modified_date_time)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));

        // Extract body content
        let body_text = if task.body.content.is_empty() {
            None
        } else {
            Some(task.body.content)
        };

        Item {
            id: item_id,
            stream_id,
            title: task.title,
            content: ItemContent::Task {
                body: body_text,
                due_date,
                is_completed,
            },
            author: None,
            published,
            updated,
            url: None,
            thumbnail_url: None,
            is_read: is_completed,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        }
    }

    /// Update a task's completion status.
    async fn update_task_status(
        &self,
        list_id: &str,
        task_id: &str,
        completed: bool,
    ) -> std::result::Result<(), MsTodoError> {
        let token = self.get_token().await?;
        let url = format!(
            "{}/me/todo/lists/{}/tasks/{}",
            self.base_url, list_id, task_id
        );

        let status = if completed { "completed" } else { "notStarted" };

        let request_body = UpdateTaskRequest {
            status: status.to_string(),
        };

        let response = self
            .client
            .patch(&url)
            .bearer_auth(&token)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MsTodoError::ApiRequest(format!(
                "Failed to update task {}: {} - {}",
                task_id, status, body
            )));
        }

        Ok(())
    }
}

// ============================================================================
// Provider Trait Implementations
// ============================================================================

#[async_trait]
impl Provider for MsTodoProvider {
    fn id(&self) -> &'static str {
        Self::SERVICE_ID
    }

    fn name(&self) -> &'static str {
        "Microsoft To Do"
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Try to fetch token and make a simple API call
        match self.fetch_task_lists().await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                message: Some("Connected to Microsoft To Do".to_string()),
                last_sync: Some(Utc::now()),
                error_count: 0,
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                message: Some(format!("Health check failed: {}", e)),
                last_sync: None,
                error_count: 1,
            }),
        }
    }

    async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();

        match self.fetch_task_lists().await {
            Ok(lists) => {
                let mut total_tasks = 0;
                for list in lists {
                    if let Ok(tasks) = self.fetch_tasks(&list.id).await {
                        total_tasks += tasks.len();
                    }
                }

                Ok(SyncResult {
                    success: true,
                    items_added: total_tasks as u32,
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
                errors: vec![e.to_string()],
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_feeds: false,
            has_collections: true,
            has_saved_items: false,
            has_communities: false,
        }
    }

    async fn available_actions(&self, item: &Item) -> Result<Vec<Action>> {
        let mut actions = vec![
            Action {
                id: "open".to_string(),
                name: "Open".to_string(),
                description: "Open task in browser".to_string(),
                kind: ActionKind::Open,
                keyboard_shortcut: Some("o".to_string()),
            },
            Action {
                id: "preview".to_string(),
                name: "Preview".to_string(),
                description: "Show task details".to_string(),
                kind: ActionKind::Preview,
                keyboard_shortcut: Some("p".to_string()),
            },
        ];

        // Add completion toggle based on current state
        if let ItemContent::Task { is_completed, .. } = item.content {
            if is_completed {
                actions.push(Action {
                    id: "uncomplete".to_string(),
                    name: "Mark as Not Completed".to_string(),
                    description: "Mark task as not completed".to_string(),
                    kind: ActionKind::Custom("uncomplete".to_string()),
                    keyboard_shortcut: Some("u".to_string()),
                });
            } else {
                actions.push(Action {
                    id: "complete".to_string(),
                    name: "Mark as Completed".to_string(),
                    description: "Mark task as completed".to_string(),
                    kind: ActionKind::Custom("complete".to_string()),
                    keyboard_shortcut: Some("c".to_string()),
                });
            }
        }

        Ok(actions)
    }

    async fn execute_action(&self, item: &Item, action: &Action) -> Result<ActionResult> {
        match action.kind {
            ActionKind::Custom(ref custom) if custom == "complete" => {
                // Extract list_id from stream_id and task_id from item_id
                let task_id = item.id.0.strip_prefix("mstodo:").unwrap_or(&item.id.0);
                let list_id = item
                    .stream_id
                    .0
                    .split(':')
                    .nth(2)
                    .ok_or_else(|| StreamError::Internal("Invalid stream ID".to_string()))?;

                self.update_task_status(list_id, task_id, true)
                    .await
                    .map_err(|e| StreamError::Provider(e.to_string()))?;

                Ok(ActionResult {
                    success: true,
                    message: Some("Task marked as completed".to_string()),
                    data: None,
                })
            }
            ActionKind::Custom(ref custom) if custom == "uncomplete" => {
                let task_id = item.id.0.strip_prefix("mstodo:").unwrap_or(&item.id.0);
                let list_id = item
                    .stream_id
                    .0
                    .split(':')
                    .nth(2)
                    .ok_or_else(|| StreamError::Internal("Invalid stream ID".to_string()))?;

                self.update_task_status(list_id, task_id, false)
                    .await
                    .map_err(|e| StreamError::Provider(e.to_string()))?;

                Ok(ActionResult {
                    success: true,
                    message: Some("Task marked as not completed".to_string()),
                    data: None,
                })
            }
            _ => Ok(ActionResult {
                success: true,
                message: Some(format!("Action {} executed", action.name)),
                data: None,
            }),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl HasCollections for MsTodoProvider {
    async fn list_collections(&self) -> Result<Vec<Collection>> {
        let lists = self
            .fetch_task_lists()
            .await
            .map_err(|e| StreamError::Provider(e.to_string()))?;

        let collections = lists
            .into_iter()
            .map(|list| Collection {
                id: CollectionId(list.id.clone()),
                name: list.display_name,
                description: None,
                icon: Some("☑".to_string()),
                item_count: 0, // Would need additional API call to get count
                is_editable: list.is_owner,
                owner: if list.is_shared {
                    Some("Shared".to_string())
                } else {
                    Some("Me".to_string())
                },
            })
            .collect();

        Ok(collections)
    }

    async fn get_collection_items(&self, collection_id: &CollectionId) -> Result<Vec<Item>> {
        let tasks = self
            .fetch_tasks(&collection_id.0)
            .await
            .map_err(|e| StreamError::Provider(e.to_string()))?;

        let items = tasks
            .into_iter()
            .map(|task| self.task_to_item(task, &collection_id.0))
            .collect();

        Ok(items)
    }

    async fn add_to_collection(
        &self,
        _collection_id: &CollectionId,
        _item_id: &ItemId,
    ) -> Result<()> {
        // Microsoft To Do doesn't support moving tasks between lists via this operation.
        // Tasks are created in specific lists and can't be added like playlist items.
        Err(StreamError::Provider(
            "Adding tasks to collections is not supported by Microsoft To Do API".to_string(),
        ))
    }

    async fn remove_from_collection(
        &self,
        _collection_id: &CollectionId,
        _item_id: &ItemId,
    ) -> Result<()> {
        // Removing a task from a list would be equivalent to deleting the task.
        // This operation is not implemented in the current phase.
        Err(StreamError::Provider(
            "Removing tasks from collections is not supported in this implementation".to_string(),
        ))
    }

    async fn create_collection(&self, name: &str) -> Result<Collection> {
        let token = self
            .get_token()
            .await
            .map_err(|e| StreamError::AuthRequired(e.to_string()))?;
        let url = format!("{}/me/todo/lists", self.base_url);

        #[derive(Serialize)]
        struct CreateListRequest {
            #[serde(rename = "displayName")]
            display_name: String,
        }

        let request_body = CreateListRequest {
            display_name: name.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| StreamError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(StreamError::Provider(format!(
                "Failed to create task list: {} - {}",
                status, body
            )));
        }

        let task_list: TaskList = response
            .json()
            .await
            .map_err(|e| StreamError::Internal(e.to_string()))?;

        Ok(Collection {
            id: CollectionId(task_list.id.clone()),
            name: task_list.display_name,
            description: None,
            icon: Some("☑".to_string()),
            item_count: 0,
            is_editable: task_list.is_owner,
            owner: if task_list.is_shared {
                Some("Shared".to_string())
            } else {
                Some("Me".to_string())
            },
        })
    }
}

#[async_trait]
impl HasTasks for MsTodoProvider {
    async fn complete_task(&self, task_id: &str) -> Result<()> {
        // Extract list_id and task_id from the format "list_id/task_id"
        let parts: Vec<&str> = task_id.split('/').collect();
        if parts.len() != 2 {
            return Err(StreamError::Internal(format!(
                "Invalid task_id format: {}",
                task_id
            )));
        }

        let list_id = parts[0];
        let task_id = parts[1];

        self.update_task_status(list_id, task_id, true)
            .await
            .map_err(|e| StreamError::Provider(e.to_string()))
    }

    async fn uncomplete_task(&self, task_id: &str) -> Result<()> {
        // Extract list_id and task_id from the format "list_id/task_id"
        let parts: Vec<&str> = task_id.split('/').collect();
        if parts.len() != 2 {
            return Err(StreamError::Internal(format!(
                "Invalid task_id format: {}",
                task_id
            )));
        }

        let list_id = parts[0];
        let task_id = parts[1];

        self.update_task_status(list_id, task_id, false)
            .await
            .map_err(|e| StreamError::Provider(e.to_string()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use scryforge_provider_core::auth::MockTokenFetcher;

    fn create_test_provider() -> MsTodoProvider {
        let token_fetcher = Arc::new(MockTokenFetcher::empty().with_token(
            "mstodo".to_string(),
            "test".to_string(),
            "test_token_123".to_string(),
        ));
        MsTodoProvider::new(token_fetcher, "test".to_string())
    }

    #[tokio::test]
    async fn test_provider_basics() {
        let provider = create_test_provider();

        assert_eq!(provider.id(), "mstodo");
        assert_eq!(provider.name(), "Microsoft To Do");

        let caps = provider.capabilities();
        assert!(!caps.has_feeds);
        assert!(caps.has_collections);
        assert!(!caps.has_saved_items);
        assert!(!caps.has_communities);
    }

    #[tokio::test]
    async fn test_get_token() {
        let provider = create_test_provider();
        let token = provider.get_token().await.unwrap();
        assert_eq!(token, "test_token_123");
    }

    #[test]
    fn test_task_to_item_conversion() {
        let provider = create_test_provider();

        let task = TodoTask {
            id: "task-123".to_string(),
            title: "Test Task".to_string(),
            body: TaskBody {
                content: "Task description".to_string(),
                content_type: "text".to_string(),
            },
            status: TaskStatus::NotStarted,
            created_date_time: "2024-01-01T10:00:00Z".to_string(),
            last_modified_date_time: "2024-01-01T10:00:00Z".to_string(),
            due_date_time: Some(DateTimeTimeZone {
                date_time: "2024-01-15T00:00:00".to_string(),
                time_zone: "UTC".to_string(),
            }),
            completed_date_time: None,
        };

        let item = provider.task_to_item(task, "list-456");

        assert_eq!(item.id.0, "mstodo:task-123");
        assert_eq!(item.stream_id.0, "mstodo:collection:list-456");
        assert_eq!(item.title, "Test Task");

        if let ItemContent::Task {
            body,
            due_date,
            is_completed,
        } = item.content
        {
            assert_eq!(body, Some("Task description".to_string()));
            assert_eq!(
                due_date,
                Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
            );
            assert!(!is_completed);
        } else {
            panic!("Expected ItemContent::Task");
        }
    }

    #[test]
    fn test_task_to_item_completed() {
        let provider = create_test_provider();

        let task = TodoTask {
            id: "task-456".to_string(),
            title: "Completed Task".to_string(),
            body: TaskBody::default(),
            status: TaskStatus::Completed,
            created_date_time: "2024-01-01T10:00:00Z".to_string(),
            last_modified_date_time: "2024-01-02T10:00:00Z".to_string(),
            due_date_time: None,
            completed_date_time: Some(DateTimeTimeZone {
                date_time: "2024-01-02T10:00:00".to_string(),
                time_zone: "UTC".to_string(),
            }),
        };

        let item = provider.task_to_item(task, "list-789");

        assert_eq!(item.title, "Completed Task");
        assert!(item.is_read);

        if let ItemContent::Task { is_completed, .. } = item.content {
            assert!(is_completed);
        } else {
            panic!("Expected ItemContent::Task");
        }
    }

    #[tokio::test]
    async fn test_available_actions_incomplete_task() {
        let provider = create_test_provider();

        let item = Item {
            id: ItemId::new("mstodo", "task-123"),
            stream_id: StreamId::new("mstodo", "collection", "list-456"),
            title: "Test Task".to_string(),
            content: ItemContent::Task {
                body: Some("Description".to_string()),
                due_date: None,
                is_completed: false,
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: false,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let actions = provider.available_actions(&item).await.unwrap();
        assert!(actions.iter().any(|a| a.id == "complete"));
        assert!(!actions.iter().any(|a| a.id == "uncomplete"));
    }

    #[tokio::test]
    async fn test_available_actions_completed_task() {
        let provider = create_test_provider();

        let item = Item {
            id: ItemId::new("mstodo", "task-123"),
            stream_id: StreamId::new("mstodo", "collection", "list-456"),
            title: "Test Task".to_string(),
            content: ItemContent::Task {
                body: Some("Description".to_string()),
                due_date: None,
                is_completed: true,
            },
            author: None,
            published: None,
            updated: None,
            url: None,
            thumbnail_url: None,
            is_read: true,
            is_saved: false,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let actions = provider.available_actions(&item).await.unwrap();
        assert!(!actions.iter().any(|a| a.id == "complete"));
        assert!(actions.iter().any(|a| a.id == "uncomplete"));
    }

    // Note: Integration tests that actually call the Microsoft Graph API
    // would require a real token and would be better suited for a separate
    // integration test suite. The tests above cover the core logic without
    // requiring network calls.
}

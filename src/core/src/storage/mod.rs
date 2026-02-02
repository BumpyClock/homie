mod sqlite;
mod types;

pub use sqlite::SqliteStore;
pub use types::{
    ChatRecord, JobRecord, JobStatus, NotificationEvent, NotificationSubscription, PairingRecord,
    PairingStatus, SessionStatus, TerminalRecord,
};

use uuid::Uuid;

/// Abstract storage interface for persistence.
///
/// Designed for future backend migration (sqlite → PostgreSQL, DynamoDB, etc.).
/// All methods use `&self` — implementations must handle interior mutability
/// (e.g. `Mutex<Connection>` for sqlite).
pub trait Store: Send + Sync + 'static {
    /// Persist or update a chat record.
    fn upsert_chat(&self, chat: &ChatRecord) -> Result<(), String>;

    /// Get a chat by ID.
    fn get_chat(&self, chat_id: &str) -> Result<Option<ChatRecord>, String>;

    /// List all chats, ordered by created_at descending.
    fn list_chats(&self) -> Result<Vec<ChatRecord>, String>;

    /// Delete a chat by ID.
    fn delete_chat(&self, chat_id: &str) -> Result<(), String>;

    /// Update the append-only event log pointer for a chat.
    fn update_event_pointer(&self, chat_id: &str, pointer: u64) -> Result<(), String>;

    /// Update persisted chat settings (model/effort/approval/etc).
    fn update_chat_settings(
        &self,
        chat_id: &str,
        settings: Option<&serde_json::Value>,
    ) -> Result<(), String>;

    /// Persist or update a terminal session record.
    fn upsert_terminal(&self, rec: &TerminalRecord) -> Result<(), String>;

    /// Get a terminal session by ID.
    fn get_terminal(&self, session_id: Uuid) -> Result<Option<TerminalRecord>, String>;

    /// List all terminal sessions, ordered by started_at descending.
    fn list_terminals(&self) -> Result<Vec<TerminalRecord>, String>;

    /// Delete a terminal session record by ID.
    fn delete_terminal(&self, session_id: Uuid) -> Result<(), String>;

    /// Mark all active sessions as inactive (used on server restart).
    fn mark_all_inactive(&self) -> Result<(), String>;

    /// Persist or update a job record.
    fn upsert_job(&self, job: &JobRecord) -> Result<(), String>;

    /// Get a job by ID.
    fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, String>;

    /// List all jobs, ordered by created_at descending.
    fn list_jobs(&self) -> Result<Vec<JobRecord>, String>;

    /// Remove expired or excess jobs.
    fn prune_jobs(&self, retention_days: u64, max_jobs: usize) -> Result<(), String>;

    /// Persist or update a pairing record.
    fn upsert_pairing(&self, pairing: &PairingRecord) -> Result<(), String>;

    /// Get a pairing by ID.
    fn get_pairing(&self, pairing_id: &str) -> Result<Option<PairingRecord>, String>;

    /// List all pairings, ordered by created_at descending.
    fn list_pairings(&self) -> Result<Vec<PairingRecord>, String>;

    /// Remove expired pairings beyond retention window.
    fn prune_pairings(&self, retention_secs: u64) -> Result<(), String>;

    /// Persist or update a notification subscription.
    fn upsert_notification_subscription(
        &self,
        subscription: &NotificationSubscription,
    ) -> Result<(), String>;

    /// List notification subscriptions.
    fn list_notification_subscriptions(&self) -> Result<Vec<NotificationSubscription>, String>;

    /// Check if any subscription exists for a target.
    fn has_notification_target(&self, target: &str) -> Result<bool, String>;

    /// Insert a notification event for audit/retention.
    fn insert_notification_event(&self, event: &NotificationEvent) -> Result<(), String>;

    /// Remove notification records beyond retention window.
    fn prune_notifications(&self, retention_days: u64) -> Result<(), String>;
}

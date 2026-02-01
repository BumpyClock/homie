mod sqlite;
mod types;

pub use sqlite::SqliteStore;
pub use types::{ChatRecord, SessionStatus, TerminalRecord};

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

    /// Update the append-only event log pointer for a chat.
    fn update_event_pointer(&self, chat_id: &str, pointer: u64) -> Result<(), String>;

    /// Persist or update a terminal session record.
    fn upsert_terminal(&self, rec: &TerminalRecord) -> Result<(), String>;

    /// Get a terminal session by ID.
    fn get_terminal(&self, session_id: Uuid) -> Result<Option<TerminalRecord>, String>;

    /// List all terminal sessions, ordered by started_at descending.
    fn list_terminals(&self) -> Result<Vec<TerminalRecord>, String>;

    /// Mark all active sessions as inactive (used on server restart).
    fn mark_all_inactive(&self) -> Result<(), String>;
}

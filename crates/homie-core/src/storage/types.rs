use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a persisted session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Inactive,
    Exited,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Exited => "exited",
        }
    }

    pub fn from_label(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "exited" => Self::Exited,
            _ => Self::Inactive,
        }
    }
}

/// Persisted chat metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRecord {
    pub chat_id: String,
    pub thread_id: String,
    pub created_at: String,
    pub status: SessionStatus,
    /// Append-only event log pointer — tracks how far the client has consumed.
    pub event_pointer: u64,
}

/// Persisted terminal session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalRecord {
    pub session_id: Uuid,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
    pub status: SessionStatus,
    pub exit_code: Option<u32>,
}

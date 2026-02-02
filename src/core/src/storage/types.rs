use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    /// Persisted chat settings (model/effort/approval/collaboration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
}

/// Persisted terminal session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalRecord {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
    pub status: SessionStatus,
    pub exit_code: Option<u32>,
}

/// Status of a job record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_label(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Queued,
        }
    }
}

/// Persisted job metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub name: String,
    pub status: JobStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub spec: serde_json::Value,
    pub logs: Vec<String>,
}

/// Status of a pairing session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingStatus {
    Pending,
    Approved,
    Revoked,
    Expired,
}

impl PairingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    pub fn from_label(s: &str) -> Self {
        match s {
            "approved" => Self::Approved,
            "revoked" => Self::Revoked,
            "expired" => Self::Expired,
            _ => Self::Pending,
        }
    }
}

/// Persisted pairing session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRecord {
    pub pairing_id: String,
    pub nonce: String,
    pub status: PairingStatus,
    pub created_at: u64,
    pub expires_at: u64,
    pub approved_by: Option<String>,
}

/// Persisted notification subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSubscription {
    pub subscription_id: String,
    pub target: String,
    pub kind: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Persisted notification event for auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub notification_id: String,
    pub title: String,
    pub body: String,
    pub target: Option<String>,
    pub created_at: u64,
}

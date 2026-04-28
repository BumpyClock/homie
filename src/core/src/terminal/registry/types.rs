use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
}

#[derive(Debug)]
pub enum TerminalError {
    NotFound(Uuid),
    Missing(String),
    Internal(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct TmuxSessionInfo {
    pub name: String,
    pub windows: u32,
    pub attached: bool,
}

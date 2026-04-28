use serde_json::Value;

/// Request IDs can be numbers or strings in JSON-RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexRequestId {
    Number(u64),
    Text(String),
}

impl CodexRequestId {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Number(n) => Value::from(*n),
            Self::Text(s) => Value::from(s.clone()),
        }
    }
}

/// A notification or request received from the Codex app-server stdout.
#[derive(Debug, Clone)]
pub struct CodexEvent {
    pub method: String,
    pub id: Option<CodexRequestId>,
    pub params: Option<Value>,
}

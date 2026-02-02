use thiserror::Error;

/// Protocol-level errors.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("binary frame too short: expected at least {expected} bytes, got {got}")]
    FrameTooShort { expected: usize, got: usize },

    #[error("invalid stream type: {0}")]
    InvalidStreamType(u8),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("version negotiation failed")]
    VersionMismatch,
}

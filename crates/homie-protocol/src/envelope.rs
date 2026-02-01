use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Top-level message envelope sent over a text WebSocket frame.
///
/// Discriminated by `type`:
/// - `request`  — client → server RPC
/// - `response` — server → client RPC reply
/// - `event`    — server → client push notification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Request(Request),
    Response(Response),
    Event(Event),
}

/// Client → server RPC request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Unique request identifier for correlation.
    pub id: Uuid,
    /// Dotted method name (e.g. "terminal.session.start").
    pub method: String,
    /// Method-specific parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Server → client RPC response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Matches the `id` of the originating `Request`.
    pub id: Uuid,
    /// Result payload on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error payload on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// Structured RPC error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    /// Machine-readable error code.
    pub code: i32,
    /// Human-readable description.
    pub message: String,
    /// Optional structured data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Well-known RPC error codes.
pub mod error_codes {
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const UNAUTHORIZED: i32 = -32001;
    pub const SESSION_NOT_FOUND: i32 = -32002;
}

/// Server → client push event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Event topic (e.g. "terminal.session.output", "agent.chat.delta").
    pub topic: String,
    /// Event payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            id: Uuid::new_v4(),
            method: method.into(),
            params,
        }
    }
}

impl Response {
    pub fn success(id: Uuid, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Uuid, code: i32, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Encode a `Message` to a JSON string for sending as a text WS frame.
pub fn encode_message(msg: &Message) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

/// Decode a `Message` from a JSON string received as a text WS frame.
pub fn decode_message(text: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_roundtrip() {
        let req = Message::Request(Request::new(
            "terminal.session.start",
            Some(json!({"shell": "/bin/bash"})),
        ));
        let encoded = encode_message(&req).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn response_success_roundtrip() {
        let id = Uuid::new_v4();
        let resp = Message::Response(Response::success(id, json!({"session_id": "abc"})));
        let encoded = encode_message(&resp).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn response_error_roundtrip() {
        let id = Uuid::new_v4();
        let resp = Message::Response(Response::error(id, error_codes::METHOD_NOT_FOUND, "nope"));
        let encoded = encode_message(&resp).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn event_roundtrip() {
        let evt = Message::Event(Event {
            topic: "terminal.session.exit".into(),
            params: Some(json!({"session_id": "abc", "exit_code": 0})),
        });
        let encoded = encode_message(&evt).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(evt, decoded);
    }

    #[test]
    fn response_omits_null_fields() {
        let id = Uuid::new_v4();
        let resp = Message::Response(Response::success(id, json!("ok")));
        let json_str = encode_message(&resp).unwrap();
        assert!(!json_str.contains("\"error\""));
    }

    #[test]
    fn request_type_tag_present() {
        let req = Message::Request(Request::new("test.method", None));
        let json_str = encode_message(&req).unwrap();
        assert!(json_str.contains("\"type\":\"request\""));
    }

    #[test]
    fn event_type_tag_present() {
        let evt = Message::Event(Event {
            topic: "some.topic".into(),
            params: None,
        });
        let json_str = encode_message(&evt).unwrap();
        assert!(json_str.contains("\"type\":\"event\""));
    }
}

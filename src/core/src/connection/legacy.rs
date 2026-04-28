use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct LooseRequest {
    #[serde(rename = "type", default)]
    message_type: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug)]
pub(super) enum LegacyDecode {
    Request(LegacyRequest),
    NonRequest,
}

#[derive(Debug)]
pub(super) struct LegacyRequest {
    pub(super) req_id: Uuid,
    pub(super) response_id: Value,
    pub(super) method: String,
    pub(super) params: Option<Value>,
}

pub(super) fn decode_legacy_request(text: &str) -> Option<LegacyDecode> {
    let req = serde_json::from_str::<LooseRequest>(text).ok()?;
    if let Some(message_type) = req.message_type.as_deref() {
        if !message_type.eq_ignore_ascii_case("request") {
            return Some(LegacyDecode::NonRequest);
        }
    }

    let id = req.id?;
    let method = req.method?;

    let response_id = match &id {
        Value::String(_) | Value::Number(_) => id.clone(),
        _ => return None,
    };

    let req_id = match &id {
        Value::String(value) => Uuid::parse_str(value).unwrap_or_else(|_| Uuid::new_v4()),
        Value::Number(_) => Uuid::new_v4(),
        _ => return None,
    };

    Some(LegacyDecode::Request(LegacyRequest {
        req_id,
        response_id,
        method,
        params: req.params,
    }))
}

#[cfg(test)]
mod tests {
    use super::{decode_legacy_request, LegacyDecode};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn decode_legacy_request_accepts_non_uuid_string_id() {
        let payload = json!({
            "type": "request",
            "id": "req-123",
            "method": "chat.list",
            "params": {}
        });
        let decoded = decode_legacy_request(&payload.to_string());
        match decoded {
            Some(LegacyDecode::Request(legacy)) => {
                assert_eq!(legacy.method, "chat.list");
                assert_eq!(legacy.response_id, json!("req-123"));
                assert_ne!(legacy.req_id, Uuid::nil());
            }
            other => panic!("expected legacy request decode, got {other:?}"),
        }
    }

    #[test]
    fn decode_legacy_request_accepts_typeless_request() {
        let payload = json!({
            "id": "request-abc",
            "method": "chat.model.list"
        });
        let decoded = decode_legacy_request(&payload.to_string());
        match decoded {
            Some(LegacyDecode::Request(legacy)) => {
                assert_eq!(legacy.method, "chat.model.list");
                assert_eq!(legacy.response_id, json!("request-abc"));
            }
            other => panic!("expected legacy request decode, got {other:?}"),
        }
    }

    #[test]
    fn decode_legacy_request_ignores_non_request_messages() {
        let payload = json!({
            "type": "response",
            "id": "req-123",
            "result": {}
        });
        let decoded = decode_legacy_request(&payload.to_string());
        assert!(matches!(decoded, Some(LegacyDecode::NonRequest)));
    }
}

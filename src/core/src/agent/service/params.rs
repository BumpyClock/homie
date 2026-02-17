use serde_json::{json, Map, Value};

use crate::homie_config::ProvidersConfig;
use crate::agent::process::CodexRequestId;
use crate::agent::process::CodexRequestId::Text;
use roci::auth::DeviceCodeSession;
use roci::auth::DeviceCodePoll;

pub(super) fn parse_message_params(
    params: &Option<Value>,
) -> Option<(
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Value>,
    bool,
)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let message = p.get("message")?.as_str()?.to_string();
    let model = p
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let effort = p
        .get("effort")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let approval_policy = p
        .get("approval_policy")
        .or_else(|| p.get("approvalPolicy"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let collaboration_mode = p
        .get("collaboration_mode")
        .or_else(|| p.get("collaborationMode"))
        .cloned();
    let inject = p.get("inject").and_then(|v| v.as_bool()).unwrap_or(false);
    Some((
        chat_id,
        message,
        model,
        effort,
        approval_policy,
        collaboration_mode,
        inject,
    ))
}

pub(super) fn build_chat_settings(
    model: Option<&String>,
    effort: Option<&String>,
    approval_policy: Option<&String>,
    collaboration_mode: Option<&Value>,
) -> Option<Value> {
    let mut map = Map::new();
    if let Some(model) = model {
        map.insert("model".into(), json!(model));
    }
    if let Some(effort) = effort {
        map.insert("effort".into(), json!(effort));
    }
    if let Some(approval_policy) = approval_policy {
        map.insert("approval_policy".into(), json!(approval_policy));
    }
    if let Some(collaboration_mode) = collaboration_mode {
        map.insert("collaboration_mode".into(), collaboration_mode.clone());
    }
    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

pub(super) fn merge_settings(existing: Option<Value>, updates: Value) -> Value {
    match (existing, updates) {
        (Some(Value::Object(mut base)), Value::Object(update)) => {
            for (key, value) in update {
                if value.is_null() {
                    base.remove(&key);
                } else {
                    base.insert(key, value);
                }
            }
            Value::Object(base)
        }
        (_, update) => update,
    }
}

pub(super) fn parse_cancel_params(params: &Option<Value>) -> Option<(String, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let turn_id = p.get("turn_id")?.as_str()?.to_string();
    Some((chat_id, turn_id))
}

pub(super) fn parse_settings_update_params(params: &Option<Value>) -> Option<(String, Value)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let settings = p.get("settings")?.clone();
    Some((chat_id, settings))
}

pub(super) fn normalize_settings_models(settings: Value, providers: &ProvidersConfig) -> Value {
    let mut map = match settings {
        Value::Object(map) => map,
        other => return other,
    };
    if let Some(model_value) = map.get("model").and_then(|v| v.as_str()) {
        map.insert(
            "model".to_string(),
            Value::String(normalize_model_selector(model_value, providers)),
        );
    }
    Value::Object(map)
}

pub(super) fn normalize_model_selector(raw: &str, providers: &ProvidersConfig) -> String {
    let trimmed = raw.trim();
    if !providers.github_copilot.enabled {
        return trimmed.to_string();
    }
    let has_local_openai_compat = providers.openai_compatible.enabled
        && (!providers.openai_compatible.base_url.trim().is_empty()
            || !providers.openai_compatible.models.is_empty());
    if has_local_openai_compat {
        return trimmed.to_string();
    }
    if let Some(model_id) = trimmed.strip_prefix("openai-compatible:") {
        if super::models::is_known_copilot_model(model_id) {
            return format!("github-copilot:{model_id}");
        }
    }
    trimmed.to_string()
}

pub(super) fn parse_files_search_params(
    params: &Option<Value>,
) -> Option<(String, String, usize, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let query = p.get("query")?.as_str()?.to_string();
    let limit = p
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(200) as usize)
        .unwrap_or(40);
    let base_path = p
        .get("base_path")
        .or_else(|| p.get("basePath"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    Some((chat_id, query, limit, base_path))
}

pub(super) fn parse_tool_channel(params: &Option<Value>) -> String {
    params
        .as_ref()
        .and_then(|value| value.get("channel"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "web".to_string())
}

pub(super) fn parse_resume_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

pub(super) fn parse_thread_read_params(
    params: &Option<Value>,
) -> Option<(Option<String>, Option<String>, bool)> {
    let p = params.as_ref()?;
    let chat_id = p
        .get("chat_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let include_turns = p
        .get("include_turns")
        .or_else(|| p.get("includeTurns"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if chat_id.is_none() && thread_id.is_none() {
        None
    } else {
        Some((chat_id, thread_id, include_turns))
    }
}

pub(super) fn parse_thread_archive_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

pub(super) fn parse_thread_rename_params(
    params: &Option<Value>,
) -> Option<(String, Option<String>, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let title = p
        .get("title")
        .or_else(|| p.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id, title))
}

pub(super) fn parse_approval_params(params: &Option<Value>) -> Option<(CodexRequestId, String)> {
    let p = params.as_ref()?;
    let raw = p.get("codex_request_id")?;
    let codex_request_id = if let Some(num) = raw.as_u64() {
        CodexRequestId::Number(num)
    } else if let Some(num) = raw.as_i64() {
        if num < 0 {
            return None;
        }
        CodexRequestId::Number(num as u64)
    } else if let Some(text) = raw.as_str() {
        Text(text.to_string())
    } else {
        return None;
    };
    let decision = p.get("decision")?.as_str()?.to_string();
    Some((codex_request_id, decision))
}

pub(super) fn parse_account_provider_params(
    params: &Option<Value>,
) -> Option<(String, String, Map<String, Value>)> {
    let p = params.as_ref()?.as_object()?;
    let provider_raw = p.get("provider")?.as_str()?;
    let provider_id = normalize_provider_id(provider_raw)?;
    let profile = p
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    Some((provider_id, profile, p.clone()))
}

fn normalize_provider_id(raw: &str) -> Option<String> {
    match raw.trim() {
        "openai-codex" | "openai_codex" => Some("openai-codex".to_string()),
        "github-copilot" | "github_copilot" => Some("github-copilot".to_string()),
        "claude-code" | "claude_code" => Some("claude-code".to_string()),
        _ => None,
    }
}

pub(super) fn parse_device_code_session(
    params: &Map<String, Value>,
    provider_id: &str,
) -> Option<DeviceCodeSession> {
    let session = params
        .get("session")
        .and_then(|v| v.as_object())
        .unwrap_or(params);
    let device_code = get_string(session, &["device_code", "deviceCode"])?;
    let user_code = get_string(session, &["user_code", "userCode"])?;
    let verification_url = get_string(session, &["verification_url", "verificationUrl"])?;
    let interval_secs = get_u64(session, &["interval_secs", "intervalSecs"])?;
    let expires_at_str = get_string(session, &["expires_at", "expiresAt"])?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
        .ok()?
        .with_timezone(&chrono::Utc);
    Some(DeviceCodeSession {
        provider: provider_id.to_string(),
        verification_url,
        user_code,
        device_code,
        interval_secs,
        expires_at,
    })
}

pub(super) fn device_code_session_json(session: &DeviceCodeSession) -> Value {
    json!({
        "provider": session.provider,
        "verification_url": session.verification_url,
        "user_code": session.user_code,
        "device_code": session.device_code,
        "interval_secs": session.interval_secs,
        "expires_at": session.expires_at.to_rfc3339(),
    })
}

pub(super) fn device_code_poll_json(poll: DeviceCodePoll) -> Value {
    match poll {
        DeviceCodePoll::Pending { interval_secs } => {
            json!({ "status": "pending", "interval_secs": interval_secs })
        }
        DeviceCodePoll::SlowDown { interval_secs } => {
            json!({ "status": "slow_down", "interval_secs": interval_secs })
        }
        DeviceCodePoll::Authorized { .. } => json!({ "status": "authorized" }),
        DeviceCodePoll::AccessDenied => json!({ "status": "denied" }),
        DeviceCodePoll::Expired => json!({ "status": "expired" }),
    }
}

pub(super) fn get_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(super) fn get_u64(map: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(num) = value.as_u64() {
                return Some(num);
            }
            if let Some(text) = value.as_str() {
                if let Ok(num) = text.parse::<u64>() {
                    return Some(num);
                }
            }
        }
    }
    None
}

pub(super) fn extract_thread_id(params: &Value) -> Option<String> {
    params
        .get("threadId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("thread_id").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

pub(super) fn extract_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("turn_id").and_then(|v| v.as_str()))
        .or_else(|| {
            params
                .get("turn")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
}

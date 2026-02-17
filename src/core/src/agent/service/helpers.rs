use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};
use roci::auth::TokenStoreConfig;
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::auth::providers::claude_code::ClaudeCodeAuth;
use roci::auth::providers::github_copilot::GitHubCopilotAuth;
use roci::auth::providers::openai_codex::OpenAiCodexAuth;
use crate::homie_config::{OpenAiCompatibleProviderConfig, ProvidersConfig};
use crate::outbound::OutboundMessage;
use crate::paths::homie_skills_dir;
use crate::router::ReapEvent;
use crate::storage::{ChatRecord, SessionStatus, Store};
use crate::{ExecPolicy, HomieConfig};

use super::process::{CodexEvent, CodexProcess, CodexRequestId, CodexResponseSender};
use super::roci_backend::{ChatBackend, RociBackend};
use super::tools::{list_tools, ToolContext, DEFAULT_TOOL_CHANNEL};
use roci::agent_loop::ApprovalDecision;
use shell_words;
use reqwest;
use crate::agent::service::core::CodexChatCore;
use std::sync::Arc;

fn codex_method_to_topics(method: &str) -> Option<(&'static str, &'static str)> {
    match method {
        "item/agentMessage/delta" => Some(("chat.message.delta", "agent.chat.delta")),
        "item/started" => Some(("chat.item.started", "agent.chat.item.started")),
        "item/completed" => Some(("chat.item.completed", "agent.chat.item.completed")),
        "turn/started" => Some(("chat.turn.started", "agent.chat.turn.started")),
        "turn/completed" => Some(("chat.turn.completed", "agent.chat.turn.completed")),
        "item/commandExecution/outputDelta" => {
            Some(("chat.command.output", "agent.chat.command.output"))
        }
        "item/fileChange/outputDelta" => Some(("chat.file.output", "agent.chat.file.output")),
        "item/reasoning/summaryTextDelta" => {
            Some(("chat.reasoning.delta", "agent.chat.reasoning.delta"))
        }
        "turn/diff/updated" => Some(("chat.diff.updated", "agent.chat.diff.updated")),
        "turn/plan/updated" => Some(("chat.plan.updated", "agent.chat.plan.updated")),
        "thread/tokenUsage/updated" => {
            Some(("chat.token.usage.updated", "agent.chat.token.usage.updated"))
        }
        "item/commandExecution/requestApproval" => {
            Some(("chat.approval.required", "agent.chat.approval.required"))
        }
        "item/fileChange/requestApproval" => {
            Some(("chat.approval.required", "agent.chat.approval.required"))
        }
        _ => None,
    }
}
fn debug_enabled() -> bool {
    matches!(
        std::env::var("HOMIE_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    ) || matches!(
        std::env::var("HOME_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    )
}
/// Background task: reads Codex events and forwards them as Homie Event
/// messages via the outbound WS channel.
async fn event_forwarder_loop(
    mut event_rx: mpsc::Receiver<CodexEvent>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: Arc<dyn Store>,
    response_sender: CodexResponseSender,
    exec_policy: Arc<ExecPolicy>,
    homie_config: Arc<HomieConfig>,
) {
    while let Some(event) = event_rx.recv().await {
        let raw_params = event.params.unwrap_or(json!({}));
        if homie_config.raw_events_enabled() {
            if let (Some(thread_id), Some(run_id)) =
                (extract_thread_id(&raw_params), extract_turn_id(&raw_params))
            {
                if store
                    .insert_chat_raw_event(&run_id, &thread_id, &event.method, &raw_params)
                    .is_ok()
                {
                    let _ = store.prune_chat_raw_events(10);
                }
            }
        }

        if event.method == "item/commandExecution/requestApproval" {
            if let Some(id) = event.id.clone() {
                if let Some(argv) = approval_command_argv(&raw_params) {
                    if exec_policy.is_allowed(&argv) {
                        if let Some(thread_id) = extract_thread_id(&raw_params) {
                            if let Ok(Some(chat)) = store.get_chat(&thread_id) {
                                let next = chat.event_pointer.saturating_add(1);
                                let _ = store.update_event_pointer(&chat.chat_id, next);
                            }
                        }
                        let result = json!({ "decision": "accept" });
                        if response_sender.send_response(id, result).await.is_ok() {
                            tracing::info!("execpolicy auto-approved command");
                            continue;
                        }
                    }
                }
            }
        }

        let (chat_topic, agent_topic) = match codex_method_to_topics(&event.method) {
            Some(t) => t,
            None => {
                tracing::debug!(method = %event.method, "unmapped codex event, skipping");
                continue;
            }
        };

        let mut event_params = raw_params;

        if let Some(codex_id) = event.id {
            if let Some(obj) = event_params.as_object_mut() {
                obj.insert("codex_request_id".into(), codex_id.to_json());
            }
        } else if let Some(obj) = event_params.as_object_mut() {
            if !obj.contains_key("codex_request_id") {
                let fallback = obj
                    .get("requestId")
                    .or_else(|| obj.get("request_id"))
                    .or_else(|| obj.get("id"))
                    .cloned();
                if let Some(value) = fallback {
                    obj.insert("codex_request_id".into(), value);
                }
            }
        }

        if matches!(
            event.method.as_str(),
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
        ) {
            tracing::info!(
                method = %event.method,
                params = ?event_params,
                "codex approval requested"
            );
        }

        if let Some(thread_id) = extract_thread_id(&event_params) {
            if let Ok(Some(chat)) = store.get_chat(&thread_id) {
                let next = chat.event_pointer.saturating_add(1);
                let _ = store.update_event_pointer(&chat.chat_id, next);
            }
        }

        let chat_params = event_params.clone();
        match outbound_tx.try_send(OutboundMessage::event(chat_topic, Some(chat_params))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic = chat_topic, "backpressure: dropping chat event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }

        match outbound_tx.try_send(OutboundMessage::event(agent_topic, Some(event_params))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic = agent_topic, "backpressure: dropping agent event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }
    }

    tracing::debug!("agent event forwarder exited");
}

fn approval_command_argv(params: &Value) -> Option<Vec<String>> {
    let command = params.get("command")?.as_str()?;
    if command.trim().is_empty() {
        return None;
    }
    shell_words::split(command).ok()
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

fn codex_model() -> String {
    std::env::var("HOMIE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.1-codex".to_string())
}

fn extract_id_from_result(
    value: &Value,
    direct_keys: &[&str],
    nested_keys: &[(&str, &str)],
) -> Option<String> {
    for key in direct_keys {
        if let Some(id) = value.get(*key).and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    for (outer, inner) in nested_keys {
        if let Some(id) = value
            .get(*outer)
            .and_then(|v| v.get(*inner))
            .and_then(|v| v.as_str())
        {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

// -- Param parsing helpers ------------------------------------------------

fn parse_message_params(
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

fn build_chat_settings(
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

fn merge_settings(existing: Option<Value>, updates: Value) -> Value {
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

fn parse_cancel_params(params: &Option<Value>) -> Option<(String, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let turn_id = p.get("turn_id")?.as_str()?.to_string();
    Some((chat_id, turn_id))
}

fn parse_settings_update_params(params: &Option<Value>) -> Option<(String, Value)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let settings = p.get("settings")?.clone();
    Some((chat_id, settings))
}

fn normalize_settings_models(settings: Value, providers: &ProvidersConfig) -> Value {
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

fn normalize_model_selector(raw: &str, providers: &ProvidersConfig) -> String {
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
        if is_known_copilot_model(model_id) {
            return format!("github-copilot:{model_id}");
        }
    }
    trimmed.to_string()
}

const COPILOT_FALLBACK_MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4.1-mini",
    "gpt-4.1-nano",
    "gpt-4.1",
    "gpt-5",
    "gpt-5-mini",
    "gpt-5.3-codex",
    "gpt-5-codex",
    "gpt-5.1",
    "gpt-5.1-codex",
    "gpt-5.1-codex-mini",
    "gpt-5.1-codex-max",
    "gpt-5.2",
    "gpt-5.2-codex",
    "o1",
    "o1-preview",
    "o1-mini",
    "o3",
    "o3-mini",
    "o4-mini",
    "o3-deep-research",
    "o4-mini-deep-research",
    "claude-haiku-4.5",
    "claude-opus-4.1",
    "claude-opus-4.5",
    "claude-opus-4.6",
    "claude-opus-4.6-fast",
    "claude-sonnet-3.5",
    "claude-sonnet-3.7",
    "claude-sonnet-4",
    "claude-sonnet-4.5",
    "gemini-2.0-flash",
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-3-flash",
    "gemini-3-pro",
    "grok-code-fast-1",
    "raptor-mini",
];

fn is_known_copilot_model(model_id: &str) -> bool {
    COPILOT_FALLBACK_MODELS
        .iter()
        .any(|known| *known == model_id.trim())
}

fn parse_files_search_params(
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

fn parse_tool_channel(params: &Option<Value>) -> String {
    params
        .as_ref()
        .and_then(|value| value.get("channel"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_TOOL_CHANNEL.to_string())
}

fn parse_resume_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

fn parse_thread_read_params(
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

fn parse_thread_archive_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

fn parse_thread_rename_params(params: &Option<Value>) -> Option<(String, Option<String>, String)> {
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

fn parse_approval_params(params: &Option<Value>) -> Option<(CodexRequestId, String)> {
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
        CodexRequestId::Text(text.to_string())
    } else {
        return None;
    };
    let decision = p.get("decision")?.as_str()?.to_string();
    Some((codex_request_id, decision))
}

fn parse_account_provider_params(
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

fn parse_device_code_session(
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
    let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
        .ok()?
        .with_timezone(&Utc);
    Some(DeviceCodeSession {
        provider: provider_id.to_string(),
        verification_url,
        user_code,
        device_code,
        interval_secs,
        expires_at,
    })
}

fn device_code_session_json(session: &DeviceCodeSession) -> Value {
    json!({
        "provider": session.provider,
        "verification_url": session.verification_url,
        "user_code": session.user_code,
        "device_code": session.device_code,
        "interval_secs": session.interval_secs,
        "expires_at": session.expires_at.to_rfc3339(),
    })
}

fn device_code_poll_json(poll: DeviceCodePoll) -> Value {
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

fn get_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn get_u64(map: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
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

fn extract_thread_id(params: &Value) -> Option<String> {
    params
        .get("threadId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("thread_id").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

fn extract_turn_id(params: &Value) -> Option<String> {
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

fn extract_attached_folder(settings: Option<&Value>) -> Option<String> {
    let settings = settings?;
    let attachments = settings.get("attachments")?;
    if let Some(folder) = attachments.get("folder").and_then(|v| v.as_str()) {
        if !folder.trim().is_empty() {
            return Some(folder.to_string());
        }
    }
    let folders = attachments.get("folders").and_then(|v| v.as_array())?;
    folders
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| !v.trim().is_empty())
        .map(|v| v.to_string())
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".cache"
    )
}

fn normalize_search_root(base: &str) -> PathBuf {
    let trimmed = base.trim();
    let home_dir = crate::paths::user_home_dir();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home_dir.as_ref() {
            return home.join(rest);
        }
    }
    if trimmed == "~" {
        if let Some(home) = home_dir.as_ref() {
            return home.to_path_buf();
        }
    }
    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(path);
        }
    }
    path
}

fn search_files_in_folder(base: &str, query: &str, limit: usize) -> Result<Vec<Value>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let base_path = normalize_search_root(base);
    if !base_path.is_dir() {
        return Ok(Vec::new());
    }

    let mut queue = VecDeque::new();
    queue.push_back(base_path.clone());
    let mut results = Vec::new();
    let mut visited = 0usize;
    let query_lower = query.to_lowercase();

    while let Some(dir) = queue.pop_front() {
        if visited > 25_000 || results.len() >= limit {
            break;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if results.len() >= limit {
                break;
            }
            visited = visited.saturating_add(1);
            if visited > 25_000 {
                break;
            }
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                queue.push_back(path.clone());
            }
            if !file_type.is_file() && !file_type.is_dir() {
                continue;
            }
            let rel = match path.strip_prefix(&base_path) {
                Ok(p) => p,
                Err(_) => Path::new(&name),
            };
            let rel_str = rel.to_string_lossy().to_string();
            let haystack = format!("{name} {rel_str}").to_lowercase();
            if !haystack.contains(&query_lower) {
                continue;
            }
            visited += 1;
            let kind = if file_type.is_dir() {
                "directory"
            } else {
                "file"
            };
            results.push(json!({
                "name": name,
                "path": path.to_string_lossy(),
                "relative_path": rel_str,
                "type": kind,
            }));
        }
    }

    Ok(results)
}

fn list_homie_skills() -> Result<Vec<Value>, String> {
    let dir = homie_skills_dir()?;
    let mut skills = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("read skills dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read skills dir entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => continue,
        };
        let path_str = path.to_string_lossy().to_string();
        skills.push(json!({ "name": name, "path": path_str }));
    }
    skills.sort_by(|a, b| {
        let a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a.cmp(b)
    });
    Ok(skills)
}

fn parse_openai_compat_models_csv(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn append_openai_compatible_models(models: &mut Vec<Value>, compat_models: Vec<String>) {
    if compat_models.is_empty() {
        return;
    }

    let mut seen = HashSet::new();
    let mut has_default = false;
    for entry in models.iter() {
        if let Some(model_id) = entry.get("model").and_then(|value| value.as_str()) {
            seen.insert(model_id.to_string());
        }
        if entry
            .get("is_default")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            has_default = true;
        }
    }

    for model_id in compat_models {
        let selector = format!("openai-compatible:{model_id}");
        if !seen.insert(selector.clone()) {
            continue;
        }
        let is_default = !has_default;
        if is_default {
            has_default = true;
        }
        models.push(json!({
            "id": selector,
            "model": selector,
            "provider": "openai-compatible",
            "display_name": format!("{model_id} (Local)"),
            "is_default": is_default,
        }));
    }
}

fn replace_github_copilot_models(models: &mut Vec<Value>, copilot_models: Vec<String>) {
    if copilot_models.is_empty() {
        return;
    }
    models.retain(|entry| {
        entry
            .get("provider")
            .and_then(|value| value.as_str())
            .map(|provider| provider != "github-copilot")
            .unwrap_or(true)
    });

    let mut seen = HashSet::new();
    let mut has_default = models.iter().any(|entry| {
        entry
            .get("is_default")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    });
    for model_id in copilot_models {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        let selector = format!("github-copilot:{trimmed}");
        let is_default = !has_default;
        if is_default {
            has_default = true;
        }
        models.push(json!({
            "id": selector,
            "model": selector,
            "provider": "github-copilot",
            "display_name": format!("{trimmed} (Copilot)"),
            "is_default": is_default,
        }));
    }
}

async fn discover_github_copilot_models(auth: &GitHubCopilotAuth) -> Result<Vec<String>, String> {
    let token = auth
        .exchange_copilot_token()
        .await
        .map_err(|err| format!("exchange github-copilot token: {err}"))?;
    let endpoint = format!("{}/models", token.base_url.trim().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|err| format!("build github-copilot http client: {err}"))?;
    let response = client
        .get(endpoint)
        .bearer_auth(token.token)
        .send()
        .await
        .map_err(|err| format!("request github-copilot models: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "github-copilot models request returned status {status}"
        ));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|err| format!("decode github-copilot models payload: {err}"))?;
    let mut discovered = Vec::new();
    if let Some(items) = payload.get("data").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(model_id) = item.get("id").and_then(|value| value.as_str()) {
                let normalized = model_id.trim();
                if !normalized.is_empty() {
                    discovered.push(normalized.to_string());
                }
            }
        }
    }
    if discovered.is_empty() {
        return Err("github-copilot models response did not include model ids".to_string());
    }
    let mut unique = HashSet::new();
    discovered.retain(|value| unique.insert(value.clone()));
    Ok(discovered)
}

async fn discover_openai_compatible_models(
    provider_cfg: &OpenAiCompatibleProviderConfig,
) -> Result<Vec<String>, String> {
    let mut fallback = if let Ok(value) = std::env::var("OPENAI_COMPAT_MODELS") {
        parse_openai_compat_models_csv(&value)
    } else {
        provider_cfg
            .models
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
    };
    if !fallback.is_empty() {
        let mut unique = HashSet::new();
        fallback.retain(|value| unique.insert(value.clone()));
    }

    let config = RociConfig::from_env();
    let base_url = if let Some(url) = config.get_base_url("openai-compatible") {
        url
    } else {
        provider_cfg.base_url.clone()
    };
    if base_url.trim().is_empty() {
        return Ok(fallback);
    }

    let endpoint = format!("{}/models", base_url.trim().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|err| format!("build openai-compatible http client: {err}"))?;

    let mut request = client.get(endpoint.as_str());
    let api_key = config
        .get_api_key("openai-compatible")
        .unwrap_or_else(|| provider_cfg.api_key.clone());
    if !api_key.trim().is_empty() {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request openai-compatible models: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        if fallback.is_empty() {
            return Err(format!(
                "openai-compatible models request returned status {status}"
            ));
        }
        tracing::warn!(
            "openai-compatible model discovery returned status {status}; using OPENAI_COMPAT_MODELS fallback"
        );
        return Ok(fallback);
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|err| format!("decode openai-compatible models payload: {err}"))?;
    let mut discovered = Vec::new();
    if let Some(items) = payload.get("data").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(model_id) = item.get("id").and_then(|value| value.as_str()) {
                let normalized = model_id.trim();
                if !normalized.is_empty() {
                    discovered.push(normalized.to_string());
                }
            }
        }
    }

    if discovered.is_empty() {
        if fallback.is_empty() {
            return Err("openai-compatible models response did not include model ids".to_string());
        }
        return Ok(fallback);
    }

    let mut unique = HashSet::new();
    discovered.retain(|value| unique.insert(value.clone()));
    Ok(discovered)
}

fn roci_model_catalog(providers: &ProvidersConfig) -> Vec<Value> {
    let mut models = Vec::new();
    let mut default_set = false;
    let has_openai_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    if has_openai_key {
        let openai_models = [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4.1",
            "gpt-4.1-mini",
            "gpt-4.1-nano",
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5.2",
            "o1",
            "o1-mini",
            "o1-pro",
            "o3",
            "o3-mini",
            "o4-mini",
        ];
        for model_id in openai_models {
            let model = format!("openai:{model_id}");
            let is_default = !default_set && model_id == "gpt-4o-mini";
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "openai",
                "display_name": model_id,
                "is_default": is_default,
            }));
        }
    }

    if providers.openai_codex.enabled {
        let codex_models = [
            "gpt-5.3-codex",
            "gpt-5.2",
            "gpt-5.2-codex",
            "gpt-5-codex",
            "gpt-5.1-codex",
            "gpt-5.1-codex-mini",
            "gpt-5.1-codex-max",
        ];
        for (idx, model_id) in codex_models.iter().enumerate() {
            let model = format!("openai-codex:{model_id}");
            let is_default = !default_set && idx == 0;
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "openai-codex",
                "display_name": format!("{model_id} (Codex)"),
                "is_default": is_default,
            }));
        }
    }

    if providers.github_copilot.enabled {
        for model_id in COPILOT_FALLBACK_MODELS {
            let model = format!("github-copilot:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "github-copilot",
                "display_name": format!("{model_id} (Copilot)"),
                "is_default": false,
            }));
        }
    }

    if providers.claude_code.enabled {
        let claude_models = [
            "claude-sonnet-4-5-20250514",
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-20250514",
            "claude-haiku-3-5-20241022",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
        ];
        for model_id in claude_models {
            let model = format!("anthropic:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "anthropic",
                "display_name": model_id,
                "is_default": false,
            }));
        }
    }

    models
}


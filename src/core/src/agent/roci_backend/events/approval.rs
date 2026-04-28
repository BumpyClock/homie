use roci::agent_loop::ApprovalRequest;
use serde_json::Value;

use super::json_utils::{canonical_json, normalize_json};

pub(in crate::agent::roci_backend) fn approval_command_from_payload(
    payload: &Value,
) -> (Option<String>, Option<String>) {
    let obj = match payload.as_object() {
        Some(obj) => obj,
        None => return (None, None),
    };
    let args = payload_arguments(payload)
        .and_then(|v| v.as_object())
        .unwrap_or(obj);
    let command = if let Some(argv) = obj.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    } else if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
        Some(cmd.to_string())
    } else if let Some(argv) = args.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    } else if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
        Some(cmd.to_string())
    } else {
        obj.get("tool_name")
            .and_then(|v| v.as_str())
            .map(|tool| tool.to_string())
    };
    let cwd = obj
        .get("cwd")
        .and_then(|v| v.as_str())
        .or_else(|| args.get("cwd").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    (command, cwd)
}

pub(in crate::agent::roci_backend) fn approval_command_argv(
    payload: &Value,
) -> Option<Vec<String>> {
    let obj = payload.as_object()?;
    let args = payload_arguments(payload)
        .and_then(|v| v.as_object())
        .unwrap_or(obj);
    if let Some(argv) = obj.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    if let Some(argv) = args.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .or_else(|| args.get("command").and_then(|v| v.as_str()))?
        .trim();
    if command.is_empty() {
        return None;
    }
    shell_words::split(command).ok()
}

pub(in crate::agent::roci_backend) fn approval_cache_key(
    request: &ApprovalRequest,
) -> Option<String> {
    let mut payload = request.payload.clone();
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("tool_call_id");
    }
    let normalized = normalize_json(payload);
    let kind = match request.kind {
        roci::agent_loop::ApprovalKind::CommandExecution => "command",
        roci::agent_loop::ApprovalKind::FileChange => "file",
        roci::agent_loop::ApprovalKind::Other => "other",
    };
    Some(format!("{kind}|{}", canonical_json(&normalized)))
}

fn payload_arguments(payload: &Value) -> Option<&Value> {
    let obj = payload.as_object()?;
    obj.get("arguments")
        .or_else(|| obj.get("args"))
        .or_else(|| obj.get("input"))
}

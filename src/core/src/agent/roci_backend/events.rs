use std::sync::Arc;

use roci::agent_loop::ApprovalRequest;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::outbound::OutboundMessage;
use crate::storage::Store;

pub(super) struct ToolEventContext<'a> {
    outbound: &'a mpsc::Sender<OutboundMessage>,
    store: &'a Arc<dyn Store>,
    chat_id: &'a str,
    thread_id: &'a str,
    turn_id: &'a str,
}

impl<'a> ToolEventContext<'a> {
    pub(super) fn new(
        outbound: &'a mpsc::Sender<OutboundMessage>,
        store: &'a Arc<dyn Store>,
        chat_id: &'a str,
        thread_id: &'a str,
        turn_id: &'a str,
    ) -> Self {
        Self {
            outbound,
            store,
            chat_id,
            thread_id,
            turn_id,
        }
    }
}

pub(super) struct ToolItemStartedData<'a> {
    item_id: &'a str,
    tool_name: &'a str,
    input: &'a Value,
}

impl<'a> ToolItemStartedData<'a> {
    pub(super) fn new(item_id: &'a str, tool_name: &'a str, input: &'a Value) -> Self {
        Self {
            item_id,
            tool_name,
            input,
        }
    }
}

pub(super) struct ToolItemCompletedData<'a> {
    item_id: &'a str,
    tool_name: &'a str,
    input: &'a Value,
    result: &'a Value,
    is_error: bool,
}

impl<'a> ToolItemCompletedData<'a> {
    pub(super) fn new(
        item_id: &'a str,
        tool_name: &'a str,
        input: &'a Value,
        result: &'a Value,
        is_error: bool,
    ) -> Self {
        Self {
            item_id,
            tool_name,
            input,
            result,
            is_error,
        }
    }
}

pub(super) fn emit_error(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    message: String,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "status": "failed" })),
    );
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.error",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "message": message })),
    );
}

pub(super) fn emit_turn_started(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id })),
    );
}

pub(super) fn emit_turn_completed(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    status: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "status": status })),
    );
}

pub(super) fn emit_message_delta(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.message.delta",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": item_id,
            "delta": delta,
        })),
    );
}

pub(super) fn emit_reasoning_delta(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.reasoning.delta",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": item_id,
            "delta": delta,
        })),
    );
}

pub(super) fn emit_user_item(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: String,
    text: &str,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "userMessage",
        "content": [{ "type": "text", "text": text }],
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

pub(super) fn emit_assistant_item(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: String,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "agentMessage",
        "text": "",
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

pub(super) fn emit_item_completed(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    text: &str,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "agentMessage",
        "text": text,
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

pub(super) fn emit_tool_item_started(ctx: ToolEventContext<'_>, data: ToolItemStartedData<'_>) {
    let item = serde_json::json!({
        "id": data.item_id,
        "type": "mcpToolCall",
        "tool": data.tool_name,
        "status": "running",
        "input": data.input,
    });
    emit_event(
        ctx.outbound,
        ctx.store,
        ctx.chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": ctx.thread_id, "turnId": ctx.turn_id, "item": item })),
    );
}

pub(super) fn emit_tool_item_completed(ctx: ToolEventContext<'_>, data: ToolItemCompletedData<'_>) {
    let status = if data.is_error { "failed" } else { "completed" };
    let item = serde_json::json!({
        "id": data.item_id,
        "type": "mcpToolCall",
        "tool": data.tool_name,
        "status": status,
        "input": data.input,
        "result": data.result,
        "error": data.is_error,
    });
    emit_event(
        ctx.outbound,
        ctx.store,
        ctx.chat_id,
        "chat.item.completed",
        Some(serde_json::json!({ "threadId": ctx.thread_id, "turnId": ctx.turn_id, "item": item })),
    );
}

pub(super) fn emit_approval_required(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    request: &ApprovalRequest,
) {
    let (command, cwd) = approval_command_from_payload(&request.payload);
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.approval.required",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": request.id,
            "request_id": request.id,
            "codex_request_id": request.id,
            "reason": request.reason,
            "command": command,
            "cwd": cwd,
        })),
    );
}

pub(super) fn emit_plan_updated(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    plan: &str,
) {
    let trimmed = plan.trim();
    let steps = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .lines()
            .filter_map(|line| {
                let step = line.trim().trim_start_matches("- ").trim();
                if step.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "step": step, "status": "pending" }))
                }
            })
            .collect::<Vec<_>>()
    };
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.plan.updated",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "plan": steps,
        })),
    );
}

pub(super) fn emit_diff_updated(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    diff: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.diff.updated",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "diff": diff,
        })),
    );
}

pub(super) fn approval_command_from_payload(payload: &Value) -> (Option<String>, Option<String>) {
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

pub(super) fn approval_command_argv(payload: &Value) -> Option<Vec<String>> {
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

fn payload_arguments(payload: &Value) -> Option<&Value> {
    let obj = payload.as_object()?;
    obj.get("arguments")
        .or_else(|| obj.get("args"))
        .or_else(|| obj.get("input"))
}

pub(super) fn approval_cache_key(request: &ApprovalRequest) -> Option<String> {
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

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn normalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut normalized = serde_json::Map::new();
            for (key, value) in entries {
                normalized.insert(key, normalize_json(value));
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize_json).collect()),
        other => other,
    }
}

fn emit_event(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    topic: &str,
    params: Option<Value>,
) {
    if let Ok(Some(chat)) = store.get_chat(chat_id) {
        let next = chat.event_pointer.saturating_add(1);
        let _ = store.update_event_pointer(chat_id, next);
    }
    match outbound.try_send(OutboundMessage::event(topic, params)) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!(topic = topic, "backpressure: dropping chat event");
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {}
    }
}

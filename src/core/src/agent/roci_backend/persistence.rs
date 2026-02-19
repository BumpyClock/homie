use std::collections::HashMap;
use std::sync::Arc;

use roci::types::ModelMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::storage::{ChatRawEventRecord, Store};

use super::state::{
    last_assistant_item_id_from_turns, model_messages_from_turns, upsert_assistant_item,
    upsert_tool_item, upsert_user_item, RociThread, RociThreadState, RociTurn,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedThreadSnapshot {
    pub(super) thread: RociThread,
    #[serde(default)]
    pub(super) messages: Vec<ModelMessage>,
    #[serde(default)]
    pub(super) last_assistant_item_id: Option<String>,
}

impl PersistedThreadSnapshot {
    pub(super) fn from_thread_state(state: &RociThreadState) -> Self {
        Self {
            thread: state.thread.clone(),
            messages: state.messages.clone(),
            last_assistant_item_id: state.last_assistant_item_id.clone(),
        }
    }

    fn into_thread_state(self, thread_id: &str) -> RociThreadState {
        let mut thread = self.thread;
        if thread.id != thread_id {
            thread.id = thread_id.to_string();
        }
        let messages = if self.messages.is_empty() && !thread.turns.is_empty() {
            model_messages_from_turns(&thread.turns)
        } else {
            self.messages
        };
        let last_assistant_item_id = self
            .last_assistant_item_id
            .or_else(|| last_assistant_item_id_from_turns(&thread.turns));
        RociThreadState {
            thread,
            messages,
            last_assistant_item_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PersistedThreadSnapshotPayload {
    Snapshot(PersistedThreadSnapshot),
    LegacyThread(RociThread),
}

pub(super) fn decode_persisted_thread_state(
    thread_id: &str,
    value: Value,
) -> Option<RociThreadState> {
    let payload = match serde_json::from_value::<PersistedThreadSnapshotPayload>(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to decode persisted roci thread state: {error}");
            return None;
        }
    };
    let state = match payload {
        PersistedThreadSnapshotPayload::Snapshot(snapshot) => snapshot.into_thread_state(thread_id),
        PersistedThreadSnapshotPayload::LegacyThread(thread) => PersistedThreadSnapshot {
            messages: model_messages_from_turns(&thread.turns),
            last_assistant_item_id: last_assistant_item_id_from_turns(&thread.turns),
            thread,
        }
        .into_thread_state(thread_id),
    };
    Some(state)
}

pub(super) fn persist_thread_snapshot(
    store: &Arc<dyn Store>,
    thread_id: &str,
    snapshot: Option<PersistedThreadSnapshot>,
) {
    let Some(snapshot) = snapshot else {
        return;
    };
    let value = match serde_json::to_value(snapshot) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to serialize roci thread state: {error}");
            return;
        }
    };
    if let Err(error) = store.upsert_chat_thread_state(thread_id, &value) {
        tracing::warn!(%thread_id, "failed to persist roci thread state: {error}");
    }
}

pub(super) fn backfill_thread_state_from_raw_events(
    store: &Arc<dyn Store>,
    thread_id: &str,
) -> Option<RociThreadState> {
    let events = match store.list_chat_raw_events(thread_id, 8_000) {
        Ok(events) => events,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to read raw events for backfill: {error}");
            return None;
        }
    };
    if events.is_empty() {
        return None;
    }

    let mut thread = RociThread {
        id: thread_id.to_string(),
        created_at: events
            .first()
            .map(|e| e.created_at)
            .unwrap_or_else(super::now_unix),
        updated_at: events
            .last()
            .map(|e| e.created_at)
            .unwrap_or_else(super::now_unix),
        turns: Vec::new(),
    };
    let mut turn_indices: HashMap<String, usize> = HashMap::new();

    for event in &events {
        apply_raw_event_to_thread(&mut thread, &mut turn_indices, event);
    }

    if thread.turns.is_empty() {
        return None;
    }

    let messages = model_messages_from_turns(&thread.turns);
    let last_assistant_item_id = last_assistant_item_id_from_turns(&thread.turns);
    let state = RociThreadState {
        thread,
        messages,
        last_assistant_item_id,
    };
    persist_thread_snapshot(
        store,
        thread_id,
        Some(PersistedThreadSnapshot::from_thread_state(&state)),
    );
    Some(state)
}

fn apply_raw_event_to_thread(
    thread: &mut RociThread,
    turn_indices: &mut HashMap<String, usize>,
    event: &ChatRawEventRecord,
) {
    let params = &event.params;
    let turn_id = raw_event_turn_id(params);

    match event.method.as_str() {
        "turn/started" => {
            if let Some(turn_id) = turn_id {
                ensure_turn(thread, turn_indices, &turn_id);
            }
        }
        "item/started" | "item/completed" => {
            let Some(turn_id) = turn_id else { return };
            let Some(item) = params.get("item").and_then(|v| v.as_object()) else {
                return;
            };
            let Some(item_id) = item.get("id").and_then(|v| v.as_str()) else {
                return;
            };
            let Some(item_type) = item.get("type").and_then(|v| v.as_str()) else {
                return;
            };
            let turn = ensure_turn(thread, turn_indices, &turn_id);
            match item_type {
                "userMessage" => {
                    let text = extract_user_item_text(item);
                    upsert_user_item(turn, item_id, text);
                }
                "agentMessage" => {
                    let text = item
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    upsert_assistant_item(turn, item_id, text, false);
                }
                "mcpToolCall" => {
                    let tool = item
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool")
                        .to_string();
                    let status = item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if event.method == "item/completed" {
                            "completed"
                        } else {
                            "running"
                        })
                        .to_string();
                    let input = item
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let result = item.get("result").cloned();
                    let is_error = item.get("error").and_then(|v| v.as_bool()).unwrap_or(false);
                    upsert_tool_item(turn, item_id, tool, status, input, result, is_error);
                }
                _ => {}
            }
        }
        "item/agentMessage/delta" | "chat.message.delta" => {
            let Some(turn_id) = turn_id else { return };
            let Some(item_id) = raw_event_item_id(params) else {
                return;
            };
            let delta = params
                .get("delta")
                .or_else(|| params.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if delta.is_empty() {
                return;
            }
            let turn = ensure_turn(thread, turn_indices, &turn_id);
            upsert_assistant_item(turn, &item_id, delta.to_string(), true);
        }
        _ => {}
    }
}

fn ensure_turn<'a>(
    thread: &'a mut RociThread,
    turn_indices: &mut HashMap<String, usize>,
    turn_id: &str,
) -> &'a mut RociTurn {
    if let Some(index) = turn_indices.get(turn_id).copied() {
        return &mut thread.turns[index];
    }
    let index = thread.turns.len();
    thread
        .turns
        .push(RociTurn::new(turn_id.to_string(), Vec::new()));
    turn_indices.insert(turn_id.to_string(), index);
    &mut thread.turns[index]
}

fn raw_event_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

fn raw_event_item_id(params: &Value) -> Option<String> {
    params
        .get("itemId")
        .or_else(|| params.get("item_id"))
        .or_else(|| params.get("item").and_then(|i| i.get("id")))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

fn extract_user_item_text(item: &serde_json::Map<String, Value>) -> String {
    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    let Some(parts) = item.get("content").and_then(|v| v.as_array()) else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| {
            if part
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|kind| kind.eq_ignore_ascii_case("text"))
            {
                return part.get("text").and_then(|v| v.as_str());
            }
            None
        })
        .collect::<Vec<_>>()
        .join("")
}

pub(super) fn persist_roci_raw_event(
    store: &Arc<dyn Store>,
    run_id: &str,
    thread_id: &str,
    method: &str,
    params: Value,
) {
    if let Err(error) = store.insert_chat_raw_event(run_id, thread_id, method, &params) {
        tracing::warn!(
            %thread_id,
            %method,
            "failed to persist roci raw event: {error}"
        );
    }
}

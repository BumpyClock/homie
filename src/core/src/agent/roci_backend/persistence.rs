use std::sync::Arc;

use roci::agent::{ImportedThread, ThreadSnapshot};
use roci::types::ModelMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::storage::Store;

const RUNTIME_THREAD_FORMAT: &str = "roci_agent_runtime_thread_snapshot";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedRuntimeThread {
    format: String,
    thread: ThreadSnapshot,
    model_messages: Vec<ModelMessage>,
}

pub(super) fn decode_persisted_runtime_thread(
    value: Value,
) -> Result<Option<ImportedThread>, String> {
    let Some(format) = value.get("format").and_then(Value::as_str) else {
        return Ok(None);
    };
    if format != RUNTIME_THREAD_FORMAT {
        return Ok(None);
    }
    let persisted: PersistedRuntimeThread =
        serde_json::from_value(value).map_err(|e| format!("decode runtime thread: {e}"))?;
    Ok(Some(ImportedThread {
        thread: persisted.thread,
        model_messages: persisted.model_messages,
    }))
}

pub(super) fn persist_runtime_thread(
    store: &Arc<dyn Store>,
    thread_id: &str,
    thread: ThreadSnapshot,
    model_messages: Vec<ModelMessage>,
) {
    let value = match serde_json::to_value(PersistedRuntimeThread {
        format: RUNTIME_THREAD_FORMAT.to_string(),
        thread,
        model_messages,
    }) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to encode roci runtime thread: {error}");
            return;
        }
    };
    if let Err(error) = store.upsert_chat_thread_state(thread_id, &value) {
        tracing::warn!(%thread_id, "failed to persist roci runtime thread: {error}");
    }
}

pub(super) fn delete_persisted_runtime_thread(store: &Arc<dyn Store>, thread_id: &str) {
    if let Err(error) = store.delete_chat_thread_state(thread_id) {
        tracing::warn!(%thread_id, "failed to delete roci runtime thread: {error}");
    }
}

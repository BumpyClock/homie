use std::collections::HashMap;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::runtime::PendingEntry;
use super::types::{CodexEvent, CodexRequestId};

/// Background task: reads JSONL lines from Codex stdout, routes responses to
/// pending waiters and notifications/requests to the event channel.
pub(super) async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    event_tx: mpsc::Sender<CodexEvent>,
    mut pending_rx: mpsc::Receiver<PendingEntry>,
) {
    let mut reader = BufReader::new(stdout);
    let mut line_buf = String::new();
    let mut pending: HashMap<u64, oneshot::Sender<Value>> = HashMap::new();

    loop {
        tokio::select! {
            entry = pending_rx.recv() => {
                match entry {
                    Some(e) => { pending.insert(e.id, e.tx); }
                    None => break,
                }
            }
            result = reader.read_line(&mut line_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(_) => {
                        dispatch_line(&line_buf, &mut pending, &event_tx);
                        line_buf.clear();
                    }
                    Err(e) => {
                        tracing::warn!("codex stdout read error: {e}");
                        break;
                    }
                }
            }
        }
    }

    tracing::debug!("codex reader loop exited");
}

/// Parse a JSONL line and route it to the correct destination.
pub(super) fn dispatch_line(
    line: &str,
    pending: &mut HashMap<u64, oneshot::Sender<Value>>,
    event_tx: &mpsc::Sender<CodexEvent>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    let obj: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("codex sent non-JSON line: {e}");
            return;
        }
    };

    let has_method = obj.get("method").and_then(|v| v.as_str()).is_some();
    let id_value = obj.get("id").and_then(|v| {
        if let Some(num) = v.as_u64() {
            Some(CodexRequestId::Number(num))
        } else if let Some(num) = v.as_i64() {
            if num >= 0 {
                Some(CodexRequestId::Number(num as u64))
            } else {
                None
            }
        } else {
            v.as_str()
                .map(|text| CodexRequestId::Text(text.to_string()))
        }
    });
    let numeric_id = match &id_value {
        Some(CodexRequestId::Number(n)) => Some(*n),
        Some(CodexRequestId::Text(s)) => s.parse::<u64>().ok(),
        None => None,
    };

    if !has_method {
        if let Some(resp_id) = numeric_id {
            if let Some(tx) = pending.remove(&resp_id) {
                let result = obj.get("result").cloned().unwrap_or_else(|| obj.clone());
                let _ = tx.send(result);
            } else {
                tracing::debug!(id = resp_id, "codex response with no waiter");
            }
        }
        return;
    }

    let method = obj["method"].as_str().unwrap_or_default().to_string();
    let params = obj.get("params").cloned();

    let event = CodexEvent {
        method,
        id: id_value,
        params,
    };

    if event_tx.try_send(event).is_err() {
        tracing::warn!("codex event channel full, dropping event");
    }
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

/// A notification or request received from the Codex app-server stdout.
#[derive(Debug, Clone)]
pub struct CodexEvent {
    pub method: String,
    pub id: Option<u64>,
    pub params: Option<Value>,
}

/// Manages a `codex app-server` child process, providing typed send/receive
/// over its JSONL stdio protocol.
///
/// # Lifecycle
///
/// 1. `spawn()` starts the process and a background reader task.
/// 2. `initialize()` performs the Codex handshake (initialize + initialized).
/// 3. `send_request()` sends a request and waits for the correlated response.
/// 4. `send_notification()` fires a notification (no response expected).
/// 5. Events/notifications from Codex flow through the `event_rx` channel
///    returned by `spawn()`.
///
/// # Example
///
/// ```ignore
/// let (process, event_rx) = CodexProcess::spawn().await?;
/// process.initialize().await?;
/// let result = process.send_request("thread/start", None).await?;
/// ```
pub struct CodexProcess {
    child: Child,
    stdin_tx: mpsc::Sender<String>,
    next_id: AtomicU64,
    pending_tx: mpsc::Sender<PendingEntry>,
    reader_task: Option<tokio::task::JoinHandle<()>>,
    writer_task: Option<tokio::task::JoinHandle<()>>,
}

/// Internal entry for registering a pending request waiter.
struct PendingEntry {
    id: u64,
    tx: oneshot::Sender<Value>,
}

impl CodexProcess {
    /// Spawn `codex app-server` and return the process handle plus an event
    /// receiver for notifications/requests from the Codex server.
    pub async fn spawn() -> Result<(Self, mpsc::Receiver<CodexEvent>), String> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("failed to spawn codex app-server: {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "codex stdout not captured".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "codex stdin not captured".to_string())?;

        let (event_tx, event_rx) = mpsc::channel::<CodexEvent>(256);
        let (pending_tx, pending_rx) = mpsc::channel::<PendingEntry>(64);
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(64);

        let reader_task = tokio::spawn(reader_loop(stdout, event_tx, pending_rx));
        let writer_task = tokio::spawn(writer_loop(stdin, stdin_rx));

        let process = Self {
            child,
            stdin_tx,
            next_id: AtomicU64::new(1),
            pending_tx,
            reader_task: Some(reader_task),
            writer_task: Some(writer_task),
        };

        Ok((process, event_rx))
    }

    /// Perform the Codex handshake: send `initialize`, wait for response,
    /// then send `initialized` notification.
    pub async fn initialize(&self) -> Result<Value, String> {
        let params = serde_json::json!({
            "clientInfo": {
                "name": "homie",
                "title": "Homie Gateway",
                "version": "0.1.0"
            }
        });
        let result = self.send_request("initialize", Some(params)).await?;
        self.send_notification("initialized", None).await?;
        Ok(result)
    }

    /// Send a JSON-RPC request and wait for the correlated response.
    pub async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let (tx, rx) = oneshot::channel();
        self.pending_tx
            .send(PendingEntry { id, tx })
            .await
            .map_err(|_| "reader task closed".to_string())?;

        let mut msg = serde_json::json!({
            "method": method,
            "id": id,
        });
        if let Some(p) = params {
            msg["params"] = p;
        }

        let line =
            serde_json::to_string(&msg).map_err(|e| format!("failed to serialize request: {e}"))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| "writer task closed".to_string())?;

        rx.await.map_err(|_| "response sender dropped".to_string())
    }

    /// Send a notification (no `id`, no response expected).
    pub async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), String> {
        let mut msg = serde_json::json!({ "method": method });
        if let Some(p) = params {
            msg["params"] = p;
        }

        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("failed to serialize notification: {e}"))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| "writer task closed".to_string())
    }

    /// Send a raw JSON-RPC response (used for approval replies).
    pub async fn send_response(&self, id: u64, result: Value) -> Result<(), String> {
        let msg = serde_json::json!({
            "id": id,
            "result": result,
        });

        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("failed to serialize response: {e}"))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| "writer task closed".to_string())
    }

    /// Shut down the process and background tasks.
    pub fn shutdown(&mut self) {
        if let Some(h) = self.reader_task.take() {
            h.abort();
        }
        if let Some(h) = self.writer_task.take() {
            h.abort();
        }
        let _ = self.child.start_kill();
    }
}

impl Drop for CodexProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Background task: reads JSONL lines from Codex stdout, routes responses to
/// pending waiters and notifications/requests to the event channel.
async fn reader_loop(
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
fn dispatch_line(
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
    let id = obj.get("id").and_then(|v| v.as_u64());

    if !has_method {
        if let Some(resp_id) = id {
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

    let event = CodexEvent { method, id, params };

    if event_tx.try_send(event).is_err() {
        tracing::warn!("codex event channel full, dropping event");
    }
}

/// Background task: serializes outbound lines to the Codex stdin pipe.
async fn writer_loop(mut stdin: tokio::process::ChildStdin, mut rx: mpsc::Receiver<String>) {
    while let Some(line) = rx.recv().await {
        let mut data = line.into_bytes();
        data.push(b'\n');
        if let Err(e) = stdin.write_all(&data).await {
            tracing::warn!("codex stdin write error: {e}");
            break;
        }
        if let Err(e) = stdin.flush().await {
            tracing::warn!("codex stdin flush error: {e}");
            break;
        }
    }

    tracing::debug!("codex writer loop exited");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_line_routes_response_to_pending_waiter() {
        let (tx, mut rx) = oneshot::channel();
        let mut pending = HashMap::new();
        pending.insert(42u64, tx);

        let (event_tx, _event_rx) = mpsc::channel(16);
        let line = r#"{"id":42,"result":{"ok":true}}"#;
        dispatch_line(line, &mut pending, &event_tx);

        let result = rx.try_recv().expect("should receive response");
        assert_eq!(result, serde_json::json!({"ok": true}));
        assert!(pending.is_empty());
    }

    #[test]
    fn dispatch_line_routes_notification_to_event_channel() {
        let mut pending = HashMap::new();
        let (event_tx, mut event_rx) = mpsc::channel(16);

        let line = r#"{"method":"turn/started","params":{"threadId":"t1"}}"#;
        dispatch_line(line, &mut pending, &event_tx);

        let event = event_rx.try_recv().expect("should receive event");
        assert_eq!(event.method, "turn/started");
        assert!(event.id.is_none());
        assert!(event.params.is_some());
    }

    #[test]
    fn dispatch_line_routes_request_from_codex_to_event_channel() {
        let mut pending = HashMap::new();
        let (event_tx, mut event_rx) = mpsc::channel(16);

        let line = r#"{"method":"item/commandExecution/requestApproval","id":7,"params":{"command":"rm -rf /"}}"#;
        dispatch_line(line, &mut pending, &event_tx);

        let event = event_rx
            .try_recv()
            .expect("should receive approval request");
        assert_eq!(event.method, "item/commandExecution/requestApproval");
        assert_eq!(event.id, Some(7));
    }

    #[test]
    fn dispatch_line_ignores_empty_and_whitespace() {
        let mut pending = HashMap::new();
        let (event_tx, mut event_rx) = mpsc::channel(16);

        dispatch_line("", &mut pending, &event_tx);
        dispatch_line("   \n", &mut pending, &event_tx);

        assert!(event_rx.try_recv().is_err());
    }

    #[test]
    fn dispatch_line_handles_malformed_json() {
        let mut pending = HashMap::new();
        let (event_tx, mut event_rx) = mpsc::channel(16);

        dispatch_line("not json at all", &mut pending, &event_tx);

        assert!(event_rx.try_recv().is_err());
    }

    #[test]
    fn dispatch_line_response_without_waiter_does_not_panic() {
        let mut pending: HashMap<u64, oneshot::Sender<Value>> = HashMap::new();
        let (event_tx, _event_rx) = mpsc::channel(16);

        let line = r#"{"id":999,"result":"orphan"}"#;
        dispatch_line(line, &mut pending, &event_tx);
    }
}

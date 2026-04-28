use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use crate::paths::{homie_home_dir, homie_skills_dir};

use super::reader::reader_loop;
use super::types::{CodexEvent, CodexRequestId};
use super::writer::writer_loop;

/// Internal entry for registering a pending request waiter.
pub(super) struct PendingEntry {
    pub(super) id: u64,
    pub(super) tx: oneshot::Sender<Value>,
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

#[derive(Clone)]
pub struct CodexResponseSender {
    stdin_tx: mpsc::Sender<String>,
}

impl CodexResponseSender {
    pub async fn send_response(&self, id: CodexRequestId, result: Value) -> Result<(), String> {
        let msg = serde_json::json!({
            "id": id.to_json(),
            "result": result,
        });

        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("failed to serialize response: {e}"))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| "writer task closed".to_string())
    }
}

impl CodexProcess {
    /// Spawn `codex app-server` and return the process handle plus an event
    /// receiver for notifications/requests from the Codex server.
    pub async fn spawn() -> Result<(Self, mpsc::Receiver<CodexEvent>), String> {
        let homie_dir = homie_home_dir()?;
        let _ = homie_skills_dir()?;
        let mut child = Command::new("codex")
            .arg("app-server")
            .current_dir(&homie_dir)
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
    pub async fn send_response(&self, id: CodexRequestId, result: Value) -> Result<(), String> {
        let msg = serde_json::json!({
            "id": id.to_json(),
            "result": result,
        });

        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("failed to serialize response: {e}"))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| "writer task closed".to_string())
    }

    pub fn response_sender(&self) -> CodexResponseSender {
        CodexResponseSender {
            stdin_tx: self.stdin_tx.clone(),
        }
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

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::Message as WsMessage;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response, StreamType};

use super::runtime::SessionRuntime;
use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{SessionStatus, Store, TerminalRecord};

/// Metadata for a running session, visible to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
}

/// Tracks one active session: runtime + output forwarding task.
struct ActiveSession {
    runtime: SessionRuntime,
    info: SessionInfo,
    /// Handle to the tokio task that forwards PTY output → outbound_tx.
    output_task: tokio::task::JoinHandle<()>,
}

/// Terminal service: manages PTY sessions for a single connection.
///
/// Each `TerminalService` is scoped to one WS connection. When the connection
/// drops, all sessions are cleaned up.
pub struct TerminalService {
    sessions: HashMap<Uuid, ActiveSession>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: Arc<dyn Store>,
}

impl TerminalService {
    pub fn new(outbound_tx: mpsc::Sender<OutboundMessage>, store: Arc<dyn Store>) -> Self {
        Self {
            sessions: HashMap::new(),
            outbound_tx,
            store,
        }
    }

    /// Check for exited sessions and return exit info.
    fn reap_exited(&mut self) -> Vec<(Uuid, u32)> {
        let mut exited = Vec::new();
        for (id, active) in &mut self.sessions {
            if let Some(code) = active.runtime.try_wait() {
                exited.push((*id, code));
            }
        }
        for (id, code) in &exited {
            // Persist exit status before removing.
            if let Some(active) = self.sessions.get(id) {
                let rec = TerminalRecord {
                    session_id: *id,
                    shell: active.info.shell.clone(),
                    cols: active.info.cols,
                    rows: active.info.rows,
                    started_at: active.info.started_at.clone(),
                    status: SessionStatus::Exited,
                    exit_code: Some(*code),
                };
                if let Err(e) = self.store.upsert_terminal(&rec) {
                    tracing::warn!(%id, "failed to persist terminal exit: {e}");
                }
            }
            self.remove_session(*id);
        }
        exited
    }

    // ── RPC handlers ─────────────────────────────────────────────────

    fn session_start(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (shell, cols, rows) = parse_start_params(&params);

        let pty_system = native_pty_system();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = match pty_system.openpty(size) {
            Ok(p) => p,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("failed to open pty: {e}"),
                )
            }
        };

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("failed to spawn: {e}"),
                )
            }
        };

        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("failed to take writer: {e}"),
                )
            }
        };

        let session_id = Uuid::new_v4();

        // Channel for reader thread → tokio output forwarder.
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let reader_handle =
            match SessionRuntime::spawn_reader(&*pair.master, session_id, output_tx, shutdown_rx) {
                Ok(h) => h,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("failed to spawn reader: {e}"),
                    )
                }
            };

        let runtime = SessionRuntime::new(
            session_id,
            pair.master,
            writer,
            child,
            reader_handle,
            shutdown_tx,
        );

        let info = SessionInfo {
            session_id,
            shell: shell.clone(),
            cols,
            rows,
            started_at: chrono_now(),
        };

        // Persist terminal session metadata.
        let rec = TerminalRecord {
            session_id,
            shell: info.shell.clone(),
            cols: info.cols,
            rows: info.rows,
            started_at: info.started_at.clone(),
            status: SessionStatus::Active,
            exit_code: None,
        };
        if let Err(e) = self.store.upsert_terminal(&rec) {
            tracing::warn!(%session_id, "failed to persist terminal start: {e}");
        }

        // Spawn a tokio task to forward PTY output as binary WS frames.
        let outbound = self.outbound_tx.clone();
        let output_task = tokio::spawn(forward_pty_output(session_id, output_rx, outbound));

        self.sessions.insert(
            session_id,
            ActiveSession {
                runtime,
                info: info.clone(),
                output_task,
            },
        );

        tracing::info!(%session_id, %shell, cols, rows, "session started");

        Response::success(req_id, json!({ "session_id": session_id }))
    }

    fn session_attach(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let session_id = match parse_session_id(&params) {
            Some(id) => id,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing or invalid session_id",
                )
            }
        };

        match self.sessions.get(&session_id) {
            Some(active) => Response::success(
                req_id,
                serde_json::to_value(&active.info).unwrap_or(json!({})),
            ),
            None => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
        }
    }

    fn session_resize(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (session_id, cols, rows) = match parse_resize_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing session_id, cols, or rows",
                )
            }
        };

        match self.sessions.get_mut(&session_id) {
            Some(active) => match active.runtime.resize(rows, cols) {
                Ok(()) => {
                    active.info.cols = cols;
                    active.info.rows = rows;
                    Response::success(req_id, json!({ "ok": true }))
                }
                Err(e) => Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("resize failed: {e}"),
                ),
            },
            None => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
        }
    }

    fn session_input(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (session_id, data) = match parse_input_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing session_id or data",
                )
            }
        };

        match self.sessions.get_mut(&session_id) {
            Some(active) => match active.runtime.write_input(data.as_bytes()) {
                Ok(()) => Response::success(req_id, json!({ "ok": true })),
                Err(e) => Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("input failed: {e}"),
                ),
            },
            None => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
        }
    }

    fn session_kill(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let session_id = match parse_session_id(&params) {
            Some(id) => id,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing or invalid session_id",
                )
            }
        };

        if let Some(active) = self.sessions.get(&session_id) {
            // Persist exited status before removing.
            let rec = TerminalRecord {
                session_id,
                shell: active.info.shell.clone(),
                cols: active.info.cols,
                rows: active.info.rows,
                started_at: active.info.started_at.clone(),
                status: SessionStatus::Exited,
                exit_code: None,
            };
            if let Err(e) = self.store.upsert_terminal(&rec) {
                tracing::warn!(%session_id, "failed to persist terminal kill: {e}");
            }
            self.remove_session(session_id);
            tracing::info!(%session_id, "session killed");
            Response::success(req_id, json!({ "ok": true }))
        } else {
            Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            )
        }
    }

    /// List all persisted terminal sessions from the store.
    fn session_list(&self, req_id: Uuid) -> Response {
        match self.store.list_terminals() {
            Ok(records) => {
                let sessions: Vec<Value> = records
                    .into_iter()
                    .map(|r| {
                        json!({
                            "session_id": r.session_id,
                            "shell": r.shell,
                            "cols": r.cols,
                            "rows": r.rows,
                            "started_at": r.started_at,
                            "status": r.status,
                            "exit_code": r.exit_code,
                        })
                    })
                    .collect();
                Response::success(req_id, json!({ "sessions": sessions }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("list failed: {e}"),
            ),
        }
    }

    fn remove_session(&mut self, id: Uuid) {
        if let Some(mut active) = self.sessions.remove(&id) {
            active.output_task.abort();
            active.runtime.shutdown();
        }
    }
}

impl ServiceHandler for TerminalService {
    fn namespace(&self) -> &str {
        "terminal"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let resp = match method {
            "terminal.session.start" => self.session_start(id, params),
            "terminal.session.attach" => self.session_attach(id, params),
            "terminal.session.resize" => self.session_resize(id, params),
            "terminal.session.input" => self.session_input(id, params),
            "terminal.session.kill" => self.session_kill(id, params),
            "terminal.session.list" => self.session_list(id),
            _ => Response::error(
                id,
                error_codes::METHOD_NOT_FOUND,
                format!("unknown method: {method}"),
            ),
        };
        Box::pin(async move { resp })
    }

    fn handle_binary(&mut self, frame: &BinaryFrame) {
        if frame.stream != StreamType::Stdin {
            tracing::debug!(
                session = %frame.session_id,
                stream = ?frame.stream,
                "ignoring non-stdin binary frame"
            );
            return;
        }
        if let Some(active) = self.sessions.get_mut(&frame.session_id) {
            if let Err(e) = active.runtime.write_input(&frame.payload) {
                tracing::warn!(session = %frame.session_id, "write_input error: {e}");
            }
        } else {
            tracing::debug!(session = %frame.session_id, "binary frame for unknown session");
        }
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        self.reap_exited()
            .into_iter()
            .map(|(session_id, exit_code)| ReapEvent {
                topic: "terminal.session.exit".into(),
                params: Some(json!({
                    "session_id": session_id,
                    "exit_code": exit_code,
                })),
            })
            .collect()
    }

    fn shutdown(&mut self) {
        // Mark all active sessions as inactive in storage on disconnect.
        for (id, active) in &self.sessions {
            let rec = TerminalRecord {
                session_id: *id,
                shell: active.info.shell.clone(),
                cols: active.info.cols,
                rows: active.info.rows,
                started_at: active.info.started_at.clone(),
                status: SessionStatus::Inactive,
                exit_code: None,
            };
            if let Err(e) = self.store.upsert_terminal(&rec) {
                tracing::warn!(%id, "failed to persist terminal disconnect: {e}");
            }
        }

        let ids: Vec<Uuid> = self.sessions.keys().copied().collect();
        for id in ids {
            self.remove_session(id);
        }
    }
}

impl Drop for TerminalService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ── Output forwarding task with backpressure ─────────────────────────

/// Forwards PTY output as binary WS frames.
///
/// Backpressure: uses `try_send` on the outbound channel. When the channel
/// is full (client is slow), frames are dropped and a warning is logged.
/// This prevents a fast PTY from blocking the entire connection.
async fn forward_pty_output(
    session_id: Uuid,
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
) {
    let mut dropped_frames: u64 = 0;

    while let Some(data) = output_rx.recv().await {
        let frame = BinaryFrame {
            session_id,
            stream: StreamType::Stdout,
            payload: data,
        };
        let encoded = frame.encode();
        match outbound_tx.try_send(OutboundMessage::raw(WsMessage::Binary(encoded.into()))) {
            Ok(()) => {
                if dropped_frames > 0 {
                    tracing::warn!(
                        session = %session_id,
                        dropped_frames,
                        "backpressure eased, resumed sending"
                    );
                    dropped_frames = 0;
                }
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                dropped_frames += 1;
                if dropped_frames == 1 || dropped_frames.is_multiple_of(100) {
                    tracing::warn!(
                        session = %session_id,
                        dropped_frames,
                        "backpressure: dropping PTY output frame"
                    );
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                break; // connection closed
            }
        }
    }
}

// ── Param parsing helpers ────────────────────────────────────────────

fn parse_start_params(params: &Option<Value>) -> (String, u16, u16) {
    let default_shell = detect_default_shell();
    let p = params.as_ref();
    let shell = p
        .and_then(|v| v.get("shell"))
        .and_then(|v| v.as_str())
        .unwrap_or(&default_shell)
        .to_string();
    let cols = p
        .and_then(|v| v.get("cols"))
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as u16;
    let rows = p
        .and_then(|v| v.get("rows"))
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u16;
    (shell, cols, rows)
}

fn parse_session_id(params: &Option<Value>) -> Option<Uuid> {
    params
        .as_ref()?
        .get("session_id")?
        .as_str()?
        .parse::<Uuid>()
        .ok()
}

fn parse_resize_params(params: &Option<Value>) -> Option<(Uuid, u16, u16)> {
    let p = params.as_ref()?;
    let session_id = p.get("session_id")?.as_str()?.parse::<Uuid>().ok()?;
    let cols = p.get("cols")?.as_u64()? as u16;
    let rows = p.get("rows")?.as_u64()? as u16;
    Some((session_id, cols, rows))
}

fn parse_input_params(params: &Option<Value>) -> Option<(Uuid, String)> {
    let p = params.as_ref()?;
    let session_id = p.get("session_id")?.as_str()?.parse::<Uuid>().ok()?;
    let data = p.get("data")?.as_str()?.to_string();
    Some((session_id, data))
}

fn detect_default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

fn chrono_now() -> String {
    // Simple ISO-8601 timestamp without pulling in chrono crate.
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response, StreamType};

use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::terminal::{TerminalError, TerminalRegistry};

/// Terminal service: manages session RPCs for a single connection.
pub struct TerminalService {
    registry: Arc<Mutex<TerminalRegistry>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    subscriber_id: Uuid,
    attached: HashSet<Uuid>,
}

impl TerminalService {
    pub fn new(
        subscriber_id: Uuid,
        registry: Arc<Mutex<TerminalRegistry>>,
        outbound_tx: mpsc::Sender<OutboundMessage>,
    ) -> Self {
        Self {
            registry,
            outbound_tx,
            subscriber_id,
            attached: HashSet::new(),
        }
    }

    fn session_start(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (shell, cols, rows) = parse_start_params(&params);
        let info = {
            let mut registry = self.registry.lock().unwrap();
            registry.start_session(
                shell,
                cols,
                rows,
                self.subscriber_id,
                self.outbound_tx.clone(),
            )
        };
        match info {
            Ok(info) => {
                self.attached.insert(info.session_id);
                Response::success(req_id, json!({ "session_id": info.session_id }))
            }
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
            Err(TerminalError::NotFound(_)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, "session not found")
            }
        }
    }

    fn session_attach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

        let info = {
            let mut registry = self.registry.lock().unwrap();
            registry.attach_session(session_id, self.subscriber_id, self.outbound_tx.clone())
        };

        match info {
            Ok(info) => {
                self.attached.insert(info.session_id);
                Response::success(req_id, serde_json::to_value(&info).unwrap_or(json!({})))
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
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

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.resize_session(session_id, cols, rows)
        };

        match result {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
        }
    }

    fn session_detach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

        if let Ok(mut registry) = self.registry.lock() {
            registry.detach_session(session_id, self.subscriber_id);
        }
        self.attached.remove(&session_id);
        Response::success(req_id, json!({ "ok": true }))
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

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.input_session(session_id, &data)
        };

        match result {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
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

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.kill_session(session_id)
        };

        match result {
            Ok(()) => {
                self.attached.remove(&session_id);
                Response::success(req_id, json!({ "ok": true }))
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
        }
    }

    fn session_remove(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.remove_record(session_id)
        };

        match result {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INVALID_PARAMS, msg)
            }
        }
    }

    fn tmux_list(&self, req_id: Uuid) -> Response {
        let result = {
            let registry = self.registry.lock().unwrap();
            registry.list_tmux_sessions()
        };

        match result {
            Ok((supported, sessions)) => Response::success(
                req_id,
                json!({ "supported": supported, "sessions": sessions }),
            ),
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                "session not found",
            ),
        }
    }

    fn tmux_attach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing params",
                )
            }
        };
        let session_name = match p.get("session_name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing session_name",
                )
            }
        };
        let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
        let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.attach_tmux_session(
                session_name,
                cols,
                rows,
                self.subscriber_id,
                self.outbound_tx.clone(),
            )
        };

        match result {
            Ok(info) => {
                self.attached.insert(info.session_id);
                Response::success(req_id, serde_json::to_value(&info).unwrap_or(json!({})))
            }
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                "session not found",
            ),
        }
    }

    fn tmux_kill(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing params",
                )
            }
        };
        let session_name = match p.get("session_name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing session_name",
                )
            }
        };

        let result = {
            let registry = self.registry.lock().unwrap();
            registry.kill_tmux_session(session_name)
        };

        match result {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                "session not found",
            ),
        }
    }

    fn session_list(&self, req_id: Uuid) -> Response {
        let records = {
            let registry = self.registry.lock().unwrap();
            registry.list_sessions()
        };

        match records {
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

    fn detach_all(&mut self) {
        let session_ids: Vec<Uuid> = self.attached.iter().copied().collect();
        for session_id in session_ids {
            if let Ok(mut registry) = self.registry.lock() {
                registry.detach_session(session_id, self.subscriber_id);
            }
            self.attached.remove(&session_id);
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
            "terminal.session.detach" => self.session_detach(id, params),
            "terminal.session.resize" => self.session_resize(id, params),
            "terminal.session.input" => self.session_input(id, params),
            "terminal.session.kill" => self.session_kill(id, params),
            "terminal.session.remove" => self.session_remove(id, params),
            "terminal.session.list" => self.session_list(id),
            "terminal.tmux.list" => self.tmux_list(id),
            "terminal.tmux.attach" => self.tmux_attach(id, params),
            "terminal.tmux.kill" => self.tmux_kill(id, params),
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
        if let Ok(mut registry) = self.registry.lock() {
            if let Err(TerminalError::NotFound(_)) = registry.input_binary(frame) {
                tracing::debug!(session = %frame.session_id, "binary frame for unknown session");
            }
        }
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        let events = {
            let mut registry = self.registry.lock().unwrap();
            registry.reap_exited()
        };
        events
    }

    fn shutdown(&mut self) {
        self.detach_all();
    }
}

impl Drop for TerminalService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

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

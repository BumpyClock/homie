use serde_json::{json, Value};
use uuid::Uuid;

use homie_protocol::{error_codes, Response};

use super::params::{
    parse_attach_params, parse_input_params, parse_resize_params, parse_session_id,
    parse_start_params,
};
use super::TerminalService;
use crate::debug_bytes::{contains_subseq, fmt_bytes, terminal_debug_enabled_for};
use crate::router::ReapEvent;
use crate::terminal::TerminalError;
impl TerminalService {
    pub(super) fn session_start(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (shell, cols, rows) = parse_start_params(&params);
        let info = {
            let mut registry = self.registry.lock().unwrap();
            registry.start_session(shell, cols, rows)
        };
        match info {
            Ok(info) => {
                let _ = self.event_tx.send(ReapEvent::new(
                    "terminal.session.start",
                    Some(json!({
                        "session_id": info.session_id,
                        "name": info.name,
                        "shell": info.shell,
                        "cols": info.cols,
                        "rows": info.rows,
                        "started_at": info.started_at,
                    })),
                ));
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

    pub(super) fn session_attach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (session_id, replay, max_bytes) = match parse_attach_params(&params) {
            Some(v) => v,
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
            registry.attach_session(
                session_id,
                self.subscriber_id,
                self.outbound_tx.clone(),
                replay,
                max_bytes,
            )
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

    pub(super) fn session_resize(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    pub(super) fn session_detach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    pub(super) fn session_input(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

        if terminal_debug_enabled_for(session_id) {
            let bytes = data.as_bytes();
            let has_esc = contains_subseq(bytes, b"\x1b[");
            let dsr_reply = has_esc && bytes.ends_with(b"R");
            tracing::info!(
                session = %session_id,
                esc = has_esc,
                dsr_reply = dsr_reply,
                msg = %fmt_bytes(bytes, 80),
                "terminal ws in text stdin"
            );
        }

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

    pub(super) fn session_kill(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    pub(super) fn session_remove(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    pub(super) fn session_rename(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };
        let session_id = match p.get("session_id").and_then(|v| v.as_str()) {
            Some(v) => v.parse::<Uuid>().ok(),
            None => None,
        };
        let session_id = match session_id {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing or invalid session_id",
                )
            }
        };
        let name = match p.get("name") {
            Some(Value::String(v)) => Some(v.clone()),
            Some(Value::Null) => None,
            Some(_) => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "name must be a string or null",
                )
            }
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing name"),
        };

        let name_for_event = name.clone();
        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.rename_session(session_id, name)
        };

        match result {
            Ok(()) => {
                let _ = self.event_tx.send(ReapEvent::new(
                    "terminal.session.rename",
                    Some(json!({ "session_id": session_id, "name": name_for_event })),
                ));
                Response::success(req_id, json!({ "ok": true }))
            }
            Err(TerminalError::NotFound(_)) => Response::error(
                req_id,
                error_codes::SESSION_NOT_FOUND,
                format!("session not found: {session_id}"),
            ),
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::INVALID_PARAMS, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
        }
    }

    pub(super) fn tmux_list(&self, req_id: Uuid) -> Response {
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
            Err(TerminalError::NotFound(_)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, "session not found")
            }
        }
    }

    pub(super) fn tmux_attach(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };
        let session_name = match p.get("session_name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing session_name")
            }
        };
        let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
        let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;

        let result = {
            let mut registry = self.registry.lock().unwrap();
            registry.attach_tmux_session(session_name, cols, rows)
        };

        match result {
            Ok(info) => {
                let _ = self.event_tx.send(ReapEvent::new(
                    "terminal.session.start",
                    Some(json!({
                        "session_id": info.session_id,
                        "name": info.name,
                        "shell": info.shell,
                        "cols": info.cols,
                        "rows": info.rows,
                        "started_at": info.started_at,
                    })),
                ));
                Response::success(req_id, serde_json::to_value(&info).unwrap_or(json!({})))
            }
            Err(TerminalError::Missing(msg)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, msg)
            }
            Err(TerminalError::Internal(msg)) => {
                Response::error(req_id, error_codes::INTERNAL_ERROR, msg)
            }
            Err(TerminalError::NotFound(_)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, "session not found")
            }
        }
    }

    pub(super) fn tmux_kill(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };
        let session_name = match p.get("session_name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing session_name")
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
            Err(TerminalError::NotFound(_)) => {
                Response::error(req_id, error_codes::SESSION_NOT_FOUND, "session not found")
            }
        }
    }

    pub(super) fn session_list(&self, req_id: Uuid) -> Response {
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
                            "name": r.name,
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

    pub(super) fn session_preview(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let p = match params {
            Some(v) => v,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };
        let session_id = match p.get("session_id").and_then(|v| v.as_str()) {
            Some(v) => v.parse::<Uuid>().ok(),
            None => None,
        };
        let session_id = match session_id {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing or invalid session_id",
                )
            }
        };
        let max_bytes = p.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(65536) as usize;

        let result = {
            let registry = self.registry.lock().unwrap();
            registry.preview_session(session_id, max_bytes)
        };

        match result {
            Ok(text) => Response::success(req_id, json!({ "text": text })),
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
}

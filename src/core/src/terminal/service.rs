use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

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
    event_tx: tokio::sync::broadcast::Sender<ReapEvent>,
    attached: HashSet<Uuid>,
}

impl TerminalService {
    pub fn new(
        subscriber_id: Uuid,
        registry: Arc<Mutex<TerminalRegistry>>,
        outbound_tx: mpsc::Sender<OutboundMessage>,
        event_tx: tokio::sync::broadcast::Sender<ReapEvent>,
    ) -> Self {
        Self {
            registry,
            outbound_tx,
            subscriber_id,
            event_tx,
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

    fn session_rename(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing name",
                )
            }
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

    fn session_preview(&self, req_id: Uuid, params: Option<Value>) -> Response {
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
            "terminal.session.rename" => self.session_rename(id, params),
            "terminal.session.list" => self.session_list(id),
            "terminal.session.preview" => self.session_preview(id, params),
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
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED.get_or_init(detect_default_shell_uncached).clone()
}

fn detect_default_shell_uncached() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Some(pwsh) = detect_latest_pwsh() {
            return pwsh;
        }
        if let Some(powershell) = where_first("powershell.exe") {
            return powershell;
        }
        return std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

#[cfg(target_os = "windows")]
fn detect_latest_pwsh() -> Option<String> {
    let mut candidates = Vec::new();

    // Common install locations (PATH may not include pwsh.exe for services/daemons).
    candidates.extend(common_pwsh_paths());
    candidates.extend(where_all("pwsh.exe"));

    let candidates = dedupe_preserve_order(candidates);
    if candidates.is_empty() {
        return None;
    }

    let mut best: Option<(String, SemVer)> = None;
    let mut fallback: Option<String> = None;

    for path in candidates {
        if fallback.is_none() {
            fallback = Some(path.clone());
        }
        let Some(ver) = probe_pwsh_version(&path) else {
            continue;
        };
        match &best {
            None => best = Some((path, ver)),
            Some((_, best_ver)) => {
                if ver > *best_ver {
                    best = Some((path, ver));
                }
            }
        }
    }

    best.map(|(path, _)| path).or(fallback)
}

#[cfg(target_os = "windows")]
fn common_pwsh_paths() -> Vec<String> {
    use std::path::PathBuf;

    let mut out = Vec::new();

    let program_files = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or_else(|_| r"C:\Program Files".to_string());

    let program_files_x86 = std::env::var("ProgramFiles(x86)")
        .unwrap_or_else(|_| r"C:\Program Files (x86)".to_string());

    let candidates = [
        PathBuf::from(&program_files).join(r"PowerShell\7\pwsh.exe"),
        PathBuf::from(&program_files).join(r"PowerShell\7-preview\pwsh.exe"),
        PathBuf::from(&program_files).join(r"PowerShell\6\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\7\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\7-preview\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\6\pwsh.exe"),
    ];

    for path in candidates {
        if path.exists() {
            out.push(path.to_string_lossy().to_string());
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn where_first(exe: &str) -> Option<String> {
    where_all(exe).into_iter().next()
}

#[cfg(target_os = "windows")]
fn where_all(exe: &str) -> Vec<String> {
    use std::process::Command;

    let output = match Command::new("where").arg(exe).output() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(target_os = "windows")]
fn probe_pwsh_version(pwsh_path: &str) -> Option<SemVer> {
    use std::process::{Command, Stdio};

    // Prefer PSSemVer when available (includes preview tags). Fall back to PSVersion.
    let script = "$v=$PSVersionTable.PSSemVer;if($null -ne $v){$v.ToString()}else{$PSVersionTable.PSVersion.ToString()}";

    let mut child = Command::new(pwsh_path)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    // Guard against App Execution Aliases opening the Store / hanging.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => return None,
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(ver) = SemVer::parse(line.trim()) {
            return Some(ver);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for v in values {
        let key = v.to_lowercase();
        if seen.insert(key) {
            out.push(v);
        }
    }
    out
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<SemVerIdent>,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum SemVerIdent {
    Numeric(u64),
    Alpha(String),
}

#[cfg(any(target_os = "windows", test))]
impl SemVer {
    fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if s.is_empty() {
            return None;
        }

        let s = s.split('+').next().unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };

        let mut nums = core.split('.').map(|p| p.trim()).filter(|p| !p.is_empty());
        let major = nums.next()?.parse::<u64>().ok()?;
        let minor = nums.next().unwrap_or("0").parse::<u64>().ok()?;
        let patch = nums.next().unwrap_or("0").parse::<u64>().ok()?;

        let mut pre_idents = Vec::new();
        if let Some(pre) = pre {
            for part in pre.split('.').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                if part.chars().all(|c| c.is_ascii_digit()) {
                    pre_idents.push(SemVerIdent::Numeric(part.parse::<u64>().ok()?));
                } else {
                    pre_idents.push(SemVerIdent::Alpha(part.to_string()));
                }
            }
        }

        Some(Self {
            major,
            minor,
            patch,
            pre: pre_idents,
        })
    }
}

#[cfg(any(target_os = "windows", test))]
impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(any(target_os = "windows", test))]
impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Stable beats pre-release when core is equal.
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            (false, false) => {}
        }

        for (a, b) in self.pre.iter().zip(other.pre.iter()) {
            match a.cmp(b) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }

        // If all shared identifiers match, longer pre-release is higher precedence.
        self.pre.len().cmp(&other.pre.len())
    }
}

#[cfg(any(target_os = "windows", test))]
impl PartialOrd for SemVerIdent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(any(target_os = "windows", test))]
impl Ord for SemVerIdent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (self, other) {
            (SemVerIdent::Numeric(a), SemVerIdent::Numeric(b)) => a.cmp(b),
            (SemVerIdent::Numeric(_), SemVerIdent::Alpha(_)) => Ordering::Less,
            (SemVerIdent::Alpha(_), SemVerIdent::Numeric(_)) => Ordering::Greater,
            (SemVerIdent::Alpha(a), SemVerIdent::Alpha(b)) => a.cmp(b),
        }
    }
}

#[cfg(test)]
mod semver_tests {
    use super::SemVer;

    #[test]
    fn semver_parse_and_ordering() {
        let stable = SemVer::parse("7.4.2").unwrap();
        let older = SemVer::parse("7.4.1").unwrap();
        let preview = SemVer::parse("7.5.0-preview.1").unwrap();
        let rc = SemVer::parse("7.5.0-rc.1").unwrap();

        assert!(stable > older);
        assert!(preview > stable);
        assert!(rc > preview);
        assert!(SemVer::parse("7.5.0").unwrap() > rc);
    }
}

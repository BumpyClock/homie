use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::extract::ws::Message as WsMessage;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;
use std::process::Command;

use homie_protocol::{BinaryFrame, StreamType};
use crate::debug_bytes::{contains_subseq, fmt_bytes, terminal_debug_enabled_for};

use super::runtime::SessionRuntime;
use crate::outbound::OutboundMessage;
use crate::router::ReapEvent;
use crate::storage::{SessionStatus, Store, TerminalRecord};

const HISTORY_CHUNK_BYTES: usize = 16 * 1024;
const DEFAULT_HISTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
}

#[derive(Debug)]
pub enum TerminalError {
    NotFound(Uuid),
    Missing(String),
    Internal(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct TmuxSessionInfo {
    pub name: String,
    pub windows: u32,
    pub attached: bool,
}

struct ActiveSession {
    runtime: SessionRuntime,
    info: SessionInfo,
    output_task: tokio::task::JoinHandle<()>,
    subscribers: Arc<Mutex<HashMap<Uuid, mpsc::Sender<OutboundMessage>>>>,
    history: Arc<Mutex<HistoryBuffer>>,
}

struct HistoryBuffer {
    data: VecDeque<u8>,
    max_bytes: usize,
}

impl HistoryBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_bytes,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if self.max_bytes == 0 || chunk.is_empty() {
            return;
        }
        if chunk.len() >= self.max_bytes {
            self.data.clear();
            self.data
                .extend(chunk[chunk.len() - self.max_bytes..].iter().copied());
            return;
        }
        while self.data.len() + chunk.len() > self.max_bytes {
            self.data.pop_front();
        }
        self.data.extend(chunk.iter().copied());
    }

    fn snapshot(&self) -> Vec<u8> {
        if self.data.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.data.len());
        out.extend(self.data.iter().copied());
        out
    }
}

pub struct TerminalRegistry {
    sessions: HashMap<Uuid, ActiveSession>,
    store: Arc<dyn Store>,
}

impl TerminalRegistry {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            sessions: HashMap::new(),
            store,
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<TerminalRecord>, String> {
        self.store.list_terminals().map_err(|e| e.to_string())
    }

    pub fn start_session(
        &mut self,
        shell: String,
        cols: u16,
        rows: u16,
    ) -> Result<SessionInfo, TerminalError> {
        let (display_shell, cmd) = build_shell_command(&shell);
        self.start_session_with_command(
            display_shell,
            cmd,
            cols,
            rows,
            None,
        )
    }

    pub fn list_tmux_sessions(&self) -> Result<(bool, Vec<TmuxSessionInfo>), TerminalError> {
        if !tmux_supported() {
            return Ok((false, Vec::new()));
        }
        let output = Command::new("tmux")
            .args([
                "list-sessions",
                "-F",
                "#{session_name}|#{session_windows}|#{session_attached}",
            ])
            .output()
            .map_err(|e| TerminalError::Internal(format!("tmux list failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let lowered = stderr.to_lowercase();
            if lowered.contains("no server running") || lowered.contains("no sessions") {
                return Ok((true, Vec::new()));
            }
            return Err(TerminalError::Internal(format!(
                "tmux list failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();
        for line in stdout.lines() {
            let mut parts = line.split('|');
            let name = match parts.next() {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => continue,
            };
            let windows = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let attached = parts
                .next()
                .map(|v| v == "1")
                .unwrap_or(false);
            sessions.push(TmuxSessionInfo {
                name,
                windows,
                attached,
            });
        }
        Ok((true, sessions))
    }

    pub fn attach_tmux_session(
        &mut self,
        session_name: String,
        cols: u16,
        rows: u16,
    ) -> Result<SessionInfo, TerminalError> {
        if !tmux_supported() {
            return Err(TerminalError::Internal("tmux not supported".into()));
        }
        if !tmux_has_session(&session_name)? {
            return Err(TerminalError::Missing(format!(
                "tmux session not found: {session_name}"
            )));
        }
        let mut cmd = CommandBuilder::new("tmux");
        cmd.arg("attach");
        cmd.arg("-t");
        cmd.arg(&session_name);
        let display = format!("tmux:{session_name}");
        self.start_session_with_command(
            display,
            cmd,
            cols,
            rows,
            Some(session_name),
        )
    }

    pub fn kill_tmux_session(&self, session_name: String) -> Result<(), TerminalError> {
        if !tmux_supported() {
            return Err(TerminalError::Internal("tmux not supported".into()));
        }
        let output = Command::new("tmux")
            .args(["kill-session", "-t", &session_name])
            .output()
            .map_err(|e| TerminalError::Internal(format!("tmux kill failed: {e}")))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lowered = stderr.to_lowercase();
        if lowered.contains("no server running") || lowered.contains("no sessions") {
            return Err(TerminalError::Missing(format!(
                "tmux session not found: {session_name}"
            )));
        }
        Err(TerminalError::Internal(format!(
            "tmux kill failed: {stderr}"
        )))
    }

    fn start_session_with_command(
        &mut self,
        display_shell: String,
        mut cmd: CommandBuilder,
        cols: u16,
        rows: u16,
        name: Option<String>,
    ) -> Result<SessionInfo, TerminalError> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| TerminalError::Internal(format!("failed to open pty: {e}")))?;

        if cfg!(not(target_os = "windows")) {
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Internal(format!("failed to spawn: {e}")))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TerminalError::Internal(format!("failed to take writer: {e}")))?;

        let session_id = Uuid::new_v4();

        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let reader_handle = SessionRuntime::spawn_reader(
            &*pair.master,
            session_id,
            output_tx,
            shutdown_rx,
        )
        .map_err(|e| TerminalError::Internal(format!("failed to spawn reader: {e}")))?;

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
            name: name.clone(),
            shell: display_shell.clone(),
            cols,
            rows,
            started_at: chrono_now(),
        };

        let rec = TerminalRecord {
            session_id,
            name,
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

        let subscribers = Arc::new(Mutex::new(HashMap::new()));
        let history = Arc::new(Mutex::new(HistoryBuffer::new(history_limit_bytes())));
        let output_task = tokio::spawn(forward_pty_output(
            session_id,
            output_rx,
            subscribers.clone(),
            history.clone(),
        ));

        self.sessions.insert(
            session_id,
            ActiveSession {
                runtime,
                info: info.clone(),
                output_task,
                subscribers,
                history,
            },
        );

        tracing::info!(%session_id, shell = %display_shell, cols, rows, "session started");
        Ok(info)
    }

    pub fn attach_session(
        &mut self,
        session_id: Uuid,
        subscriber_id: Uuid,
        outbound_tx: mpsc::Sender<OutboundMessage>,
        replay: bool,
        max_bytes: usize,
    ) -> Result<SessionInfo, TerminalError> {
        let (info, history, should_replay) = {
            let active = self
                .sessions
                .get_mut(&session_id)
                .ok_or(TerminalError::NotFound(session_id))?;
            let mut subs = active.subscribers.lock().unwrap();
            let already_attached = subs.contains_key(&subscriber_id);
            subs.insert(subscriber_id, outbound_tx.clone());
            drop(subs);
            (
                active.info.clone(),
                active.history.clone(),
                replay || !already_attached,
            )
        };
        if should_replay {
            let snapshot = history.lock().unwrap().snapshot();
            if !snapshot.is_empty() {
                let slice = if max_bytes > 0 && snapshot.len() > max_bytes {
                    snapshot[snapshot.len() - max_bytes..].to_vec()
                } else {
                    snapshot
                };
                let session_id = info.session_id;
                tokio::spawn(async move {
                    for chunk in slice.chunks(HISTORY_CHUNK_BYTES) {
                        if terminal_debug_enabled_for(session_id) {
                            tracing::info!(
                                session = %session_id,
                                msg = %fmt_bytes(chunk, 80),
                                "terminal replay chunk"
                            );
                        }
                        let frame = BinaryFrame {
                            session_id,
                            stream: StreamType::Stdout,
                            payload: chunk.to_vec(),
                        };
                        if outbound_tx
                            .send(OutboundMessage::raw(WsMessage::Binary(
                                frame.encode().into(),
                            )))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
        }
        self.persist_status(&info, SessionStatus::Active, None);
        Ok(info)
    }

    pub fn detach_session(&mut self, session_id: Uuid, subscriber_id: Uuid) {
        if let Some(active) = self.sessions.get_mut(&session_id) {
            let mut subs = active.subscribers.lock().unwrap();
            subs.remove(&subscriber_id);
            drop(subs);
        }
    }

    pub fn resize_session(
        &mut self,
        session_id: Uuid,
        cols: u16,
        rows: u16,
    ) -> Result<(), TerminalError> {
        let active = self
            .sessions
            .get_mut(&session_id)
            .ok_or(TerminalError::NotFound(session_id))?;
        active
            .runtime
            .resize(rows, cols)
            .map_err(|e| TerminalError::Internal(format!("resize failed: {e}")))?;
        active.info.cols = cols;
        active.info.rows = rows;
        Ok(())
    }

    pub fn input_session(&mut self, session_id: Uuid, data: &str) -> Result<(), TerminalError> {
        let active = self
            .sessions
            .get_mut(&session_id)
            .ok_or(TerminalError::NotFound(session_id))?;
        active
            .runtime
            .write_input(data.as_bytes())
            .map_err(|e| TerminalError::Internal(format!("write_input failed: {e}")))?;
        Ok(())
    }

    pub fn input_binary(&mut self, frame: &BinaryFrame) -> Result<(), TerminalError> {
        if frame.stream != StreamType::Stdin {
            return Ok(());
        }
        let active = self
            .sessions
            .get_mut(&frame.session_id)
            .ok_or(TerminalError::NotFound(frame.session_id))?;
        active
            .runtime
            .write_input(&frame.payload)
            .map_err(|e| TerminalError::Internal(format!("write_input failed: {e}")))?;
        Ok(())
    }

    pub fn kill_session(&mut self, session_id: Uuid) -> Result<(), TerminalError> {
        let active = self
            .sessions
            .get(&session_id)
            .ok_or(TerminalError::NotFound(session_id))?;
        let rec = TerminalRecord {
            session_id,
            name: active.info.name.clone(),
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
        Ok(())
    }

    pub fn reap_exited(&mut self) -> Vec<ReapEvent> {
        let mut exited = Vec::new();
        for (id, active) in &mut self.sessions {
            if let Some(code) = active.runtime.try_wait() {
                exited.push((*id, code));
            }
        }
        for (id, code) in &exited {
            if let Some(active) = self.sessions.get(id) {
                let rec = TerminalRecord {
                    session_id: *id,
                    name: active.info.name.clone(),
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
            .into_iter()
            .map(|(session_id, exit_code)| {
                ReapEvent::new(
                    "terminal.session.exit",
                    Some(json!({
                        "session_id": session_id,
                        "exit_code": exit_code,
                    })),
                )
            })
            .collect()
    }

    pub fn remove_record(&mut self, session_id: Uuid) -> Result<(), TerminalError> {
        if self.sessions.contains_key(&session_id) {
            return Err(TerminalError::Internal(
                "session is active; kill it first".into(),
            ));
        }
        self.store
            .delete_terminal(session_id)
            .map_err(TerminalError::Internal)?;
        Ok(())
    }

    pub fn rename_session(
        &mut self,
        session_id: Uuid,
        name: Option<String>,
    ) -> Result<(), TerminalError> {
        let trimmed = name.and_then(|value| {
            let next = value.trim();
            if next.is_empty() {
                None
            } else {
                Some(next.to_string())
            }
        });

        let active_info = if let Some(active) = self.sessions.get_mut(&session_id) {
            if is_tmux_shell(&active.info.shell) {
                return Err(TerminalError::Missing(
                    "tmux sessions cannot be renamed".into(),
                ));
            }
            active.info.name = trimmed.clone();
            Some(active.info.clone())
        } else {
            None
        };
        if let Some(info) = active_info {
            self.persist_status(&info, SessionStatus::Active, None);
            return Ok(());
        }

        let rec = self
            .store
            .get_terminal(session_id)
            .map_err(TerminalError::Internal)?;
        let mut rec = match rec {
            Some(value) => value,
            None => return Err(TerminalError::NotFound(session_id)),
        };
        if is_tmux_shell(&rec.shell) {
            return Err(TerminalError::Missing(
                "tmux sessions cannot be renamed".into(),
            ));
        }
        rec.name = trimmed;
        self.store.upsert_terminal(&rec).map_err(TerminalError::Internal)?;
        Ok(())
    }

    pub fn preview_session(
        &self,
        session_id: Uuid,
        max_bytes: usize,
    ) -> Result<String, TerminalError> {
        let active = self
            .sessions
            .get(&session_id)
            .ok_or(TerminalError::NotFound(session_id))?;
        let snapshot = active.history.lock().unwrap().snapshot();
        if snapshot.is_empty() {
            return Ok(String::new());
        }
        let slice = if max_bytes > 0 && snapshot.len() > max_bytes {
            &snapshot[snapshot.len() - max_bytes..]
        } else {
            snapshot.as_slice()
        };
        Ok(String::from_utf8_lossy(slice).to_string())
    }

    fn remove_session(&mut self, id: Uuid) {
        if let Some(mut active) = self.sessions.remove(&id) {
            active.output_task.abort();
            active.runtime.shutdown();
        }
    }

    fn persist_status(&self, info: &SessionInfo, status: SessionStatus, exit_code: Option<u32>) {
        let rec = TerminalRecord {
            session_id: info.session_id,
            name: info.name.clone(),
            shell: info.shell.clone(),
            cols: info.cols,
            rows: info.rows,
            started_at: info.started_at.clone(),
            status,
            exit_code,
        };
        if let Err(e) = self.store.upsert_terminal(&rec) {
            tracing::warn!(%info.session_id, "failed to persist terminal status: {e}");
        }
    }
}

async fn forward_pty_output(
    session_id: Uuid,
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    subscribers: Arc<Mutex<HashMap<Uuid, mpsc::Sender<OutboundMessage>>>>,
    history: Arc<Mutex<HistoryBuffer>>,
) {
    while let Some(data) = output_rx.recv().await {
        if terminal_debug_enabled_for(session_id) {
            let has_dsr = contains_subseq(&data, b"\x1b[6n") || contains_subseq(&data, b"[6n");
            tracing::info!(
                session = %session_id,
                dsr = has_dsr,
                msg = %fmt_bytes(&data, 80),
                "terminal pty out"
            );
        }
        if let Ok(mut buffer) = history.lock() {
            buffer.push(&data);
        }
        let frame = BinaryFrame {
            session_id,
            stream: StreamType::Stdout,
            payload: data,
        };
        let encoded = frame.encode();
        let mut to_remove = Vec::new();
        let targets: Vec<(Uuid, mpsc::Sender<OutboundMessage>)> = {
            let guard = subscribers.lock().unwrap();
            guard.iter().map(|(id, tx)| (*id, tx.clone())).collect()
        };
        for (id, tx) in targets {
            if tx
                .send(OutboundMessage::raw(WsMessage::Binary(encoded.clone().into())))
                .await
                .is_err()
            {
                to_remove.push(id);
            }
        }
        if !to_remove.is_empty() {
            let mut guard = subscribers.lock().unwrap();
            for id in to_remove {
                guard.remove(&id);
            }
        }
    }
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

fn history_limit_bytes() -> usize {
    std::env::var("HOMIE_HISTORY_BYTES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_HISTORY_BYTES)
}

fn tmux_supported() -> bool {
    if cfg!(target_os = "windows") {
        return false;
    }
    Command::new("tmux").arg("-V").output().is_ok()
}

fn is_tmux_shell(shell: &str) -> bool {
    shell.starts_with("tmux:")
}

fn tmux_has_session(session_name: &str) -> Result<bool, TerminalError> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map_err(|e| TerminalError::Internal(format!("tmux has-session failed: {e}")))?;
    if output.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lowered = stderr.to_lowercase();
    if lowered.contains("no server running") || lowered.contains("no sessions") {
        return Ok(false);
    }
    Ok(false)
}

fn build_shell_command(shell: &str) -> (String, CommandBuilder) {
    #[cfg(target_os = "windows")]
    {
        let raw = shell.trim();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(raw);

        let lower = unquoted.to_ascii_lowercase();
        let marker = "cmd.exe";
        if let Some(pos) = lower.find(marker) {
            let exe = unquoted[..pos + marker.len()].trim().to_string();
            let rest = unquoted[pos + marker.len()..].trim();

            // Special-case: allow "cmd.exe /d" to be passed as a single string (common mistake).
            // Also default to "/d" for cmd to avoid AutoRun side effects.
            if rest.is_empty() || rest.eq_ignore_ascii_case("/d") {
                let mut cmd = CommandBuilder::new(&exe);
                cmd.arg("/d");
                return (format!("{exe} /d"), cmd);
            }
        }

        (shell.to_string(), CommandBuilder::new(shell))
    }

    #[cfg(not(target_os = "windows"))]
    {
        (shell.to_string(), CommandBuilder::new(shell))
    }
}

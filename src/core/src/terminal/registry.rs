use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::outbound::OutboundMessage;
use crate::router::ReapEvent;
use crate::storage::{SessionStatus, Store, TerminalRecord};
use homie_protocol::{BinaryFrame, StreamType};

use super::runtime::SessionRuntime;

mod history;
mod output;
mod shell;
mod time;
mod tmux;
mod types;

use history::{history_limit_bytes, HistoryBuffer};
use output::{forward_pty_output, replay_history};
use shell::build_shell_command;
use time::chrono_now;
use tmux::is_tmux_shell;
pub use types::{SessionInfo, TerminalError, TmuxSessionInfo};

struct ActiveSession {
    runtime: SessionRuntime,
    info: SessionInfo,
    output_task: tokio::task::JoinHandle<()>,
    subscribers: Arc<Mutex<HashMap<Uuid, mpsc::Sender<OutboundMessage>>>>,
    history: Arc<Mutex<HistoryBuffer>>,
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
        self.start_session_with_command(display_shell, cmd, cols, rows, None)
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

        let reader_handle =
            SessionRuntime::spawn_reader(&*pair.master, session_id, output_tx, shutdown_rx)
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
            replay_history(info.session_id, snapshot, max_bytes, outbound_tx);
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
        self.store
            .upsert_terminal(&rec)
            .map_err(TerminalError::Internal)?;
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

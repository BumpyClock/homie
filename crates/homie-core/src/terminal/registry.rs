use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::extract::ws::Message as WsMessage;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{BinaryFrame, StreamType};

use super::runtime::SessionRuntime;
use crate::outbound::OutboundMessage;
use crate::router::ReapEvent;
use crate::storage::{SessionStatus, Store, TerminalRecord};

const HISTORY_CHUNK_BYTES: usize = 16 * 1024;
const DEFAULT_HISTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub started_at: String,
}

#[derive(Debug)]
pub enum TerminalError {
    NotFound(Uuid),
    Internal(String),
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
        subscriber_id: Uuid,
        outbound_tx: mpsc::Sender<OutboundMessage>,
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

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

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
            shell: shell.clone(),
            cols,
            rows,
            started_at: chrono_now(),
        };

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

        let subscribers = Arc::new(Mutex::new(HashMap::from([(
            subscriber_id,
            outbound_tx.clone(),
        )])));
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

        tracing::info!(%session_id, %shell, cols, rows, "session started");
        Ok(info)
    }

    pub fn attach_session(
        &mut self,
        session_id: Uuid,
        subscriber_id: Uuid,
        outbound_tx: mpsc::Sender<OutboundMessage>,
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
            (active.info.clone(), active.history.clone(), !already_attached)
        };
        if should_replay {
            let snapshot = history.lock().unwrap().snapshot();
            if !snapshot.is_empty() {
                let session_id = info.session_id;
                tokio::spawn(async move {
                    for chunk in snapshot.chunks(HISTORY_CHUNK_BYTES) {
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
            .map(|(session_id, exit_code)| ReapEvent {
                topic: "terminal.session.exit".into(),
                params: Some(json!({
                    "session_id": session_id,
                    "exit_code": exit_code,
                })),
            })
            .collect()
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
    let mut dropped_frames: u64 = 0;

    while let Some(data) = output_rx.recv().await {
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
            match tx.try_send(OutboundMessage::raw(WsMessage::Binary(encoded.clone().into()))) {
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
                    to_remove.push(id);
                }
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

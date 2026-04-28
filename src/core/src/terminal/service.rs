use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response, StreamType};

use crate::debug_bytes::{fmt_bytes, terminal_debug_enabled_for};
use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::terminal::{TerminalError, TerminalRegistry};

mod handlers;
mod params;
mod shell;

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
        if terminal_debug_enabled_for(frame.session_id) {
            tracing::info!(
                session = %frame.session_id,
                stream = ?frame.stream,
                msg = %fmt_bytes(&frame.payload, 80),
                "terminal ws in binary stdin"
            );
        }
        if let Ok(mut registry) = self.registry.lock() {
            if let Err(TerminalError::NotFound(_)) = registry.input_binary(frame) {
                tracing::debug!(session = %frame.session_id, "binary frame for unknown session");
            }
        }
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        let mut registry = self.registry.lock().unwrap();
        registry.reap_exited()
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

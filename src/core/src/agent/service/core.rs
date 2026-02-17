use std::collections::HashMap;
use std::sync::Arc;

use crate::agent::process::{CodexEvent, CodexProcess};
use crate::agent::roci_backend::{ChatBackend, RociBackend};
use tokio::sync::mpsc;

use crate::outbound::OutboundMessage;
use crate::router::ReapEvent;
use crate::storage::{SessionStatus, Store};
use crate::{ExecPolicy, HomieConfig};

use super::events::event_forwarder_loop;

/// Chat core: bridges the Codex app-server to the Homie WS protocol.
///
/// Each WS connection gets its own core. A `CodexProcess` is started lazily
/// on the first chat request and killed on shutdown.
///
/// # Example interaction
///
/// Client sends:
/// ```json
/// {"type":"request","id":"...","method":"chat.create","params":{}}
/// ```
///
/// Service spawns Codex, runs the handshake, sends `thread/start`, and
/// returns `{"chat_id":"<thread_id>"}`.
pub(super) struct CodexChatCore {
    pub(super) backend: ChatBackend,
    pub(super) outbound_tx: mpsc::Sender<OutboundMessage>,
    pub(super) process: Option<CodexProcess>,
    pub(super) event_forwarder: Option<tokio::task::JoinHandle<()>>,
    pub(super) reap_events: Vec<ReapEvent>,
    pub(super) thread_ids: HashMap<String, String>,
    pub(super) store: Arc<dyn Store>,
    pub(super) homie_config: Arc<HomieConfig>,
    pub(super) exec_policy: Arc<ExecPolicy>,
    pub(super) roci: RociBackend,
}

impl CodexChatCore {
    pub(super) fn use_roci(&self) -> bool {
        matches!(self.backend, ChatBackend::Roci)
    }

    pub(super) fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        let backend = ChatBackend::from_env();
        let roci = RociBackend::new(
            outbound_tx.clone(),
            store.clone(),
            exec_policy.clone(),
            homie_config.clone(),
        );
        Self {
            backend,
            outbound_tx,
            process: None,
            event_forwarder: None,
            reap_events: Vec::new(),
            thread_ids: HashMap::new(),
            store,
            homie_config,
            exec_policy,
            roci,
        }
    }

    /// Ensure the Codex process is running; spawn + initialize if needed.
    pub(super) async fn ensure_process(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Ok(());
        }

        let (process, event_rx): (CodexProcess, mpsc::Receiver<CodexEvent>) =
            CodexProcess::spawn().await?;
        process.initialize().await?;

        let outbound = self.outbound_tx.clone();
        let store = self.store.clone();
        let exec_policy = self.exec_policy.clone();
        let homie_config = self.homie_config.clone();
        let response_sender = process.response_sender();
        let forwarder = tokio::spawn(event_forwarder_loop(
            event_rx,
            outbound,
            store,
            response_sender,
            exec_policy,
            homie_config,
        ));
        self.event_forwarder = Some(forwarder);
        self.process = Some(process);
        Ok(())
    }

    pub(super) fn reap(&mut self) -> Vec<ReapEvent> {
        std::mem::take(&mut self.reap_events)
    }

    pub(super) fn shutdown(&mut self) {
        // Mark all active chats as inactive in storage on disconnect.
        for chat_id in self.thread_ids.keys() {
            if let Ok(Some(mut rec)) = self.store.get_chat(chat_id) {
                if rec.status != SessionStatus::Active {
                    continue;
                }
                rec.status = SessionStatus::Inactive;
                if let Err(e) = self.store.upsert_chat(&rec) {
                    tracing::warn!(%chat_id, "failed to persist chat disconnect: {e}");
                }
            }
        }

        if let Some(h) = self.event_forwarder.take() {
            h.abort();
        }
        if let Some(mut p) = self.process.take() {
            p.shutdown();
        }
        if self.use_roci() {
            let roci = self.roci.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    roci.shutdown().await;
                });
            }
        }
    }

    pub(super) fn resolve_thread_id(&mut self, chat_id: &str, explicit: Option<&str>) -> Option<String> {
        if let Some(thread_id) = explicit {
            let thread_id = thread_id.to_string();
            self.thread_ids
                .insert(chat_id.to_string(), thread_id.clone());
            return Some(thread_id);
        }

        if let Some(thread_id) = self.thread_ids.get(chat_id) {
            return Some(thread_id.clone());
        }

        match self.store.get_chat(chat_id) {
            Ok(Some(rec)) if !rec.thread_id.is_empty() => {
                self.thread_ids
                    .insert(chat_id.to_string(), rec.thread_id.clone());
                Some(rec.thread_id)
            }
            _ => None,
        }
    }
}

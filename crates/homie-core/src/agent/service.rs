use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};

use super::process::{CodexEvent, CodexProcess};
use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{ChatRecord, SessionStatus, Store};

/// Maps Codex notification methods to Homie event topics.
fn codex_method_to_topic(method: &str) -> Option<&'static str> {
    match method {
        "item/agentMessage/delta" => Some("agent.chat.delta"),
        "item/started" => Some("agent.chat.item.started"),
        "item/completed" => Some("agent.chat.item.completed"),
        "turn/started" => Some("agent.chat.turn.started"),
        "turn/completed" => Some("agent.chat.turn.completed"),
        "item/commandExecution/outputDelta" => Some("agent.chat.command.output"),
        "item/fileChange/outputDelta" => Some("agent.chat.file.output"),
        "item/reasoning/summaryTextDelta" => Some("agent.chat.reasoning.delta"),
        "turn/diff/updated" => Some("agent.chat.diff.updated"),
        "turn/plan/updated" => Some("agent.chat.plan.updated"),
        "item/commandExecution/requestApproval" => Some("agent.chat.approval.required"),
        "item/fileChange/requestApproval" => Some("agent.chat.approval.required"),
        _ => None,
    }
}

/// Agent service: bridges the Codex app-server to the Homie WS protocol.
///
/// Each WS connection gets its own `AgentService`. A `CodexProcess` is
/// started lazily on the first `agent.chat.create` call and killed on
/// shutdown.
///
/// # Example interaction
///
/// Client sends:
/// ```json
/// {"type":"request","id":"...","method":"agent.chat.create","params":{}}
/// ```
///
/// Service spawns Codex, runs the handshake, sends `thread/start`, and
/// returns `{"chat_id":"<thread_id>"}`.
pub struct AgentService {
    outbound_tx: mpsc::Sender<OutboundMessage>,
    process: Option<CodexProcess>,
    event_forwarder: Option<tokio::task::JoinHandle<()>>,
    reap_events: Vec<ReapEvent>,
    thread_ids: HashMap<String, String>,
    store: Arc<dyn Store>,
}

impl AgentService {
    pub fn new(outbound_tx: mpsc::Sender<OutboundMessage>, store: Arc<dyn Store>) -> Self {
        Self {
            outbound_tx,
            process: None,
            event_forwarder: None,
            reap_events: Vec::new(),
            thread_ids: HashMap::new(),
            store,
        }
    }

    /// Ensure the Codex process is running; spawn + initialize if needed.
    async fn ensure_process(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Ok(());
        }

        let (process, event_rx) = CodexProcess::spawn().await?;
        process.initialize().await?;

        let outbound = self.outbound_tx.clone();
        let store = self.store.clone();
        let forwarder = tokio::spawn(event_forwarder_loop(event_rx, outbound, store));
        self.event_forwarder = Some(forwarder);
        self.process = Some(process);
        Ok(())
    }

    async fn chat_create(&mut self, req_id: Uuid) -> Response {
        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("thread/start", None).await {
            Ok(result) => {
                let thread_id = result
                    .get("threadId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let chat_id = if thread_id.is_empty() {
                    Uuid::new_v4().to_string()
                } else {
                    thread_id.clone()
                };
                self.thread_ids.insert(chat_id.clone(), thread_id.clone());

                // Persist chat metadata.
                let rec = ChatRecord {
                    chat_id: chat_id.clone(),
                    thread_id,
                    created_at: chrono_now(),
                    status: SessionStatus::Active,
                    event_pointer: 0,
                };
                if let Err(e) = self.store.upsert_chat(&rec) {
                    tracing::warn!(%chat_id, "failed to persist chat create: {e}");
                }

                Response::success(req_id, json!({ "chat_id": chat_id }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/start failed: {e}"),
            ),
        }
    }

    async fn chat_message_send(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, message) = match parse_message_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or message",
                )
            }
        };

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let thread_id = self
            .thread_ids
            .get(&chat_id)
            .cloned()
            .unwrap_or_else(|| chat_id.clone());

        let codex_params = json!({
            "threadId": thread_id,
            "input": [{"type": "text", "text": message}],
        });

        let process = self.process.as_ref().unwrap();
        match process.send_request("turn/start", Some(codex_params)).await {
            Ok(result) => {
                let turn_id = result
                    .get("turnId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Response::success(req_id, json!({ "chat_id": chat_id, "turn_id": turn_id }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("turn/start failed: {e}"),
            ),
        }
    }

    async fn chat_cancel(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, turn_id) = match parse_cancel_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or turn_id",
                )
            }
        };

        let process = match &self.process {
            Some(p) => p,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    "no codex process running",
                )
            }
        };

        let thread_id = self
            .thread_ids
            .get(&chat_id)
            .cloned()
            .unwrap_or_else(|| chat_id.clone());

        let codex_params = json!({
            "threadId": thread_id,
            "turnId": turn_id,
        });

        match process
            .send_request("turn/interrupt", Some(codex_params))
            .await
        {
            Ok(_) => Response::success(req_id, json!({ "ok": true })),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("turn/interrupt failed: {e}"),
            ),
        }
    }

    async fn approval_respond(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (codex_request_id, decision) = match parse_approval_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing codex_request_id or decision",
                )
            }
        };

        let process = match &self.process {
            Some(p) => p,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    "no codex process running",
                )
            }
        };

        let result = json!({ "decision": decision });
        match process.send_response(codex_request_id, result).await {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("approval response failed: {e}"),
            ),
        }
    }

    /// List all persisted chat sessions from the store.
    fn chat_list(&self, req_id: Uuid) -> Response {
        match self.store.list_chats() {
            Ok(records) => {
                let chats: Vec<Value> = records
                    .into_iter()
                    .map(|r| {
                        json!({
                            "chat_id": r.chat_id,
                            "thread_id": r.thread_id,
                            "created_at": r.created_at,
                            "status": r.status,
                            "event_pointer": r.event_pointer,
                        })
                    })
                    .collect();
                Response::success(req_id, json!({ "chats": chats }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("list failed: {e}"),
            ),
        }
    }
}

impl ServiceHandler for AgentService {
    fn namespace(&self) -> &str {
        "agent"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let method = method.to_string();
        let canonical = match method.as_str() {
            "agent.codex.create" => "agent.chat.create",
            "agent.codex.message.send" => "agent.chat.message.send",
            "agent.codex.cancel" => "agent.chat.cancel",
            "agent.codex.approval.respond" => "agent.chat.approval.respond",
            "agent.codex.list" => "agent.chat.list",
            other => other,
        }
        .to_string();
        Box::pin(async move {
            match canonical.as_str() {
                "agent.chat.create" => self.chat_create(id).await,
                "agent.chat.message.send" => self.chat_message_send(id, params).await,
                "agent.chat.cancel" => self.chat_cancel(id, params).await,
                "agent.chat.approval.respond" => self.approval_respond(id, params).await,
                "agent.chat.list" => self.chat_list(id),
                _ => Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {method}"),
                ),
            }
        })
    }

    fn handle_binary(&mut self, _frame: &BinaryFrame) {
        tracing::debug!("agent service does not handle binary frames");
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        std::mem::take(&mut self.reap_events)
    }

    fn shutdown(&mut self) {
        // Mark all active chats as inactive in storage on disconnect.
        for chat_id in self.thread_ids.keys() {
            if let Ok(Some(mut rec)) = self.store.get_chat(chat_id) {
                if rec.status == SessionStatus::Active {
                    rec.status = SessionStatus::Inactive;
                    if let Err(e) = self.store.upsert_chat(&rec) {
                        tracing::warn!(%chat_id, "failed to persist chat disconnect: {e}");
                    }
                }
            }
        }

        if let Some(h) = self.event_forwarder.take() {
            h.abort();
        }
        if let Some(mut p) = self.process.take() {
            p.shutdown();
        }
    }
}

impl Drop for AgentService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Background task: reads Codex events and forwards them as Homie Event
/// messages via the outbound WS channel.
async fn event_forwarder_loop(
    mut event_rx: mpsc::Receiver<CodexEvent>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: Arc<dyn Store>,
) {
    while let Some(event) = event_rx.recv().await {
        let topic = match codex_method_to_topic(&event.method) {
            Some(t) => t,
            None => {
                tracing::debug!(method = %event.method, "unmapped codex event, skipping");
                continue;
            }
        };

        let mut event_params = event.params.unwrap_or(json!({}));

        if let Some(codex_id) = event.id {
            if let Some(obj) = event_params.as_object_mut() {
                obj.insert("codex_request_id".into(), json!(codex_id));
            }
        }

        if let Some(thread_id) = extract_thread_id(&event_params) {
            if let Ok(Some(chat)) = store.get_chat(&thread_id) {
                let next = chat.event_pointer.saturating_add(1);
                let _ = store.update_event_pointer(&chat.chat_id, next);
            }
        }

        match outbound_tx.try_send(OutboundMessage::event(topic, Some(event_params))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic, "backpressure: dropping agent event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }
    }

    tracing::debug!("agent event forwarder exited");
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

// -- Param parsing helpers ------------------------------------------------

fn parse_message_params(params: &Option<Value>) -> Option<(String, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let message = p.get("message")?.as_str()?.to_string();
    Some((chat_id, message))
}

fn parse_cancel_params(params: &Option<Value>) -> Option<(String, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let turn_id = p.get("turn_id")?.as_str()?.to_string();
    Some((chat_id, turn_id))
}

fn parse_approval_params(params: &Option<Value>) -> Option<(u64, String)> {
    let p = params.as_ref()?;
    let codex_request_id = p.get("codex_request_id")?.as_u64()?;
    let decision = p.get("decision")?.as_str()?.to_string();
    Some((codex_request_id, decision))
}

fn extract_thread_id(params: &Value) -> Option<String> {
    params
        .get("threadId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("thread_id").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbound::OutboundMessage;
    use crate::storage::SqliteStore;

    fn make_store() -> Arc<dyn Store> {
        Arc::new(SqliteStore::open_memory().unwrap())
    }

    #[test]
    fn codex_method_maps_agent_message_delta_to_chat_delta() {
        assert_eq!(
            codex_method_to_topic("item/agentMessage/delta"),
            Some("agent.chat.delta")
        );
    }

    #[test]
    fn codex_method_maps_turn_events() {
        assert_eq!(
            codex_method_to_topic("turn/started"),
            Some("agent.chat.turn.started")
        );
        assert_eq!(
            codex_method_to_topic("turn/completed"),
            Some("agent.chat.turn.completed")
        );
    }

    #[test]
    fn codex_method_maps_item_events() {
        assert_eq!(
            codex_method_to_topic("item/started"),
            Some("agent.chat.item.started")
        );
        assert_eq!(
            codex_method_to_topic("item/completed"),
            Some("agent.chat.item.completed")
        );
    }

    #[test]
    fn codex_method_maps_approval_requests() {
        assert_eq!(
            codex_method_to_topic("item/commandExecution/requestApproval"),
            Some("agent.chat.approval.required")
        );
        assert_eq!(
            codex_method_to_topic("item/fileChange/requestApproval"),
            Some("agent.chat.approval.required")
        );
    }

    #[test]
    fn codex_method_maps_output_deltas() {
        assert_eq!(
            codex_method_to_topic("item/commandExecution/outputDelta"),
            Some("agent.chat.command.output")
        );
        assert_eq!(
            codex_method_to_topic("item/fileChange/outputDelta"),
            Some("agent.chat.file.output")
        );
    }

    #[test]
    fn codex_method_maps_reasoning_and_plan() {
        assert_eq!(
            codex_method_to_topic("item/reasoning/summaryTextDelta"),
            Some("agent.chat.reasoning.delta")
        );
        assert_eq!(
            codex_method_to_topic("turn/diff/updated"),
            Some("agent.chat.diff.updated")
        );
        assert_eq!(
            codex_method_to_topic("turn/plan/updated"),
            Some("agent.chat.plan.updated")
        );
    }

    #[test]
    fn unknown_codex_method_returns_none() {
        assert_eq!(codex_method_to_topic("unknown/method"), None);
    }

    #[test]
    fn parse_message_params_extracts_chat_id_and_message() {
        let params = Some(json!({
            "chat_id": "abc-123",
            "message": "hello world"
        }));
        let (chat_id, message) = parse_message_params(&params).unwrap();
        assert_eq!(chat_id, "abc-123");
        assert_eq!(message, "hello world");
    }

    #[test]
    fn parse_message_params_returns_none_when_missing_fields() {
        assert!(parse_message_params(&None).is_none());
        assert!(parse_message_params(&Some(json!({"chat_id": "x"}))).is_none());
        assert!(parse_message_params(&Some(json!({"message": "x"}))).is_none());
    }

    #[test]
    fn parse_cancel_params_extracts_ids() {
        let params = Some(json!({
            "chat_id": "c1",
            "turn_id": "t1"
        }));
        let (chat_id, turn_id) = parse_cancel_params(&params).unwrap();
        assert_eq!(chat_id, "c1");
        assert_eq!(turn_id, "t1");
    }

    #[test]
    fn parse_approval_params_extracts_id_and_decision() {
        let params = Some(json!({
            "codex_request_id": 42,
            "decision": "accept"
        }));
        let (id, decision) = parse_approval_params(&params).unwrap();
        assert_eq!(id, 42);
        assert_eq!(decision, "accept");
    }

    #[test]
    fn parse_approval_params_returns_none_for_invalid_input() {
        assert!(parse_approval_params(&None).is_none());
        assert!(
            parse_approval_params(&Some(json!({"codex_request_id": "not_a_number"}))).is_none()
        );
    }

    #[tokio::test]
    async fn agent_service_returns_error_for_unknown_method() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(tx, make_store());
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "agent.unknown.method", None).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn agent_service_namespace_is_agent() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let svc = AgentService::new(tx, make_store());
        assert_eq!(svc.namespace(), "agent");
    }

    #[test]
    fn agent_service_reap_returns_empty_initially() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(tx, make_store());
        assert!(svc.reap().is_empty());
    }

    #[tokio::test]
    async fn chat_list_returns_empty_initially() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(tx, make_store());
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "agent.chat.list", None).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let chats = result["chats"].as_array().unwrap();
        assert!(chats.is_empty());
    }
}

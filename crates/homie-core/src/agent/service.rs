use std::collections::HashMap;
use std::pin::Pin;

use axum::extract::ws::Message as WsMessage;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{
    encode_message, error_codes, BinaryFrame, Event, Message as ProtoMessage, Response,
};

use super::process::{CodexEvent, CodexProcess};
use crate::router::{ReapEvent, ServiceHandler};

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
    outbound_tx: mpsc::Sender<WsMessage>,
    process: Option<CodexProcess>,
    event_forwarder: Option<tokio::task::JoinHandle<()>>,
    reap_events: Vec<ReapEvent>,
    thread_ids: HashMap<String, String>,
}

impl AgentService {
    pub fn new(outbound_tx: mpsc::Sender<WsMessage>) -> Self {
        Self {
            outbound_tx,
            process: None,
            event_forwarder: None,
            reap_events: Vec::new(),
            thread_ids: HashMap::new(),
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
        let forwarder = tokio::spawn(event_forwarder_loop(event_rx, outbound));
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
                self.thread_ids.insert(chat_id.clone(), thread_id);
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
        Box::pin(async move {
            match method.as_str() {
                "agent.chat.create" => self.chat_create(id).await,
                "agent.chat.message.send" => self.chat_message_send(id, params).await,
                "agent.chat.cancel" => self.chat_cancel(id, params).await,
                "agent.chat.approval.respond" => self.approval_respond(id, params).await,
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
    outbound_tx: mpsc::Sender<WsMessage>,
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

        let proto_event = ProtoMessage::Event(Event {
            topic: topic.to_string(),
            params: Some(event_params),
        });

        let json = match encode_message(&proto_event) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("failed to encode agent event: {e}");
                continue;
            }
        };

        match outbound_tx.try_send(WsMessage::Text(json.into())) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic, "backpressure: dropping agent event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }
    }

    tracing::debug!("agent event forwarder exited");
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let (tx, _rx) = mpsc::channel(16);
        let mut svc = AgentService::new(tx);
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "agent.unknown.method", None).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn agent_service_namespace_is_agent() {
        let (tx, _rx) = mpsc::channel(16);
        let svc = AgentService::new(tx);
        assert_eq!(svc.namespace(), "agent");
    }

    #[test]
    fn agent_service_reap_returns_empty_initially() {
        let (tx, _rx) = mpsc::channel(16);
        let mut svc = AgentService::new(tx);
        assert!(svc.reap().is_empty());
    }
}

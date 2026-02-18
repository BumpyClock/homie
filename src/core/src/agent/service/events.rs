use crate::outbound::OutboundMessage;
use crate::storage::Store;
use crate::ExecPolicy;
use crate::HomieConfig;
use serde_json::json;
use tokio::sync::mpsc;

use super::params::{extract_thread_id, extract_turn_id};
use crate::agent::process::{CodexEvent, CodexResponseSender};

pub(super) fn codex_method_to_topics(method: &str) -> Option<(&'static str, &'static str)> {
    match method {
        "item/agentMessage/delta" => Some(("chat.message.delta", "agent.chat.delta")),
        "item/started" => Some(("chat.item.started", "agent.chat.item.started")),
        "item/completed" => Some(("chat.item.completed", "agent.chat.item.completed")),
        "turn/started" => Some(("chat.turn.started", "agent.chat.turn.started")),
        "turn/completed" => Some(("chat.turn.completed", "agent.chat.turn.completed")),
        "item/commandExecution/outputDelta" => {
            Some(("chat.command.output", "agent.chat.command.output"))
        }
        "item/fileChange/outputDelta" => Some(("chat.file.output", "agent.chat.file.output")),
        "item/reasoning/summaryTextDelta" => {
            Some(("chat.reasoning.delta", "agent.chat.reasoning.delta"))
        }
        "turn/diff/updated" => Some(("chat.diff.updated", "agent.chat.diff.updated")),
        "turn/plan/updated" => Some(("chat.plan.updated", "agent.chat.plan.updated")),
        "thread/tokenUsage/updated" => {
            Some(("chat.token.usage.updated", "agent.chat.token.usage.updated"))
        }
        "item/commandExecution/requestApproval" => {
            Some(("chat.approval.required", "agent.chat.approval.required"))
        }
        "item/fileChange/requestApproval" => {
            Some(("chat.approval.required", "agent.chat.approval.required"))
        }
        _ => None,
    }
}

/// Background task: reads Codex events and forwards them as Homie Event
/// messages via the outbound WS channel.
pub(super) async fn event_forwarder_loop(
    mut event_rx: mpsc::Receiver<CodexEvent>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: std::sync::Arc<dyn Store>,
    response_sender: CodexResponseSender,
    exec_policy: std::sync::Arc<ExecPolicy>,
    homie_config: std::sync::Arc<HomieConfig>,
) {
    while let Some(event) = event_rx.recv().await {
        let raw_params = event.params.unwrap_or(json!({}));
        if homie_config.raw_events_enabled() {
            if let (Some(thread_id), Some(run_id)) =
                (extract_thread_id(&raw_params), extract_turn_id(&raw_params))
            {
                if store
                    .insert_chat_raw_event(&run_id, &thread_id, &event.method, &raw_params)
                    .is_ok()
                {
                    let _ = store.prune_chat_raw_events(10);
                }
            }
        }

        if event.method == "item/commandExecution/requestApproval" {
            if let Some(id) = event.id.clone() {
                if let Some(argv) = super::approvals::approval_command_argv(&raw_params) {
                    if exec_policy.is_allowed(&argv) {
                        if let Some(thread_id) = extract_thread_id(&raw_params) {
                            if let Ok(Some(chat)) = store.get_chat(&thread_id) {
                                let next = chat.event_pointer.saturating_add(1);
                                let _ = store.update_event_pointer(&chat.chat_id, next);
                            }
                        }
                        let result = json!({ "decision": "accept" });
                        if response_sender.send_response(id, result).await.is_ok() {
                            tracing::info!("execpolicy auto-approved command");
                            continue;
                        }
                    }
                }
            }
        }

        let (chat_topic, agent_topic) = match codex_method_to_topics(&event.method) {
            Some(t) => t,
            None => {
                tracing::debug!(method = %event.method, "unmapped codex event, skipping");
                continue;
            }
        };

        let mut event_params = raw_params;

        if let Some(codex_id) = event.id {
            if let Some(obj) = event_params.as_object_mut() {
                obj.insert("codex_request_id".into(), codex_id.to_json());
            }
        } else if let Some(obj) = event_params.as_object_mut() {
            if !obj.contains_key("codex_request_id") {
                let fallback = obj
                    .get("requestId")
                    .or_else(|| obj.get("request_id"))
                    .or_else(|| obj.get("id"))
                    .cloned();
                if let Some(value) = fallback {
                    obj.insert("codex_request_id".into(), value);
                }
            }
        }

        if matches!(
            event.method.as_str(),
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
        ) {
            tracing::info!(
                method = %event.method,
                params = ?event_params,
                "codex approval requested"
            );
        }

        if let Some(thread_id) = extract_thread_id(&event_params) {
            if let Ok(Some(chat)) = store.get_chat(&thread_id) {
                let next = chat.event_pointer.saturating_add(1);
                let _ = store.update_event_pointer(&chat.chat_id, next);
            }
        }

        let chat_params = event_params.clone();
        match outbound_tx.try_send(OutboundMessage::event(chat_topic, Some(chat_params))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic = chat_topic, "backpressure: dropping chat event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }

        match outbound_tx.try_send(OutboundMessage::event(agent_topic, Some(event_params))) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(topic = agent_topic, "backpressure: dropping agent event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => break,
        }
    }

    tracing::debug!("agent event forwarder exited");
}

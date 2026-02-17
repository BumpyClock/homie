use homie_protocol::{error_codes, Response};
use roci::agent_loop::ApprovalDecision;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::process::CodexRequestId;
use crate::agent::service::core::CodexChatCore;

use super::params::parse_approval_params;

pub(super) fn approval_command_argv(params: &Value) -> Option<Vec<String>> {
    let command = params.get("command")?.as_str()?;
    if command.trim().is_empty() {
        return None;
    }
    shell_words::split(command).ok()
}

fn normalize_approval_decision(decision: &str) -> ApprovalDecision {
    match decision {
        "accept" => ApprovalDecision::Accept,
        "accept_for_session" => ApprovalDecision::AcceptForSession,
        "decline" => ApprovalDecision::Decline,
        "cancel" => ApprovalDecision::Cancel,
        _ => ApprovalDecision::Decline,
    }
}

impl CodexChatCore {
    pub(super) async fn approval_respond(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (codex_request_id, decision) = match parse_approval_params(&params) {
            Some(v) => v,
            None => {
                tracing::warn!(
                    ?params,
                    "approval respond missing codex_request_id or decision"
                );
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing codex_request_id or decision",
                );
            }
        };

        if self.use_roci() {
            let request_id = match &codex_request_id {
                CodexRequestId::Number(n) => n.to_string(),
                CodexRequestId::Text(text) => text.clone(),
            };
            let decision = normalize_approval_decision(&decision);
            let ok = self.roci.respond_approval(&request_id, decision).await;
            return if ok {
                Response::success(req_id, json!({ "ok": true }))
            } else {
                Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    "approval response failed",
                )
            };
        }

        let process = match &self.process {
            Some(p) => p,
            None => {
                tracing::warn!(
                    ?codex_request_id,
                    "approval respond failed: no codex process running"
                );
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    "no codex process running",
                );
            }
        };

        let result = json!({ "decision": decision });
        tracing::info!(
            ?codex_request_id,
            decision = %result["decision"],
            "approval respond"
        );
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

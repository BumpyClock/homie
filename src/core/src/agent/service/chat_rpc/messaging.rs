use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::roci_backend::{RociBackend, StartRunRequest};

use super::super::core::CodexChatCore;
use super::super::params::{
    build_chat_settings, merge_settings, normalize_model_selector, parse_cancel_params,
    parse_message_params, MessageParams,
};

impl CodexChatCore {
    pub(crate) async fn chat_message_send(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let MessageParams {
            chat_id,
            message,
            model,
            effort,
            approval_policy,
            collaboration_mode,
            inject,
        } = match parse_message_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or message",
                )
            }
        };
        let normalized_model = model
            .as_ref()
            .map(|m| normalize_model_selector(m, &self.homie_config.providers));

        if self.use_roci() {
            let thread_id = match self.resolve_thread_id(&chat_id, None) {
                Some(id) => id,
                None => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        "missing thread_id",
                    )
                }
            };

            let settings = build_chat_settings(
                normalized_model.as_ref(),
                effort.as_ref(),
                approval_policy.as_ref(),
                collaboration_mode.as_ref(),
            );
            let existing_settings = self
                .store
                .get_chat(&chat_id)
                .ok()
                .flatten()
                .and_then(|rec| rec.settings);
            if let Some(settings) = settings {
                let merged = merge_settings(existing_settings, settings);
                if let Err(e) = self.store.update_chat_settings(&chat_id, Some(&merged)) {
                    tracing::warn!(%chat_id, "failed to persist chat settings: {e}");
                }
            }

            if inject {
                if let Some(turn_id) = self
                    .roci
                    .queue_message(&chat_id, &thread_id, &message)
                    .await
                {
                    return Response::success(
                        req_id,
                        json!({ "chat_id": chat_id, "turn_id": turn_id, "queued": true }),
                    );
                }
            }

            let roci_model = match RociBackend::parse_model(normalized_model.as_ref()) {
                Ok(model) => model,
                Err(err) => return Response::error(req_id, error_codes::INVALID_PARAMS, err),
            };
            let roci_settings = RociBackend::parse_settings(
                effort.as_ref(),
                self.homie_config.chat.stream_idle_timeout_ms,
            );
            let roci_policy = RociBackend::parse_approval_policy(approval_policy.as_ref());
            let roci_collab_mode =
                RociBackend::parse_collaboration_mode(collaboration_mode.as_ref());
            let roci_config = match self.roci_config_for_model(&roci_model).await {
                Ok(config) => config,
                Err(err) => return Response::error(req_id, error_codes::INTERNAL_ERROR, err),
            };
            match self
                .roci
                .start_run(StartRunRequest {
                    chat_id: &chat_id,
                    thread_id: &thread_id,
                    message: &message,
                    model: roci_model,
                    settings: roci_settings,
                    approval_policy: roci_policy,
                    config: roci_config,
                    collaboration_mode: roci_collab_mode,
                    system_prompt: Some(self.homie_config.chat.system_prompt.clone()),
                    tool_channel: self.tool_channel.as_deref(),
                })
                .await
            {
                Ok(turn_id) => {
                    return Response::success(
                        req_id,
                        json!({ "chat_id": chat_id, "turn_id": turn_id }),
                    )
                }
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("roci run failed: {e}"),
                    )
                }
            }
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let thread_id = match self.resolve_thread_id(&chat_id, None) {
            Some(id) => id,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing thread_id")
            }
        };

        let mut codex_params = json!({
            "threadId": thread_id,
            "input": [{"type": "text", "text": message}],
        });
        if let Some(model) = normalized_model.as_ref() {
            codex_params["model"] = json!(model);
        }
        if let Some(effort) = effort.as_ref() {
            codex_params["effort"] = json!(effort);
        }
        if let Some(approval_policy) = approval_policy.as_ref() {
            codex_params["approvalPolicy"] = json!(approval_policy);
        }
        if let Some(collaboration_mode) = collaboration_mode.as_ref() {
            if collaboration_mode.is_object() {
                codex_params["collaborationMode"] = collaboration_mode.clone();
            }
        }
        let settings = build_chat_settings(
            normalized_model.as_ref(),
            effort.as_ref(),
            approval_policy.as_ref(),
            collaboration_mode.as_ref(),
        );
        let existing_settings = self
            .store
            .get_chat(&chat_id)
            .ok()
            .flatten()
            .and_then(|rec| rec.settings);

        let process = self.process.as_ref().unwrap();
        match process.send_request("turn/start", Some(codex_params)).await {
            Ok(result) => {
                let turn_id = super::super::models::extract_id_from_result(
                    &result,
                    &["turnId", "turn_id"],
                    &[("turn", "id")],
                )
                .unwrap_or_default();
                if let Some(settings) = settings {
                    let merged = merge_settings(existing_settings, settings);
                    if let Err(e) = self.store.update_chat_settings(&chat_id, Some(&merged)) {
                        tracing::warn!(%chat_id, "failed to persist chat settings: {e}");
                    }
                }
                Response::success(req_id, json!({ "chat_id": chat_id, "turn_id": turn_id }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("turn/start failed: {e}"),
            ),
        }
    }

    pub(crate) async fn chat_cancel(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

        if self.use_roci() {
            let canceled = self.roci.cancel_run(&turn_id).await;
            if canceled {
                return Response::success(req_id, json!({ "ok": true }));
            }
            return Response::error(req_id, error_codes::SESSION_NOT_FOUND, "run not found");
        }

        let thread_id = match self.resolve_thread_id(&chat_id, None) {
            Some(id) => id,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing thread_id")
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
}

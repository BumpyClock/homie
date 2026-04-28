use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::roci_backend::RociBackend;
use crate::storage::{ChatRecord, SessionStatus};

use super::super::core::CodexChatCore;
use super::super::models::{chrono_now, extract_id_from_result};
use super::super::params::parse_resume_params;

impl CodexChatCore {
    pub(crate) async fn chat_create(&mut self, req_id: Uuid) -> Response {
        if self.use_roci() {
            let chat_id = Uuid::new_v4().to_string();
            let thread_id = chat_id.clone();
            self.thread_ids.insert(chat_id.clone(), thread_id.clone());
            if let Ok(model) = RociBackend::parse_model(None) {
                let _ = self
                    .roci
                    .ensure_thread(
                        &chat_id,
                        &thread_id,
                        model,
                        roci::config::RociConfig::from_env(),
                        Some(self.homie_config.chat.system_prompt.clone()),
                        self.tool_channel.as_deref(),
                    )
                    .await;
            }
            let rec = ChatRecord {
                chat_id: chat_id.clone(),
                thread_id: thread_id.clone(),
                created_at: chrono_now(),
                status: SessionStatus::Active,
                event_pointer: 0,
                settings: None,
            };
            if let Err(e) = self.store.upsert_chat(&rec) {
                tracing::warn!(%chat_id, "failed to persist chat create: {e}");
            }
            return Response::success(
                req_id,
                json!({ "chat_id": chat_id, "thread_id": thread_id }),
            );
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = json!({ "model": super::super::models::codex_model() });
        match process.send_request("thread/start", Some(params)).await {
            Ok(result) => {
                let thread_id = extract_id_from_result(
                    &result,
                    &["threadId", "thread_id"],
                    &[("thread", "id")],
                )
                .unwrap_or_default();
                let thread_id_value = thread_id.clone();
                let chat_id = if thread_id.is_empty() {
                    Uuid::new_v4().to_string()
                } else {
                    thread_id.clone()
                };
                self.thread_ids.insert(chat_id.clone(), thread_id.clone());

                let rec = ChatRecord {
                    chat_id: chat_id.clone(),
                    thread_id,
                    created_at: chrono_now(),
                    status: SessionStatus::Active,
                    event_pointer: 0,
                    settings: None,
                };
                if let Err(e) = self.store.upsert_chat(&rec) {
                    tracing::warn!(%chat_id, "failed to persist chat create: {e}");
                }

                Response::success(
                    req_id,
                    json!({ "chat_id": chat_id, "thread_id": thread_id_value }),
                )
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/start failed: {e}"),
            ),
        }
    }

    pub(crate) async fn chat_resume(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, thread_id_param) = match parse_resume_params(&params) {
            Some(v) => v,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing chat_id"),
        };

        let thread_id = match self.resolve_thread_id(&chat_id, thread_id_param.as_deref()) {
            Some(id) => id,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing thread_id")
            }
        };

        if self.use_roci() {
            if let Ok(model) = RociBackend::parse_model(None) {
                let _ = self
                    .roci
                    .ensure_thread(
                        &chat_id,
                        &thread_id,
                        model,
                        roci::config::RociConfig::from_env(),
                        Some(self.homie_config.chat.system_prompt.clone()),
                        self.tool_channel.as_deref(),
                    )
                    .await;
            }
            let rec = match self.store.get_chat(&chat_id).ok().flatten() {
                Some(mut rec) => {
                    rec.thread_id = thread_id.clone();
                    rec.status = SessionStatus::Active;
                    rec
                }
                None => ChatRecord {
                    chat_id: chat_id.clone(),
                    thread_id: thread_id.clone(),
                    created_at: chrono_now(),
                    status: SessionStatus::Active,
                    event_pointer: 0,
                    settings: None,
                },
            };
            if let Err(e) = self.store.upsert_chat(&rec) {
                tracing::warn!(%chat_id, "failed to persist chat resume: {e}");
            }
            return Response::success(
                req_id,
                json!({ "chat_id": chat_id, "thread_id": thread_id }),
            );
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = json!({ "threadId": thread_id });
        match process.send_request("thread/resume", Some(params)).await {
            Ok(result) => {
                let resolved = extract_id_from_result(
                    &result,
                    &["threadId", "thread_id"],
                    &[("thread", "id")],
                )
                .unwrap_or_else(|| thread_id.clone());
                self.thread_ids.insert(chat_id.clone(), resolved.clone());

                let rec = match self.store.get_chat(&chat_id).ok().flatten() {
                    Some(mut rec) => {
                        rec.thread_id = resolved.clone();
                        rec.status = SessionStatus::Active;
                        rec
                    }
                    None => ChatRecord {
                        chat_id: chat_id.clone(),
                        thread_id: resolved.clone(),
                        created_at: chrono_now(),
                        status: SessionStatus::Active,
                        event_pointer: 0,
                        settings: None,
                    },
                };
                if let Err(e) = self.store.upsert_chat(&rec) {
                    tracing::warn!(%chat_id, "failed to persist chat resume: {e}");
                }

                Response::success(req_id, json!({ "chat_id": chat_id, "thread_id": resolved }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/resume failed: {e}"),
            ),
        }
    }
}

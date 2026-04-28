use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use super::super::core::CodexChatCore;
use super::super::params::{
    parse_thread_archive_params, parse_thread_read_params, parse_thread_rename_params,
};
use crate::agent::roci_backend::RociBackend;

impl CodexChatCore {
    pub(crate) async fn chat_thread_read(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (chat_id, thread_id, include_turns) = match parse_thread_read_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or thread_id",
                )
            }
        };

        let thread_id = match thread_id {
            Some(id) => id,
            None => match chat_id
                .as_deref()
                .and_then(|id| self.resolve_thread_id(id, None))
            {
                Some(id) => id,
                None => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        "missing thread_id",
                    )
                }
            },
        };

        let settings = chat_id
            .as_deref()
            .or(Some(thread_id.as_str()))
            .and_then(|id| {
                self.store
                    .get_chat(id)
                    .ok()
                    .flatten()
                    .and_then(|rec| rec.settings)
            });

        if self.use_roci() {
            let with_settings = |thread: Value| {
                let mut result = json!({ "thread": thread });
                if let Some(settings) = settings.clone() {
                    if let Some(obj) = result.as_object_mut() {
                        obj.insert("settings".into(), settings);
                    }
                }
                result
            };

            if !include_turns {
                let thread = json!({ "id": thread_id });
                return Response::success(req_id, with_settings(thread));
            }

            if let Ok(model) = RociBackend::parse_model(None) {
                let _ = self
                    .roci
                    .ensure_thread(
                        chat_id.as_deref().unwrap_or(&thread_id),
                        &thread_id,
                        model,
                        roci::config::RociConfig::from_env(),
                        Some(self.homie_config.chat.system_prompt.clone()),
                        self.tool_channel.as_deref(),
                    )
                    .await;
            }

            if let Some(thread) = self.roci.thread_read(&thread_id).await {
                return Response::success(req_id, with_settings(thread));
            }

            let thread = json!({ "id": thread_id });
            return Response::success(req_id, with_settings(thread));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = json!({ "threadId": thread_id, "includeTurns": include_turns });
        match process.send_request("thread/read", Some(params)).await {
            Ok(mut result) => {
                if let Some(settings) = settings {
                    if let Some(obj) = result.as_object_mut() {
                        obj.insert("settings".into(), settings);
                    } else {
                        result = json!({ "thread": result, "settings": settings });
                    }
                }
                Response::success(req_id, result)
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/read failed: {e}"),
            ),
        }
    }

    pub(crate) async fn chat_thread_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let threads = self.roci.thread_list().await;
            return Response::success(req_id, json!({ "threads": threads }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("thread/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/list failed: {e}"),
            ),
        }
    }

    pub(crate) async fn chat_thread_archive(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (chat_id, thread_id_param) = match parse_thread_archive_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or thread_id",
                )
            }
        };

        let thread_id = match self.resolve_thread_id(&chat_id, thread_id_param.as_deref()) {
            Some(id) => id,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing thread_id")
            }
        };

        if self.use_roci() {
            self.roci.thread_archive(&thread_id).await;
            if let Err(e) = self.store.delete_chat(&chat_id) {
                tracing::warn!(%chat_id, "failed to delete archived chat: {e}");
            }
            self.thread_ids.remove(&chat_id);
            return Response::success(req_id, json!({ "ok": true }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = json!({ "threadId": thread_id });
        match process.send_request("thread/archive", Some(params)).await {
            Ok(_) => {
                if let Err(e) = self.store.delete_chat(&chat_id) {
                    tracing::warn!(%chat_id, "failed to delete archived chat: {e}");
                }
                self.thread_ids.remove(&chat_id);
                Response::success(req_id, json!({ "ok": true }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/archive failed: {e}"),
            ),
        }
    }

    pub(crate) async fn chat_thread_rename(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (chat_id, thread_id_param, title) = match parse_thread_rename_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or title",
                )
            }
        };

        let thread_id = match self.resolve_thread_id(&chat_id, thread_id_param.as_deref()) {
            Some(id) => id,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing thread_id")
            }
        };

        if self.use_roci() {
            tracing::debug!(%chat_id, %thread_id, "roci rename ignored");
            return Response::success(req_id, json!({ "ok": true }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = json!({ "threadId": thread_id, "name": title });
        match process.send_request("thread/name/set", Some(params)).await {
            Ok(_) => Response::success(req_id, json!({ "ok": true })),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("thread/name/set failed: {e}"),
            ),
        }
    }
}

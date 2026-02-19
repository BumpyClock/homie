use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::roci_backend::{RociBackend, StartRunRequest};
use crate::storage::SessionStatus;

use super::files::{extract_attached_folder, search_files_in_folder};
use super::models::{chrono_now, extract_id_from_result};
use super::params::{
    build_chat_settings, merge_settings, normalize_model_selector, normalize_settings_models,
    parse_cancel_params, parse_files_search_params, parse_message_params, parse_resume_params,
    parse_settings_update_params, parse_thread_archive_params, parse_thread_read_params,
    parse_thread_rename_params, MessageParams,
};
use crate::agent::service::core::CodexChatCore;
use crate::storage::ChatRecord;

impl CodexChatCore {
    pub(super) async fn chat_create(&mut self, req_id: Uuid) -> Response {
        if self.use_roci() {
            let chat_id = Uuid::new_v4().to_string();
            let thread_id = chat_id.clone();
            self.thread_ids.insert(chat_id.clone(), thread_id.clone());
            self.roci.ensure_thread(&thread_id).await;
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
        let params = json!({ "model": crate::agent::service::models::codex_model() });
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

    pub(super) async fn chat_resume(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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
            self.roci.ensure_thread(&thread_id).await;
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

    pub(super) async fn chat_message_send(
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
                let turn_id =
                    extract_id_from_result(&result, &["turnId", "turn_id"], &[("turn", "id")])
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

    pub(super) async fn chat_cancel(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    pub(super) async fn chat_thread_read(
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

            self.roci.ensure_thread(&thread_id).await;

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

    pub(super) async fn chat_thread_list(
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

    pub(super) fn chat_settings_update(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, updates) = match parse_settings_update_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or settings",
                )
            }
        };
        let updates = normalize_settings_models(updates, &self.homie_config.providers);

        let existing = self
            .store
            .get_chat(&chat_id)
            .ok()
            .flatten()
            .and_then(|rec| rec.settings);
        let merged = merge_settings(existing, updates);
        if let Err(e) = self.store.update_chat_settings(&chat_id, Some(&merged)) {
            return Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("settings update failed: {e}"),
            );
        }
        Response::success(req_id, json!({ "ok": true, "settings": merged }))
    }

    pub(super) fn chat_files_search(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, query, limit, base_override) = match parse_files_search_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or query",
                )
            }
        };

        let settings = match self.store.get_chat(&chat_id) {
            Ok(Some(rec)) => rec.settings,
            _ => None,
        };
        let base = extract_attached_folder(settings.as_ref()).or_else(|| base_override.clone());
        let base = match base {
            Some(path) => path,
            None => {
                tracing::debug!(%chat_id, "chat files search skipped: no attached folder");
                return Response::success(req_id, json!({ "files": [] }));
            }
        };

        tracing::debug!(%chat_id, %base, %query, %limit, "chat files search");
        match search_files_in_folder(&base, &query, limit) {
            Ok(files) => {
                tracing::debug!(%chat_id, count = files.len(), "chat files search complete");
                Response::success(req_id, json!({ "files": files, "base_path": base }))
            }
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }

    pub(super) async fn chat_thread_archive(
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

    pub(super) async fn chat_thread_rename(
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

    pub(super) fn chat_list(&self, req_id: Uuid) -> Response {
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
                            "settings": r.settings,
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

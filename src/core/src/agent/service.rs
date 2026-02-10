use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};
use roci::auth::providers::claude_code::ClaudeCodeAuth;
use roci::auth::providers::github_copilot::GitHubCopilotAuth;
use roci::auth::providers::openai_codex::OpenAiCodexAuth;
use roci::auth::{DeviceCodePoll, DeviceCodeSession, FileTokenStore, TokenStore, TokenStoreConfig};
use roci::config::RociConfig;
use roci::models::LanguageModel;

use super::process::{CodexEvent, CodexProcess, CodexRequestId, CodexResponseSender};
use super::roci_backend::{ChatBackend, RociBackend};
use super::tools::{list_tools, ToolContext, DEFAULT_TOOL_CHANNEL};
use crate::homie_config::ProvidersConfig;
use crate::outbound::OutboundMessage;
use crate::paths::homie_skills_dir;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{ChatRecord, SessionStatus, Store};
use crate::{ExecPolicy, HomieConfig};
use roci::agent_loop::ApprovalDecision;

/// Maps Codex notification methods to Homie event topics.
fn codex_method_to_topics(method: &str) -> Option<(&'static str, &'static str)> {
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
struct CodexChatCore {
    backend: ChatBackend,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    process: Option<CodexProcess>,
    event_forwarder: Option<tokio::task::JoinHandle<()>>,
    reap_events: Vec<ReapEvent>,
    thread_ids: HashMap<String, String>,
    store: Arc<dyn Store>,
    homie_config: Arc<HomieConfig>,
    exec_policy: Arc<ExecPolicy>,
    roci: RociBackend,
}

impl CodexChatCore {
    fn use_roci(&self) -> bool {
        matches!(self.backend, ChatBackend::Roci)
    }

    fn new(
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
    async fn ensure_process(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Ok(());
        }

        let (process, event_rx) = CodexProcess::spawn().await?;
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

    async fn chat_create(&mut self, req_id: Uuid) -> Response {
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
        let params = json!({ "model": codex_model() });
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

                // Persist chat metadata.
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

    async fn chat_resume(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    async fn chat_message_send(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, message, model, effort, approval_policy, collaboration_mode, inject) =
            match parse_message_params(&params) {
                Some(v) => v,
                None => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        "missing chat_id or message",
                    )
                }
            };

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
                model.as_ref(),
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

            let roci_model = match RociBackend::parse_model(model.as_ref()) {
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
                .start_run(
                    &chat_id,
                    &thread_id,
                    &message,
                    roci_model,
                    roci_settings,
                    roci_policy,
                    roci_config,
                    roci_collab_mode,
                    Some(self.homie_config.chat.system_prompt.clone()),
                )
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
        if let Some(model) = model.as_ref() {
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
            model.as_ref(),
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

    async fn chat_cancel(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    async fn chat_thread_read(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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
            .or_else(|| Some(thread_id.as_str()))
            .and_then(|id| {
                self.store
                    .get_chat(id)
                    .ok()
                    .flatten()
                    .and_then(|rec| rec.settings)
            });

        if self.use_roci() {
            if !include_turns {
                let thread = json!({ "id": thread_id });
                let mut result = json!({ "thread": thread });
                if let Some(settings) = settings {
                    if let Some(obj) = result.as_object_mut() {
                        obj.insert("settings".into(), settings);
                    }
                }
                return Response::success(req_id, result);
            }
            match self.roci.thread_read(&thread_id).await {
                Some(thread) => {
                    let mut result = json!({ "thread": thread });
                    if let Some(settings) = settings {
                        if let Some(obj) = result.as_object_mut() {
                            obj.insert("settings".into(), settings);
                        }
                    }
                    return Response::success(req_id, result);
                }
                None => {
                    return Response::error(
                        req_id,
                        error_codes::SESSION_NOT_FOUND,
                        "thread not found",
                    )
                }
            }
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

    async fn chat_thread_list(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    fn chat_settings_update(&self, req_id: Uuid, params: Option<Value>) -> Response {
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

    fn chat_files_search(&self, req_id: Uuid, params: Option<Value>) -> Response {
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

    async fn chat_thread_archive(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    async fn chat_thread_rename(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
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

    async fn chat_account_list(&mut self, req_id: Uuid) -> Response {
        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list init failed: {e}"),
                )
            }
        };

        let cfg = &self.homie_config.providers;
        if cfg.openai_codex.enabled {
            self.import_codex_cli_credentials(&store);
        }
        if cfg.claude_code.enabled && cfg.claude_code.import_from_cli {
            self.import_claude_cli_credentials(&store);
        }

        let mut providers = Vec::new();
        match self.build_provider_status(
            &store,
            "openai-codex",
            "openai_codex",
            cfg.openai_codex.enabled,
        ) {
            Ok(status) => providers.push(status),
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list failed: {e}"),
                )
            }
        }
        match self.build_provider_status(
            &store,
            "github-copilot",
            "github_copilot",
            cfg.github_copilot.enabled,
        ) {
            Ok(status) => providers.push(status),
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list failed: {e}"),
                )
            }
        }
        match self.build_provider_status(
            &store,
            "claude-code",
            "claude_code",
            cfg.claude_code.enabled,
        ) {
            Ok(status) => providers.push(status),
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list failed: {e}"),
                )
            }
        }

        Response::success(req_id, json!({ "providers": providers }))
    }

    async fn chat_account_login_start(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (provider_id, profile, _param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login start failed: {e}"),
                )
            }
        };

        match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "claude-code" => Response::error(
                req_id,
                error_codes::INVALID_PARAMS,
                "claude-code does not support device-code login",
            ),
            _ => Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider"),
        }
    }

    async fn chat_account_login_poll(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let (provider_id, profile, param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }
        let session = match parse_device_code_session(&param_map, &provider_id) {
            Some(session) => session,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing session"),
        };

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login poll failed: {e}"),
                )
            }
        };

        let poll = match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "claude-code" => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "claude-code does not support device-code login",
                )
            }
            _ => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider")
            }
        };

        match poll {
            Ok(result) => Response::success(req_id, device_code_poll_json(result)),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("device code poll failed: {e}"),
            ),
        }
    }

    async fn chat_account_read(&mut self, req_id: Uuid) -> Response {
        if self.use_roci() {
            let store = match self.roci_token_store() {
                Ok(store) => store,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read init failed: {e}"),
                    )
                }
            };
            let cfg = &self.homie_config.providers;
            if cfg.openai_codex.enabled {
                self.import_codex_cli_credentials(&store);
            }
            if cfg.claude_code.enabled && cfg.claude_code.import_from_cli {
                self.import_claude_cli_credentials(&store);
            }
            let mut providers = Vec::new();
            let openai = match self.build_provider_status(
                &store,
                "openai-codex",
                "openai_codex",
                cfg.openai_codex.enabled,
            ) {
                Ok(status) => status,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read failed: {e}"),
                    )
                }
            };
            let github = match self.build_provider_status(
                &store,
                "github-copilot",
                "github_copilot",
                cfg.github_copilot.enabled,
            ) {
                Ok(status) => status,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read failed: {e}"),
                    )
                }
            };
            let claude = match self.build_provider_status(
                &store,
                "claude-code",
                "claude_code",
                cfg.claude_code.enabled,
            ) {
                Ok(status) => status,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read failed: {e}"),
                    )
                }
            };
            providers.push(openai);
            providers.push(github);
            providers.push(claude);
            let any_logged_in = providers.iter().any(|entry| {
                entry
                    .get("logged_in")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            });
            let account = if any_logged_in {
                json!({ "providers": providers })
            } else {
                Value::Null
            };
            return Response::success(
                req_id,
                json!({
                    "account": account,
                    "providers": providers,
                    "requires_openai_auth": !any_logged_in,
                }),
            );
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("account/read", Some(json!({}))).await {
            Ok(result) => {
                tracing::info!(result = %result, "codex account/read");
                Response::success(req_id, result)
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("account/read failed: {e}"),
            ),
        }
    }

    fn roci_token_store(&self) -> Result<FileTokenStore, String> {
        let base = self.homie_config.credentials_dir()?;
        Ok(FileTokenStore::new(TokenStoreConfig::new(base)))
    }

    fn provider_enabled(&self, provider_id: &str) -> bool {
        let cfg = &self.homie_config.providers;
        match provider_id {
            "openai-codex" => cfg.openai_codex.enabled,
            "github-copilot" => cfg.github_copilot.enabled,
            "claude-code" => cfg.claude_code.enabled,
            _ => false,
        }
    }

    fn openai_codex_auth(&self, store: FileTokenStore, profile: &str) -> OpenAiCodexAuth {
        let mut auth = OpenAiCodexAuth::new(Arc::new(store)).with_profile(profile);
        let cfg = &self.homie_config.providers.openai_codex;
        if !cfg.issuer.trim().is_empty() {
            auth = auth.with_issuer(cfg.issuer.clone());
        }
        if !cfg.refresh_token_url_override.trim().is_empty() {
            auth = auth.with_refresh_token_url_override(cfg.refresh_token_url_override.clone());
        }
        auth
    }

    fn github_copilot_auth(&self, store: FileTokenStore, profile: &str) -> GitHubCopilotAuth {
        let mut auth = GitHubCopilotAuth::new(Arc::new(store)).with_profile(profile);
        let cfg = &self.homie_config.providers.github_copilot;
        if !cfg.device_code_url.trim().is_empty() {
            auth = auth.with_device_code_url(cfg.device_code_url.clone());
        }
        if !cfg.token_url.trim().is_empty() {
            auth = auth.with_access_token_url(cfg.token_url.clone());
        }
        if !cfg.copilot_token_url.trim().is_empty() {
            auth = auth.with_copilot_token_url(cfg.copilot_token_url.clone());
        }
        auth
    }

    fn claude_code_auth(&self, store: FileTokenStore, profile: &str) -> ClaudeCodeAuth {
        ClaudeCodeAuth::new(Arc::new(store)).with_profile(profile)
    }

    async fn roci_config_for_model(&self, model: &LanguageModel) -> Result<RociConfig, String> {
        let config = RociConfig::from_env();
        let store = self.roci_token_store()?;
        let cfg = &self.homie_config.providers;
        if cfg.openai_codex.enabled {
            self.import_codex_cli_credentials(&store);
        }
        if cfg.claude_code.enabled && cfg.claude_code.import_from_cli {
            self.import_claude_cli_credentials(&store);
        }

        match model.provider_name() {
            "openai" => {
                if config.get_api_key("openai").is_none() {
                    return Err("Missing OPENAI_API_KEY. Codex OAuth is available; use openai-codex/* models or set OPENAI_API_KEY.".to_string());
                }
            }
            "openai-codex" => {
                if cfg.openai_codex.enabled {
                    let auth = self.openai_codex_auth(store.clone(), "default");
                    if let Ok(token) = auth.get_token().await {
                        if config.get_api_key("openai-codex").is_none() {
                            config.set_api_key("openai-codex", token.access_token);
                        }
                        if let Some(account_id) = token.account_id {
                            config.set_account_id("openai-codex", account_id);
                            if debug_enabled() {
                                tracing::debug!("openai-codex account_id set");
                            }
                        }
                        if config.get_base_url("openai-codex").is_none() {
                            if let Some(base) = config.get_base_url("openai") {
                                config.set_base_url("openai-codex", base);
                            }
                        }
                    }
                }
            }
            "openai-compatible" => {
                if cfg.github_copilot.enabled && config.get_api_key("openai-compatible").is_none() {
                    let auth = self.github_copilot_auth(store.clone(), "default");
                    if let Ok(token) = auth.exchange_copilot_token().await {
                        config.set_api_key("openai-compatible", token.token.clone());
                        if config.get_base_url("openai-compatible").is_none() {
                            config.set_base_url("openai-compatible", token.base_url);
                        }
                    }
                }
            }
            "anthropic" => {
                if cfg.claude_code.enabled && config.get_api_key("anthropic").is_none() {
                    let auth = self.claude_code_auth(store.clone(), "default");
                    if let Ok(token) = auth.get_token().await {
                        config.set_api_key("anthropic", token.access_token);
                    }
                }
            }
            _ => {}
        }

        Ok(config)
    }

    fn import_codex_cli_credentials(&self, store: &FileTokenStore) {
        let existing = match store.load("openai-codex", "default") {
            Ok(token) => token,
            Err(err) => {
                tracing::warn!(error = %err, "codex token load failed");
                return;
            }
        };
        if existing.is_some() {
            return;
        }
        let auth = OpenAiCodexAuth::new(Arc::new(store.clone()));
        match auth.import_codex_auth_json(None) {
            Ok(Some(_)) => {
                tracing::info!("imported codex cli credentials");
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, "codex cli credential import failed");
            }
        }
    }

    fn import_claude_cli_credentials(&self, store: &FileTokenStore) {
        let existing = match store.load("claude-code", "default") {
            Ok(token) => token,
            Err(err) => {
                tracing::warn!(error = %err, "claude token load failed");
                return;
            }
        };
        if existing.is_some() {
            return;
        }
        let auth = ClaudeCodeAuth::new(Arc::new(store.clone()));
        match auth.import_cli_credentials(None) {
            Ok(Some(_)) => {
                tracing::info!("imported claude cli credentials");
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, "claude cli credential import failed");
            }
        }
    }

    fn build_provider_status(
        &self,
        store: &FileTokenStore,
        provider_id: &str,
        provider_key: &str,
        enabled: bool,
    ) -> Result<Value, String> {
        let mut map = Map::new();
        map.insert("id".into(), json!(provider_id));
        map.insert("key".into(), json!(provider_key));
        map.insert("enabled".into(), json!(enabled));
        if !enabled {
            map.insert("logged_in".into(), json!(false));
            return Ok(Value::Object(map));
        }
        let token = store
            .load(provider_id, "default")
            .map_err(|e| format!("load {provider_id} token: {e}"))?;
        map.insert("logged_in".into(), json!(token.is_some()));
        if let Some(token) = token {
            if let Some(expires_at) = token.expires_at {
                map.insert("expires_at".into(), json!(expires_at.to_rfc3339()));
            }
            if let Some(scopes) = token.scopes {
                map.insert("scopes".into(), json!(scopes));
            }
            map.insert(
                "has_refresh_token".into(),
                json!(token.refresh_token.is_some()),
            );
        }
        Ok(Value::Object(map))
    }

    async fn chat_skills_list(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        if self.use_roci() {
            let skills = match list_homie_skills() {
                Ok(skills) => skills,
                Err(err) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("skills list failed: {err}"),
                    )
                }
            };
            return Response::success(req_id, json!({ "data": [{ "skills": skills }] }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("skills/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("skills/list failed: {e}"),
            ),
        }
    }

    async fn chat_model_list(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        if self.use_roci() {
            let models = roci_model_catalog(&self.homie_config.providers);
            return Response::success(req_id, json!({ "data": models }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("model/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("model/list failed: {e}"),
            ),
        }
    }

    async fn chat_tools_list(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        if self.use_roci() {
            let channel = parse_tool_channel(&params);
            let ctx = ToolContext::new_with_channel(self.homie_config.clone(), &channel);
            let tools = match list_tools(ctx, &self.homie_config) {
                Ok(tools) => tools,
                Err(err) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("tools list failed: {err}"),
                    )
                }
            };
            let data: Vec<Value> = tools
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "provider": tool.provider_id,
                        "provider_dynamic": tool.provider_dynamic,
                        "input_schema": tool.input_schema,
                    })
                })
                .collect();
            return Response::success(req_id, json!({ "data": data }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("tools/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("tools/list failed: {e}"),
            ),
        }
    }

    async fn chat_collaboration_mode_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let modes = vec![
                json!({
                    "name": "Plan",
                    "mode": "plan",
                    "description": "Plan steps before executing.",
                }),
                json!({
                    "name": "Code",
                    "mode": "code",
                    "description": "Act immediately on user requests.",
                }),
            ];
            return Response::success(req_id, json!({ "data": modes }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("collaborationMode/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("collaborationMode/list failed: {e}"),
            ),
        }
    }

    async fn chat_skills_config_write(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        if self.use_roci() {
            return Response::success(req_id, json!({ "ok": true }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("skills/config/write", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("skills/config/write failed: {e}"),
            ),
        }
    }

    async fn approval_respond(&self, req_id: Uuid, params: Option<Value>) -> Response {
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
            let decision = match decision.as_str() {
                "accept" => ApprovalDecision::Accept,
                "accept_for_session" => ApprovalDecision::AcceptForSession,
                "decline" => ApprovalDecision::Decline,
                "cancel" => ApprovalDecision::Cancel,
                _ => ApprovalDecision::Decline,
            };
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
        if self.use_roci() {
            let roci = self.roci.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    roci.shutdown().await;
                });
            }
        }
    }

    fn resolve_thread_id(&mut self, chat_id: &str, explicit: Option<&str>) -> Option<String> {
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

pub struct ChatService {
    core: Arc<Mutex<CodexChatCore>>,
}

pub struct AgentService {
    core: Arc<Mutex<CodexChatCore>>,
}

impl ChatService {
    #[allow(dead_code)]
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        Self {
            core: Arc::new(Mutex::new(CodexChatCore::new(
                outbound_tx,
                store,
                homie_config,
                exec_policy,
            ))),
        }
    }

    pub fn new_shared(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> (Self, AgentService) {
        let core = Arc::new(Mutex::new(CodexChatCore::new(
            outbound_tx,
            store,
            homie_config,
            exec_policy,
        )));
        (Self { core: core.clone() }, AgentService { core })
    }

    fn shutdown_core(&mut self) {
        let core = self.core.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut core = core.lock().await;
                core.shutdown();
            });
        } else if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }

    fn reap_core(&mut self) -> Vec<ReapEvent> {
        self.core
            .try_lock()
            .map(|mut core| core.reap())
            .unwrap_or_default()
    }
}

impl AgentService {
    #[allow(dead_code)]
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        Self {
            core: Arc::new(Mutex::new(CodexChatCore::new(
                outbound_tx,
                store,
                homie_config,
                exec_policy,
            ))),
        }
    }

    fn shutdown_core(&mut self) {
        let core = self.core.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut core = core.lock().await;
                core.shutdown();
            });
        } else if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }

    fn reap_core(&mut self) -> Vec<ReapEvent> {
        self.core
            .try_lock()
            .map(|mut core| core.reap())
            .unwrap_or_default()
    }
}

fn debug_enabled() -> bool {
    matches!(
        std::env::var("HOMIE_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    ) || matches!(
        std::env::var("HOME_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    )
}

impl ServiceHandler for ChatService {
    fn namespace(&self) -> &str {
        "chat"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let method = method.to_string();
        Box::pin(async move {
            let mut core = self.core.lock().await;
            match method.as_str() {
                "chat.create" => core.chat_create(id).await,
                "chat.resume" => core.chat_resume(id, params).await,
                "chat.message.send" => core.chat_message_send(id, params).await,
                "chat.cancel" => core.chat_cancel(id, params).await,
                "chat.approval.respond" => core.approval_respond(id, params).await,
                "chat.list" => core.chat_list(id),
                "chat.thread.read" => core.chat_thread_read(id, params).await,
                "chat.thread.list" => core.chat_thread_list(id, params).await,
                "chat.thread.archive" => core.chat_thread_archive(id, params).await,
                "chat.thread.rename" => core.chat_thread_rename(id, params).await,
                "chat.settings.update" => core.chat_settings_update(id, params),
                "chat.files.search" => core.chat_files_search(id, params),
                "chat.account.read" => core.chat_account_read(id).await,
                "chat.account.list" => core.chat_account_list(id).await,
                "chat.account.login.start" => core.chat_account_login_start(id, params).await,
                "chat.account.login.poll" => core.chat_account_login_poll(id, params).await,
                "chat.skills.list" => core.chat_skills_list(id, params).await,
                "chat.model.list" => core.chat_model_list(id, params).await,
                "chat.tools.list" => core.chat_tools_list(id, params).await,
                "chat.collaboration.mode.list" => {
                    core.chat_collaboration_mode_list(id, params).await
                }
                "chat.skills.config.write" => core.chat_skills_config_write(id, params).await,
                _ => Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {method}"),
                ),
            }
        })
    }

    fn handle_binary(&mut self, _frame: &BinaryFrame) {
        tracing::debug!("chat service does not handle binary frames");
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        self.reap_core()
    }

    fn shutdown(&mut self) {
        self.shutdown_core();
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
            let mut core = self.core.lock().await;
            match canonical.as_str() {
                "agent.chat.create" => core.chat_create(id).await,
                "agent.chat.message.send" => core.chat_message_send(id, params).await,
                "agent.chat.cancel" => core.chat_cancel(id, params).await,
                "agent.chat.approval.respond" => core.approval_respond(id, params).await,
                "agent.chat.list" => core.chat_list(id),
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
        self.reap_core()
    }

    fn shutdown(&mut self) {
        self.shutdown_core();
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }
}

impl Drop for AgentService {
    fn drop(&mut self) {
        if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }
}

/// Background task: reads Codex events and forwards them as Homie Event
/// messages via the outbound WS channel.
async fn event_forwarder_loop(
    mut event_rx: mpsc::Receiver<CodexEvent>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: Arc<dyn Store>,
    response_sender: CodexResponseSender,
    exec_policy: Arc<ExecPolicy>,
    homie_config: Arc<HomieConfig>,
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
                if let Some(argv) = approval_command_argv(&raw_params) {
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

fn approval_command_argv(params: &Value) -> Option<Vec<String>> {
    let command = params.get("command")?.as_str()?;
    if command.trim().is_empty() {
        return None;
    }
    shell_words::split(command).ok()
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

fn codex_model() -> String {
    std::env::var("HOMIE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.1-codex".to_string())
}

fn extract_id_from_result(
    value: &Value,
    direct_keys: &[&str],
    nested_keys: &[(&str, &str)],
) -> Option<String> {
    for key in direct_keys {
        if let Some(id) = value.get(*key).and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    for (outer, inner) in nested_keys {
        if let Some(id) = value
            .get(*outer)
            .and_then(|v| v.get(*inner))
            .and_then(|v| v.as_str())
        {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

// -- Param parsing helpers ------------------------------------------------

fn parse_message_params(
    params: &Option<Value>,
) -> Option<(
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Value>,
    bool,
)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let message = p.get("message")?.as_str()?.to_string();
    let model = p
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let effort = p
        .get("effort")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let approval_policy = p
        .get("approval_policy")
        .or_else(|| p.get("approvalPolicy"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let collaboration_mode = p
        .get("collaboration_mode")
        .or_else(|| p.get("collaborationMode"))
        .cloned();
    let inject = p.get("inject").and_then(|v| v.as_bool()).unwrap_or(false);
    Some((
        chat_id,
        message,
        model,
        effort,
        approval_policy,
        collaboration_mode,
        inject,
    ))
}

fn build_chat_settings(
    model: Option<&String>,
    effort: Option<&String>,
    approval_policy: Option<&String>,
    collaboration_mode: Option<&Value>,
) -> Option<Value> {
    let mut map = Map::new();
    if let Some(model) = model {
        map.insert("model".into(), json!(model));
    }
    if let Some(effort) = effort {
        map.insert("effort".into(), json!(effort));
    }
    if let Some(approval_policy) = approval_policy {
        map.insert("approval_policy".into(), json!(approval_policy));
    }
    if let Some(collaboration_mode) = collaboration_mode {
        map.insert("collaboration_mode".into(), collaboration_mode.clone());
    }
    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

fn merge_settings(existing: Option<Value>, updates: Value) -> Value {
    match (existing, updates) {
        (Some(Value::Object(mut base)), Value::Object(update)) => {
            for (key, value) in update {
                if value.is_null() {
                    base.remove(&key);
                } else {
                    base.insert(key, value);
                }
            }
            Value::Object(base)
        }
        (_, update) => update,
    }
}

fn parse_cancel_params(params: &Option<Value>) -> Option<(String, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let turn_id = p.get("turn_id")?.as_str()?.to_string();
    Some((chat_id, turn_id))
}

fn parse_settings_update_params(params: &Option<Value>) -> Option<(String, Value)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let settings = p.get("settings")?.clone();
    Some((chat_id, settings))
}

fn parse_files_search_params(
    params: &Option<Value>,
) -> Option<(String, String, usize, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let query = p.get("query")?.as_str()?.to_string();
    let limit = p
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(200) as usize)
        .unwrap_or(40);
    let base_path = p
        .get("base_path")
        .or_else(|| p.get("basePath"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    Some((chat_id, query, limit, base_path))
}

fn parse_tool_channel(params: &Option<Value>) -> String {
    params
        .as_ref()
        .and_then(|value| value.get("channel"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_TOOL_CHANNEL.to_string())
}

fn parse_resume_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

fn parse_thread_read_params(
    params: &Option<Value>,
) -> Option<(Option<String>, Option<String>, bool)> {
    let p = params.as_ref()?;
    let chat_id = p
        .get("chat_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let include_turns = p
        .get("include_turns")
        .or_else(|| p.get("includeTurns"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if chat_id.is_none() && thread_id.is_none() {
        None
    } else {
        Some((chat_id, thread_id, include_turns))
    }
}

fn parse_thread_archive_params(params: &Option<Value>) -> Option<(String, Option<String>)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id))
}

fn parse_thread_rename_params(params: &Option<Value>) -> Option<(String, Option<String>, String)> {
    let p = params.as_ref()?;
    let chat_id = p.get("chat_id")?.as_str()?.to_string();
    let title = p
        .get("title")
        .or_else(|| p.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;
    let thread_id = p
        .get("thread_id")
        .or_else(|| p.get("threadId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((chat_id, thread_id, title))
}

fn parse_approval_params(params: &Option<Value>) -> Option<(CodexRequestId, String)> {
    let p = params.as_ref()?;
    let raw = p.get("codex_request_id")?;
    let codex_request_id = if let Some(num) = raw.as_u64() {
        CodexRequestId::Number(num)
    } else if let Some(num) = raw.as_i64() {
        if num < 0 {
            return None;
        }
        CodexRequestId::Number(num as u64)
    } else if let Some(text) = raw.as_str() {
        CodexRequestId::Text(text.to_string())
    } else {
        return None;
    };
    let decision = p.get("decision")?.as_str()?.to_string();
    Some((codex_request_id, decision))
}

fn parse_account_provider_params(
    params: &Option<Value>,
) -> Option<(String, String, Map<String, Value>)> {
    let p = params.as_ref()?.as_object()?;
    let provider_raw = p.get("provider")?.as_str()?;
    let provider_id = normalize_provider_id(provider_raw)?;
    let profile = p
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    Some((provider_id, profile, p.clone()))
}

fn normalize_provider_id(raw: &str) -> Option<String> {
    match raw.trim() {
        "openai-codex" | "openai_codex" => Some("openai-codex".to_string()),
        "github-copilot" | "github_copilot" => Some("github-copilot".to_string()),
        "claude-code" | "claude_code" => Some("claude-code".to_string()),
        _ => None,
    }
}

fn parse_device_code_session(
    params: &Map<String, Value>,
    provider_id: &str,
) -> Option<DeviceCodeSession> {
    let session = params
        .get("session")
        .and_then(|v| v.as_object())
        .unwrap_or(params);
    let device_code = get_string(session, &["device_code", "deviceCode"])?;
    let user_code = get_string(session, &["user_code", "userCode"])?;
    let verification_url = get_string(session, &["verification_url", "verificationUrl"])?;
    let interval_secs = get_u64(session, &["interval_secs", "intervalSecs"])?;
    let expires_at_str = get_string(session, &["expires_at", "expiresAt"])?;
    let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
        .ok()?
        .with_timezone(&Utc);
    Some(DeviceCodeSession {
        provider: provider_id.to_string(),
        verification_url,
        user_code,
        device_code,
        interval_secs,
        expires_at,
    })
}

fn device_code_session_json(session: &DeviceCodeSession) -> Value {
    json!({
        "provider": session.provider,
        "verification_url": session.verification_url,
        "user_code": session.user_code,
        "device_code": session.device_code,
        "interval_secs": session.interval_secs,
        "expires_at": session.expires_at.to_rfc3339(),
    })
}

fn device_code_poll_json(poll: DeviceCodePoll) -> Value {
    match poll {
        DeviceCodePoll::Pending { interval_secs } => {
            json!({ "status": "pending", "interval_secs": interval_secs })
        }
        DeviceCodePoll::SlowDown { interval_secs } => {
            json!({ "status": "slow_down", "interval_secs": interval_secs })
        }
        DeviceCodePoll::Authorized { .. } => json!({ "status": "authorized" }),
        DeviceCodePoll::AccessDenied => json!({ "status": "denied" }),
        DeviceCodePoll::Expired => json!({ "status": "expired" }),
    }
}

fn get_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn get_u64(map: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(num) = value.as_u64() {
                return Some(num);
            }
            if let Some(text) = value.as_str() {
                if let Ok(num) = text.parse::<u64>() {
                    return Some(num);
                }
            }
        }
    }
    None
}

fn extract_thread_id(params: &Value) -> Option<String> {
    params
        .get("threadId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("thread_id").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

fn extract_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("turn_id").and_then(|v| v.as_str()))
        .or_else(|| {
            params
                .get("turn")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
}

fn extract_attached_folder(settings: Option<&Value>) -> Option<String> {
    let settings = settings?;
    let attachments = settings.get("attachments")?;
    if let Some(folder) = attachments.get("folder").and_then(|v| v.as_str()) {
        if !folder.trim().is_empty() {
            return Some(folder.to_string());
        }
    }
    let folders = attachments.get("folders").and_then(|v| v.as_array())?;
    folders
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| !v.trim().is_empty())
        .map(|v| v.to_string())
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".cache"
    )
}

fn normalize_search_root(base: &str) -> PathBuf {
    let trimmed = base.trim();
    let home_dir = crate::paths::user_home_dir();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home_dir.as_ref() {
            return home.join(rest);
        }
    }
    if trimmed == "~" {
        if let Some(home) = home_dir.as_ref() {
            return home.to_path_buf();
        }
    }
    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(path);
        }
    }
    path
}

fn search_files_in_folder(base: &str, query: &str, limit: usize) -> Result<Vec<Value>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let base_path = normalize_search_root(base);
    if !base_path.is_dir() {
        return Ok(Vec::new());
    }

    let mut queue = VecDeque::new();
    queue.push_back(base_path.clone());
    let mut results = Vec::new();
    let mut visited = 0usize;
    let query_lower = query.to_lowercase();

    while let Some(dir) = queue.pop_front() {
        if visited > 25_000 || results.len() >= limit {
            break;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if results.len() >= limit {
                break;
            }
            visited = visited.saturating_add(1);
            if visited > 25_000 {
                break;
            }
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                queue.push_back(path.clone());
            }
            if !file_type.is_file() && !file_type.is_dir() {
                continue;
            }
            let rel = match path.strip_prefix(&base_path) {
                Ok(p) => p,
                Err(_) => Path::new(&name),
            };
            let rel_str = rel.to_string_lossy().to_string();
            let haystack = format!("{name} {rel_str}").to_lowercase();
            if !haystack.contains(&query_lower) {
                continue;
            }
            visited += 1;
            let kind = if file_type.is_dir() {
                "directory"
            } else {
                "file"
            };
            results.push(json!({
                "name": name,
                "path": path.to_string_lossy(),
                "relative_path": rel_str,
                "type": kind,
            }));
        }
    }

    Ok(results)
}

fn list_homie_skills() -> Result<Vec<Value>, String> {
    let dir = homie_skills_dir()?;
    let mut skills = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("read skills dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read skills dir entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => continue,
        };
        let path_str = path.to_string_lossy().to_string();
        skills.push(json!({ "name": name, "path": path_str }));
    }
    skills.sort_by(|a, b| {
        let a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a.cmp(b)
    });
    Ok(skills)
}

fn roci_model_catalog(providers: &ProvidersConfig) -> Vec<Value> {
    let mut models = Vec::new();
    let mut default_set = false;
    let has_openai_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    if has_openai_key {
        let openai_models = [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4.1",
            "gpt-4.1-mini",
            "gpt-4.1-nano",
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5.2",
            "o1",
            "o1-mini",
            "o1-pro",
            "o3",
            "o3-mini",
            "o4-mini",
        ];
        for model_id in openai_models {
            let model = format!("openai:{model_id}");
            let is_default = !default_set && model_id == "gpt-4o-mini";
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "display_name": model_id,
                "is_default": is_default,
            }));
        }
    }

    if providers.openai_codex.enabled {
        let codex_models = [
            "gpt-5.2-codex",
            "gpt-5.1-codex",
            "gpt-5.1-codex-mini",
            "gpt-5.1-codex-max",
        ];
        for model_id in codex_models {
            let model = format!("openai-codex:{model_id}");
            let is_default = !default_set && model_id == "gpt-5.1-codex";
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "display_name": format!("{model_id} (Codex)"),
                "is_default": is_default,
            }));
        }
    }

    if providers.github_copilot.enabled {
        let compat_models = ["gpt-4o", "gpt-4o-mini", "gpt-4.1", "gpt-4.1-mini"];
        for model_id in compat_models {
            let model = format!("openai-compatible:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "display_name": format!("{model_id} (Copilot)"),
                "is_default": false,
            }));
        }
    }

    if providers.claude_code.enabled {
        let claude_models = [
            "claude-sonnet-4-5-20250514",
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-20250514",
            "claude-haiku-3-5-20241022",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
        ];
        for model_id in claude_models {
            let model = format!("anthropic:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "display_name": model_id,
                "is_default": false,
            }));
        }
    }

    models
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
            codex_method_to_topics("item/agentMessage/delta"),
            Some(("chat.message.delta", "agent.chat.delta"))
        );
    }

    #[test]
    fn codex_method_maps_turn_events() {
        assert_eq!(
            codex_method_to_topics("turn/started"),
            Some(("chat.turn.started", "agent.chat.turn.started"))
        );
        assert_eq!(
            codex_method_to_topics("turn/completed"),
            Some(("chat.turn.completed", "agent.chat.turn.completed"))
        );
    }

    #[test]
    fn codex_method_maps_item_events() {
        assert_eq!(
            codex_method_to_topics("item/started"),
            Some(("chat.item.started", "agent.chat.item.started"))
        );
        assert_eq!(
            codex_method_to_topics("item/completed"),
            Some(("chat.item.completed", "agent.chat.item.completed"))
        );
    }

    #[test]
    fn codex_method_maps_approval_requests() {
        assert_eq!(
            codex_method_to_topics("item/commandExecution/requestApproval"),
            Some(("chat.approval.required", "agent.chat.approval.required"))
        );
        assert_eq!(
            codex_method_to_topics("item/fileChange/requestApproval"),
            Some(("chat.approval.required", "agent.chat.approval.required"))
        );
    }

    #[test]
    fn codex_method_maps_output_deltas() {
        assert_eq!(
            codex_method_to_topics("item/commandExecution/outputDelta"),
            Some(("chat.command.output", "agent.chat.command.output"))
        );
        assert_eq!(
            codex_method_to_topics("item/fileChange/outputDelta"),
            Some(("chat.file.output", "agent.chat.file.output"))
        );
    }

    #[test]
    fn codex_method_maps_token_usage_updates() {
        assert_eq!(
            codex_method_to_topics("thread/tokenUsage/updated"),
            Some(("chat.token.usage.updated", "agent.chat.token.usage.updated"))
        );
    }

    #[test]
    fn codex_method_maps_reasoning_and_plan() {
        assert_eq!(
            codex_method_to_topics("item/reasoning/summaryTextDelta"),
            Some(("chat.reasoning.delta", "agent.chat.reasoning.delta"))
        );
        assert_eq!(
            codex_method_to_topics("turn/diff/updated"),
            Some(("chat.diff.updated", "agent.chat.diff.updated"))
        );
        assert_eq!(
            codex_method_to_topics("turn/plan/updated"),
            Some(("chat.plan.updated", "agent.chat.plan.updated"))
        );
    }

    #[test]
    fn unknown_codex_method_returns_none() {
        assert_eq!(codex_method_to_topics("unknown/method"), None);
    }

    #[test]
    fn parse_message_params_extracts_chat_id_and_message() {
        let params = Some(json!({
            "chat_id": "abc-123",
            "message": "hello world"
        }));
        let (chat_id, message, model, effort, approval_policy, collaboration_mode, inject) =
            parse_message_params(&params).unwrap();
        assert_eq!(chat_id, "abc-123");
        assert_eq!(message, "hello world");
        assert!(model.is_none());
        assert!(effort.is_none());
        assert!(approval_policy.is_none());
        assert!(collaboration_mode.is_none());
        assert!(!inject);
    }

    #[test]
    fn parse_message_params_returns_none_when_missing_fields() {
        assert!(parse_message_params(&None).is_none());
        assert!(parse_message_params(&Some(json!({"chat_id": "x"}))).is_none());
        assert!(parse_message_params(&Some(json!({"message": "x"}))).is_none());
    }

    #[test]
    fn parse_message_params_reads_inject_flag() {
        let params = Some(json!({
            "chat_id": "abc-123",
            "message": "hello world",
            "inject": true
        }));
        let (_, _, _, _, _, _, inject) = parse_message_params(&params).unwrap();
        assert!(inject);
    }

    #[test]
    fn parse_tool_channel_defaults_to_web() {
        assert_eq!(parse_tool_channel(&None), "web");
        assert_eq!(parse_tool_channel(&Some(json!({}))), "web");
        assert_eq!(parse_tool_channel(&Some(json!({"channel": "   "}))), "web");
    }

    #[test]
    fn parse_tool_channel_normalizes_value() {
        assert_eq!(
            parse_tool_channel(&Some(json!({"channel": "  DisCord "}))),
            "discord"
        );
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
        assert!(matches!(id, CodexRequestId::Number(42)));
        assert_eq!(decision, "accept");
    }

    #[test]
    fn parse_approval_params_returns_none_for_invalid_input() {
        assert!(parse_approval_params(&None).is_none());
        assert!(
            parse_approval_params(&Some(json!({"codex_request_id": { "bad": true } }))).is_none()
        );
    }

    #[test]
    fn parse_approval_params_accepts_string_id() {
        let params = Some(json!({
            "codex_request_id": "abc-123",
            "decision": "decline"
        }));
        let (id, decision) = parse_approval_params(&params).unwrap();
        assert!(matches!(id, CodexRequestId::Text(ref s) if s == "abc-123"));
        assert_eq!(decision, "decline");
    }

    #[tokio::test]
    async fn agent_service_returns_error_for_unknown_method() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(
            tx,
            make_store(),
            Arc::new(HomieConfig::default()),
            Arc::new(ExecPolicy::empty()),
        );
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "agent.unknown.method", None).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn agent_service_namespace_is_agent() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let svc = AgentService::new(
            tx,
            make_store(),
            Arc::new(HomieConfig::default()),
            Arc::new(ExecPolicy::empty()),
        );
        assert_eq!(svc.namespace(), "agent");
    }

    #[test]
    fn agent_service_reap_returns_empty_initially() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(
            tx,
            make_store(),
            Arc::new(HomieConfig::default()),
            Arc::new(ExecPolicy::empty()),
        );
        assert!(svc.reap().is_empty());
    }

    #[tokio::test]
    async fn chat_list_returns_empty_initially() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = AgentService::new(
            tx,
            make_store(),
            Arc::new(HomieConfig::default()),
            Arc::new(ExecPolicy::empty()),
        );
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "agent.chat.list", None).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let chats = result["chats"].as_array().unwrap();
        assert!(chats.is_empty());
    }

    #[tokio::test]
    async fn chat_tools_list_returns_expected_shape() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut svc = ChatService::new(
            tx,
            make_store(),
            Arc::new(HomieConfig::default()),
            Arc::new(ExecPolicy::empty()),
        );
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "chat.tools.list", None).await;
        assert!(resp.error.is_none());
        let result = resp.result.expect("result");
        let data = result["data"].as_array().expect("data array");
        assert!(!data.is_empty());
        let read = data
            .iter()
            .find(|tool| tool.get("name").and_then(|v| v.as_str()) == Some("read"))
            .expect("read tool");
        assert_eq!(read["provider"], "core");
        assert_eq!(read["provider_dynamic"], false);
        assert!(read["input_schema"].is_object());
        assert!(!data
            .iter()
            .any(|tool| tool.get("provider").and_then(|v| v.as_str()) == Some("openclaw_browser")));
    }

    #[tokio::test]
    async fn chat_tools_list_applies_channel_gating() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut config = HomieConfig::default();
        config.tools.providers.insert(
            "openclaw_browser".to_string(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: vec!["discord".to_string()],
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let mut svc = ChatService::new(
            tx,
            make_store(),
            Arc::new(config),
            Arc::new(ExecPolicy::empty()),
        );

        let web_resp = svc
            .handle_request(Uuid::new_v4(), "chat.tools.list", None)
            .await;
        assert!(web_resp.error.is_none());
        let web_tools = web_resp.result.expect("web result")["data"]
            .as_array()
            .expect("web data")
            .clone();
        assert!(!web_tools.iter().any(|tool| {
            tool.get("provider").and_then(|v| v.as_str()) == Some("openclaw_browser")
        }));

        let discord_resp = svc
            .handle_request(
                Uuid::new_v4(),
                "chat.tools.list",
                Some(json!({ "channel": "discord" })),
            )
            .await;
        assert!(discord_resp.error.is_none());
        let discord_tools = discord_resp.result.expect("discord result")["data"]
            .as_array()
            .expect("discord data")
            .clone();
        assert!(discord_tools.iter().any(|tool| {
            tool.get("provider").and_then(|v| v.as_str()) == Some("openclaw_browser")
        }));
    }

    #[tokio::test]
    async fn chat_account_list_reports_provider_statuses() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut config = HomieConfig::default();
        let tmp_dir = std::env::temp_dir().join(format!("homie-auth-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        config.paths.credentials_dir = Some(tmp_dir.to_string_lossy().to_string());
        let mut svc = ChatService::new(
            tx,
            make_store(),
            Arc::new(config),
            Arc::new(ExecPolicy::empty()),
        );
        let id = Uuid::new_v4();
        let resp = svc.handle_request(id, "chat.account.list", None).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let providers = result["providers"].as_array().unwrap();
        assert_eq!(providers.len(), 3);
        for provider in providers {
            assert!(provider.get("logged_in").is_some());
        }
    }
}

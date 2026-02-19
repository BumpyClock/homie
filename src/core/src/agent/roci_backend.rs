use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use roci::agent_loop::{
    ApprovalDecision, ApprovalPolicy, ApprovalRequest, LoopRunner, RunEvent, RunEventPayload,
    RunHooks, RunLifecycle, RunRequest, Runner,
};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::tools::Tool;
use roci::types::{AgentToolCall, ContentPart, ModelMessage, Role};
use roci::types::{GenerationSettings, ReasoningEffort};

use crate::agent::tools::{build_tools, ToolContext};
use crate::outbound::OutboundMessage;
use crate::storage::{ChatRawEventRecord, Store};
use crate::ExecPolicy;

const DEFAULT_ROCI_MODEL: &str = "openai-codex:gpt-5.1-codex";
const TOOL_OUTPUT_RETENTION_TURNS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatBackend {
    Codex,
    Roci,
}

impl ChatBackend {
    pub fn from_env() -> Self {
        match std::env::var("HOMIE_CHAT_BACKEND") {
            Ok(value) if value.trim().eq_ignore_ascii_case("codex") => ChatBackend::Codex,
            Ok(value) if value.trim().eq_ignore_ascii_case("roci") => ChatBackend::Roci,
            Ok(_) => ChatBackend::Roci,
            Err(_) => ChatBackend::Roci,
        }
    }
}

#[derive(Clone)]
pub struct RociBackend {
    state: Arc<Mutex<RociState>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    store: Arc<dyn Store>,
    tools: Vec<Arc<dyn Tool>>,
    processes: Arc<crate::agent::tools::ProcessRegistry>,
    exec_policy: Arc<ExecPolicy>,
    raw_events_enabled: bool,
}

struct PendingRun {
    chat_id: String,
    thread_id: String,
    turn_id: String,
    assistant_item_id: String,
    messages: Vec<ModelMessage>,
    model: LanguageModel,
    settings: GenerationSettings,
    approval_policy: ApprovalPolicy,
    config: RociConfig,
    collaboration_mode: Option<String>,
}

impl RociBackend {
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        exec_policy: Arc<ExecPolicy>,
        homie_config: Arc<crate::HomieConfig>,
        tool_channel: Option<String>,
    ) -> Self {
        let processes = Arc::new(crate::agent::tools::ProcessRegistry::new());
        let tool_ctx = ToolContext::with_processes_and_channel(
            processes.clone(),
            homie_config.clone(),
            tool_channel.as_deref(),
        )
        .with_store(store.clone());
        let tools = match build_tools(tool_ctx, &homie_config) {
            Ok(tools) => tools,
            Err(error) => {
                tracing::error!(%error, "failed to build tool registry; using empty tool set");
                Vec::new()
            }
        };
        Self {
            state: Arc::new(Mutex::new(RociState::default())),
            outbound_tx,
            store,
            tools,
            processes,
            exec_policy,
            raw_events_enabled: homie_config.raw_events_enabled(),
        }
    }

    pub async fn ensure_thread(&self, thread_id: &str) {
        {
            let state = self.state.lock().await;
            if state.threads.contains_key(thread_id) {
                return;
            }
        }

        let restored = match self.store.get_chat_thread_state(thread_id) {
            Ok(Some(value)) => decode_persisted_thread_state(thread_id, value),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%thread_id, "failed to load persisted roci thread state: {error}");
                None
            }
        }
        .or_else(|| backfill_thread_state_from_raw_events(&self.store, thread_id));

        let mut state = self.state.lock().await;
        if state.threads.contains_key(thread_id) {
            return;
        }
        state.threads.insert(
            thread_id.to_string(),
            restored.unwrap_or_else(|| RociThreadState::new(thread_id.to_string())),
        );
    }

    pub async fn thread_read(&self, thread_id: &str) -> Option<Value> {
        let state = self.state.lock().await;
        let thread = state.threads.get(thread_id)?;
        serde_json::to_value(&thread.thread).ok()
    }

    pub async fn thread_list(&self) -> Vec<Value> {
        let state = self.state.lock().await;
        state
            .threads
            .values()
            .filter_map(|thread| serde_json::to_value(&thread.thread).ok())
            .collect()
    }

    pub async fn thread_archive(&self, thread_id: &str) {
        let evicted = {
            let mut state = self.state.lock().await;
            state.threads.remove(thread_id);
            state.runs.retain(|_, run| run.thread_id != thread_id);
            state.run_queue.remove(thread_id);
            state.active_threads.remove(thread_id);
            state.approval_cache.remove(thread_id);
            state.tool_output_cache.remove(thread_id)
        };
        if let Some(turns) = evicted {
            for turn in turns {
                for process_id in turn.process_ids {
                    self.processes.remove(&process_id);
                }
            }
        }
        if let Err(error) = self.store.delete_chat_thread_state(thread_id) {
            tracing::warn!(%thread_id, "failed to delete persisted roci thread state: {error}");
        }
    }

    async fn register_tool_turn(&self, thread_id: &str, turn_id: &str) {
        let evicted = {
            let mut state = self.state.lock().await;
            let deque = state
                .tool_output_cache
                .entry(thread_id.to_string())
                .or_default();
            deque.push_back(ToolOutputRetention {
                turn_id: turn_id.to_string(),
                process_ids: Vec::new(),
            });
            let mut evicted = Vec::new();
            while deque.len() > TOOL_OUTPUT_RETENTION_TURNS {
                if let Some(turn) = deque.pop_front() {
                    evicted.push(turn);
                }
            }
            evicted
        };
        for turn in evicted {
            for process_id in turn.process_ids {
                self.processes.remove(&process_id);
            }
        }
    }

    async fn record_tool_process(&self, thread_id: &str, turn_id: &str, process_id: String) {
        let evicted = {
            let mut state = self.state.lock().await;
            let deque = state
                .tool_output_cache
                .entry(thread_id.to_string())
                .or_default();
            if let Some(entry) = deque
                .iter_mut()
                .rev()
                .find(|entry| entry.turn_id == turn_id)
            {
                entry.process_ids.push(process_id);
            } else {
                deque.push_back(ToolOutputRetention {
                    turn_id: turn_id.to_string(),
                    process_ids: vec![process_id],
                });
            }
            if deque.len() > TOOL_OUTPUT_RETENTION_TURNS {
                deque.pop_front()
            } else {
                None
            }
        };
        if let Some(turn) = evicted {
            for process_id in turn.process_ids {
                self.processes.remove(&process_id);
            }
        }
    }

    async fn persist_thread_state(&self, thread_id: &str) {
        let snapshot = {
            let state = self.state.lock().await;
            state
                .threads
                .get(thread_id)
                .map(PersistedThreadSnapshot::from_thread_state)
        };
        persist_thread_snapshot(&self.store, thread_id, snapshot);
    }

    pub async fn start_run(
        &self,
        chat_id: &str,
        thread_id: &str,
        message: &str,
        model: LanguageModel,
        settings: GenerationSettings,
        approval_policy: ApprovalPolicy,
        config: RociConfig,
        collaboration_mode: Option<String>,
        system_prompt: Option<String>,
    ) -> Result<String, String> {
        self.ensure_thread(thread_id).await;
        let system_prompt = system_prompt
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());
        let (turn_id, user_item_id, assistant_item_id) = {
            let mut state = self.state.lock().await;
            let thread = state
                .threads
                .get_mut(thread_id)
                .ok_or_else(|| "thread missing".to_string())?;
            if let Some(prompt) = system_prompt.as_ref() {
                let has_system = thread.messages.iter().any(|msg| msg.role == Role::System);
                if !has_system {
                    thread
                        .messages
                        .insert(0, ModelMessage::system(prompt.clone()));
                }
            }
            let turn_id = Uuid::new_v4().to_string();
            let user_item_id = Uuid::new_v4().to_string();
            let assistant_item_id = Uuid::new_v4().to_string();
            let user_item = RociItem::user(user_item_id.clone(), message.to_string());
            let assistant_item = RociItem::assistant(assistant_item_id.clone(), String::new());
            let turn = RociTurn::new(turn_id.clone(), vec![user_item, assistant_item]);
            thread.thread.turns.push(turn);
            thread.thread.updated_at = now_unix();
            thread
                .messages
                .push(ModelMessage::user(message.to_string()));
            thread.last_assistant_item_id = Some(assistant_item_id.clone());
            (turn_id, user_item_id, assistant_item_id)
        };
        self.persist_thread_state(thread_id).await;

        if self.raw_events_enabled {
            persist_roci_raw_event(
                &self.store,
                &turn_id,
                thread_id,
                "turn/started",
                serde_json::json!({ "threadId": thread_id, "turnId": turn_id.clone() }),
            );
            persist_roci_raw_event(
                &self.store,
                &turn_id,
                thread_id,
                "item/completed",
                serde_json::json!({
                    "threadId": thread_id,
                    "turnId": turn_id.clone(),
                    "item": {
                        "id": user_item_id.clone(),
                        "type": "userMessage",
                        "content": [{ "type": "text", "text": message }],
                    },
                }),
            );
        }

        self.register_tool_turn(thread_id, &turn_id).await;

        emit_turn_started(&self.outbound_tx, &self.store, chat_id, thread_id, &turn_id);
        emit_user_item(
            &self.outbound_tx,
            &self.store,
            chat_id,
            thread_id,
            &turn_id,
            user_item_id,
            message,
        );
        emit_assistant_item(
            &self.outbound_tx,
            &self.store,
            chat_id,
            thread_id,
            &turn_id,
            assistant_item_id.clone(),
        );

        let messages = {
            let state = self.state.lock().await;
            state
                .threads
                .get(thread_id)
                .map(|thread| thread.messages.clone())
                .unwrap_or_default()
        };
        let pending = PendingRun {
            chat_id: chat_id.to_string(),
            thread_id: thread_id.to_string(),
            turn_id: turn_id.clone(),
            assistant_item_id: assistant_item_id.clone(),
            messages,
            model,
            settings,
            approval_policy,
            config,
            collaboration_mode,
        };

        let mut pending = Some(pending);
        let should_start = {
            let mut state = self.state.lock().await;
            if state.active_threads.contains_key(thread_id) {
                state
                    .run_queue
                    .entry(thread_id.to_string())
                    .or_default()
                    .push_back(pending.take().unwrap());
                false
            } else {
                state
                    .active_threads
                    .insert(thread_id.to_string(), turn_id.clone());
                true
            }
        };

        if !should_start {
            if debug_enabled() {
                tracing::debug!(
                    %chat_id,
                    %thread_id,
                    %turn_id,
                    "roci run queued"
                );
            }
            return Ok(turn_id);
        }

        if let Err(err) = self.clone().start_run_inner(pending.take().unwrap()).await {
            {
                let mut state = self.state.lock().await;
                if state.active_threads.get(thread_id) == Some(&turn_id) {
                    state.active_threads.remove(thread_id);
                }
            }
            if let Some(next) = self.dequeue_next_run(thread_id).await {
                if let Err(next_err) = self.clone().start_run_inner(next).await {
                    if debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            error = %next_err,
                            "roci queued run start failed"
                        );
                    }
                }
            }
            return Err(err);
        }

        Ok(turn_id)
    }

    pub async fn queue_message(
        &self,
        chat_id: &str,
        thread_id: &str,
        message: &str,
    ) -> Option<String> {
        let (turn_id, item_id, queued) = {
            let mut state = self.state.lock().await;
            let turn_id = state.active_threads.get(thread_id)?.clone();
            let turn_exists = state
                .threads
                .get(thread_id)
                .map(|thread| thread.thread.turns.iter().any(|turn| turn.id == turn_id))
                .unwrap_or(false);
            if !turn_exists {
                return None;
            }
            let queued = {
                let run = state.runs.get(&turn_id)?;
                let handle = run.handle.as_ref()?;
                handle.queue_message(ModelMessage::user(message.to_string()))
            };
            if !queued {
                return None;
            }
            let item_id = Uuid::new_v4().to_string();
            let thread = state.threads.get_mut(thread_id)?;
            if let Some(turn) = thread
                .thread
                .turns
                .iter_mut()
                .find(|turn| turn.id == turn_id)
            {
                turn.items
                    .push(RociItem::user(item_id.clone(), message.to_string()));
            }
            thread.thread.updated_at = now_unix();
            thread
                .messages
                .push(ModelMessage::user(message.to_string()));
            (turn_id, item_id, queued)
        };
        self.persist_thread_state(thread_id).await;

        if queued {
            emit_user_item(
                &self.outbound_tx,
                &self.store,
                chat_id,
                thread_id,
                &turn_id,
                item_id,
                message,
            );
            return Some(turn_id);
        }

        None
    }

    async fn start_run_inner(self, pending: PendingRun) -> Result<(), String> {
        let run_id = Uuid::parse_str(&pending.turn_id).unwrap_or_else(|_| Uuid::new_v4());
        if debug_enabled() {
            tracing::debug!(
                chat_id = %pending.chat_id,
                thread_id = %pending.thread_id,
                turn_id = %pending.turn_id,
                run_id = %run_id,
                model = %pending.model.to_string(),
                "roci start_run"
            );
        }

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RunEvent>();
        let event_sink = Arc::new(move |event: RunEvent| {
            let _ = event_tx.send(event);
        });

        let state = self.state.clone();
        let exec_policy = self.exec_policy.clone();
        let thread_id_for_cache = pending.thread_id.clone();
        let approval_handler: roci::agent_loop::ApprovalHandler = Arc::new(move |request| {
            let state = state.clone();
            let exec_policy = exec_policy.clone();
            let thread_id = thread_id_for_cache.clone();
            Box::pin(async move {
                if request.kind == roci::agent_loop::ApprovalKind::CommandExecution {
                    if let Some(argv) = approval_command_argv(&request.payload) {
                        if exec_policy.is_allowed(&argv) {
                            if debug_enabled() {
                                tracing::debug!(argv = ?argv, "roci execpolicy auto-approve");
                            }
                            return ApprovalDecision::Accept;
                        }
                    }
                }
                let cache_key = approval_cache_key(&request);
                if let Some(key) = cache_key.as_ref() {
                    let cached = {
                        let guard = state.lock().await;
                        guard
                            .approval_cache
                            .get(&thread_id)
                            .map(|set| set.contains(key))
                            .unwrap_or(false)
                    };
                    if cached {
                        return ApprovalDecision::Accept;
                    }
                }
                let (tx, rx) = oneshot::channel();
                {
                    let mut guard = state.lock().await;
                    guard.approvals.insert(request.id.clone(), tx);
                }
                let decision = rx.await.unwrap_or(ApprovalDecision::Decline);
                let mut guard = state.lock().await;
                guard.approvals.remove(&request.id);
                if matches!(decision, ApprovalDecision::AcceptForSession) {
                    if let Some(key) = cache_key {
                        guard
                            .approval_cache
                            .entry(thread_id)
                            .or_default()
                            .insert(key);
                    }
                }
                decision
            })
        });

        let mut run_request = RunRequest::new(pending.model, pending.messages);
        run_request.run_id = run_id;
        run_request.settings = pending.settings;
        run_request.tools = self.tools.clone();
        run_request.approval_policy = pending.approval_policy;
        run_request.event_sink = Some(event_sink);
        run_request.approval_handler = Some(approval_handler);
        run_request.hooks = RunHooks {
            compaction: Some(Arc::new(|messages| compact_messages(messages))),
            tool_result_persist: Some(Arc::new(|result| trim_tool_result(result))),
        };

        let runner = LoopRunner::new(pending.config);
        let handle = runner
            .start(run_request)
            .await
            .map_err(|e| format!("run start failed: {e}"))?;

        {
            let mut state = self.state.lock().await;
            state.runs.insert(
                pending.turn_id.clone(),
                RociRunState {
                    thread_id: pending.thread_id.clone(),
                    handle: Some(handle),
                },
            );
        }

        let outbound = self.outbound_tx.clone();
        let store = self.store.clone();
        let state = self.state.clone();
        let thread_id = pending.thread_id.clone();
        let turn_id_clone = pending.turn_id.clone();
        let chat_id = pending.chat_id.clone();
        let assistant_item_id_clone = pending.assistant_item_id.clone();
        let collaboration_mode = pending.collaboration_mode.clone();
        let raw_events_enabled = self.raw_events_enabled;
        let backend = self.clone();

        tokio::spawn(async move {
            let mut assistant_text = String::new();
            let mut tool_calls: HashMap<String, ToolCallInfo> = HashMap::new();
            while let Some(event) = event_rx.recv().await {
                match event.payload {
                    RunEventPayload::AssistantDelta { text } => {
                        if !text.is_empty() {
                            if debug_enabled() {
                                tracing::debug!(
                                    %chat_id,
                                    %thread_id,
                                    %turn_id_clone,
                                    delta_len = text.len(),
                                    "roci assistant delta"
                                );
                            }
                            assistant_text.push_str(&text);
                            emit_message_delta(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                &assistant_item_id_clone,
                                &text,
                            );
                            if raw_events_enabled {
                                persist_roci_raw_event(
                                    &store,
                                    &turn_id_clone,
                                    &thread_id,
                                    "item/agentMessage/delta",
                                    serde_json::json!({
                                        "threadId": thread_id,
                                        "turnId": turn_id_clone,
                                        "itemId": assistant_item_id_clone,
                                        "delta": text,
                                    }),
                                );
                            }
                        }
                    }
                    RunEventPayload::ReasoningDelta { text } => {
                        if !text.is_empty() {
                            if debug_enabled() {
                                tracing::debug!(
                                    %chat_id,
                                    %thread_id,
                                    %turn_id_clone,
                                    delta_len = text.len(),
                                    "roci reasoning delta"
                                );
                            }
                            emit_reasoning_delta(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                &assistant_item_id_clone,
                                &text,
                            );
                        }
                    }
                    RunEventPayload::ToolCallStarted { call } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                tool_call_id = %call.id,
                                tool = %call.name,
                                "roci tool call started"
                            );
                        }
                        {
                            let mut guard = state.lock().await;
                            if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                if let Some(turn) = thread
                                    .thread
                                    .turns
                                    .iter_mut()
                                    .find(|turn| turn.id == turn_id_clone)
                                {
                                    upsert_tool_item_started(
                                        turn,
                                        &call.id,
                                        &call.name,
                                        call.arguments.clone(),
                                    );
                                }
                                thread.messages.push(model_tool_call_message(&call));
                                thread.thread.updated_at = now_unix();
                            }
                        }
                        backend.persist_thread_state(&thread_id).await;
                        if raw_events_enabled {
                            persist_roci_raw_event(
                                &store,
                                &turn_id_clone,
                                &thread_id,
                                "item/started",
                                serde_json::json!({
                                    "threadId": thread_id,
                                    "turnId": turn_id_clone,
                                    "item": {
                                        "id": call.id.clone(),
                                        "type": "mcpToolCall",
                                        "tool": call.name.clone(),
                                        "status": "running",
                                        "input": call.arguments.clone(),
                                    },
                                }),
                            );
                        }
                        tool_calls.insert(
                            call.id.clone(),
                            ToolCallInfo {
                                name: call.name.clone(),
                                input: call.arguments.clone(),
                            },
                        );
                        emit_tool_item_started(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &call.id,
                            &call.name,
                            &call.arguments,
                        );
                    }
                    RunEventPayload::ToolResult { result } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                tool_call_id = %result.tool_call_id,
                                is_error = result.is_error,
                                result = %result.result,
                                "roci tool result"
                            );
                        }
                        let info = tool_calls.remove(&result.tool_call_id).unwrap_or_else(|| {
                            ToolCallInfo {
                                name: "tool".to_string(),
                                input: serde_json::Value::Null,
                            }
                        });
                        {
                            let mut guard = state.lock().await;
                            if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                if let Some(turn) = thread
                                    .thread
                                    .turns
                                    .iter_mut()
                                    .find(|turn| turn.id == turn_id_clone)
                                {
                                    upsert_tool_item_completed(
                                        turn,
                                        &result.tool_call_id,
                                        &info.name,
                                        info.input.clone(),
                                        result.result.clone(),
                                        result.is_error,
                                    );
                                }
                                thread.messages.push(ModelMessage::tool_result(
                                    result.tool_call_id.clone(),
                                    result.result.clone(),
                                    result.is_error,
                                ));
                                thread.thread.updated_at = now_unix();
                            }
                        }
                        backend.persist_thread_state(&thread_id).await;
                        if raw_events_enabled {
                            let status = if result.is_error {
                                "failed"
                            } else {
                                "completed"
                            };
                            persist_roci_raw_event(
                                &store,
                                &turn_id_clone,
                                &thread_id,
                                "item/completed",
                                serde_json::json!({
                                    "threadId": thread_id,
                                    "turnId": turn_id_clone,
                                    "item": {
                                        "id": result.tool_call_id.clone(),
                                        "type": "mcpToolCall",
                                        "tool": info.name.clone(),
                                        "status": status,
                                        "input": info.input.clone(),
                                        "result": result.result.clone(),
                                        "error": result.is_error,
                                    },
                                }),
                            );
                        }
                        if let Some(process_id) =
                            exec_process_id_from_result(&info.name, &result.result)
                        {
                            backend
                                .record_tool_process(&thread_id, &turn_id_clone, process_id)
                                .await;
                        }
                        emit_tool_item_completed(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &result.tool_call_id,
                            &info.name,
                            &info.input,
                            &result.result,
                            result.is_error,
                        );
                        if info.name == "apply_patch" {
                            if let Some(diff) = result.result.get("diff").and_then(|v| v.as_str()) {
                                emit_diff_updated(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    diff,
                                );
                            }
                        }
                    }
                    RunEventPayload::PlanUpdated { plan } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                plan_len = plan.len(),
                                "roci plan updated"
                            );
                        }
                        emit_plan_updated(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &plan,
                        );
                    }
                    RunEventPayload::DiffUpdated { diff } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                diff_len = diff.len(),
                                "roci diff updated"
                            );
                        }
                        emit_diff_updated(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &diff,
                        );
                    }
                    RunEventPayload::ApprovalRequired { request } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                request_id = %request.id,
                                kind = ?request.kind,
                                "roci approval required"
                            );
                        }
                        emit_approval_required(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &request,
                        );
                    }
                    RunEventPayload::Lifecycle { state: lifecycle } => {
                        if debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                lifecycle = ?lifecycle,
                                "roci lifecycle event"
                            );
                        }
                        match lifecycle {
                            RunLifecycle::Completed => {
                                let snapshot = {
                                    let mut guard = state.lock().await;
                                    if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                        thread.update_assistant_text(
                                            &assistant_item_id_clone,
                                            &assistant_text,
                                        );
                                        thread
                                            .messages
                                            .push(ModelMessage::assistant(assistant_text.clone()));
                                        thread.thread.updated_at = now_unix();
                                    }
                                    let snapshot = guard
                                        .threads
                                        .get(&thread_id)
                                        .map(PersistedThreadSnapshot::from_thread_state);
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone)
                                    {
                                        guard.active_threads.remove(&thread_id);
                                    }
                                    snapshot
                                };
                                persist_thread_snapshot(&store, &thread_id, snapshot);
                                if raw_events_enabled {
                                    persist_roci_raw_event(
                                        &store,
                                        &turn_id_clone,
                                        &thread_id,
                                        "item/completed",
                                        serde_json::json!({
                                            "threadId": thread_id,
                                            "turnId": turn_id_clone,
                                            "item": {
                                                "id": assistant_item_id_clone,
                                                "type": "agentMessage",
                                                "text": assistant_text,
                                            },
                                        }),
                                    );
                                    let _ = store.prune_chat_raw_events(10);
                                }

                                emit_item_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    &assistant_item_id_clone,
                                    &assistant_text,
                                );
                                if collaboration_mode.as_deref() == Some("plan")
                                    && !assistant_text.trim().is_empty()
                                {
                                    emit_plan_updated(
                                        &outbound,
                                        &store,
                                        &chat_id,
                                        &thread_id,
                                        &turn_id_clone,
                                        &assistant_text,
                                    );
                                }
                                emit_turn_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    "completed",
                                );
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    spawn_next_run(
                                        backend.clone(),
                                        next,
                                        chat_id.clone(),
                                        thread_id.clone(),
                                    );
                                }
                                break;
                            }
                            RunLifecycle::Failed { error } => {
                                let failure_text = if assistant_text.trim().is_empty() {
                                    format!("Run failed: {error}")
                                } else {
                                    assistant_text.clone()
                                };
                                emit_error(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    error.clone(),
                                );
                                let snapshot = {
                                    let mut guard = state.lock().await;
                                    if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                        thread.update_assistant_text(
                                            &assistant_item_id_clone,
                                            &failure_text,
                                        );
                                        thread
                                            .messages
                                            .push(ModelMessage::assistant(failure_text.clone()));
                                        thread.thread.updated_at = now_unix();
                                    }
                                    let snapshot = guard
                                        .threads
                                        .get(&thread_id)
                                        .map(PersistedThreadSnapshot::from_thread_state);
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone)
                                    {
                                        guard.active_threads.remove(&thread_id);
                                    }
                                    snapshot
                                };
                                persist_thread_snapshot(&store, &thread_id, snapshot);
                                if raw_events_enabled {
                                    persist_roci_raw_event(
                                        &store,
                                        &turn_id_clone,
                                        &thread_id,
                                        "item/completed",
                                        serde_json::json!({
                                            "threadId": thread_id,
                                            "turnId": turn_id_clone,
                                            "item": {
                                                "id": assistant_item_id_clone,
                                                "type": "agentMessage",
                                                "text": failure_text,
                                            },
                                        }),
                                    );
                                    let _ = store.prune_chat_raw_events(10);
                                }
                                emit_item_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    &assistant_item_id_clone,
                                    &failure_text,
                                );
                                emit_turn_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    "failed",
                                );
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    spawn_next_run(
                                        backend.clone(),
                                        next,
                                        chat_id.clone(),
                                        thread_id.clone(),
                                    );
                                }
                                break;
                            }
                            RunLifecycle::Canceled => {
                                emit_turn_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    "canceled",
                                );
                                let snapshot = {
                                    let mut guard = state.lock().await;
                                    let snapshot = guard
                                        .threads
                                        .get(&thread_id)
                                        .map(PersistedThreadSnapshot::from_thread_state);
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone)
                                    {
                                        guard.active_threads.remove(&thread_id);
                                    }
                                    snapshot
                                };
                                persist_thread_snapshot(&store, &thread_id, snapshot);
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    spawn_next_run(
                                        backend.clone(),
                                        next,
                                        chat_id.clone(),
                                        thread_id.clone(),
                                    );
                                }
                                break;
                            }
                            RunLifecycle::Started => {}
                        }
                    }
                    RunEventPayload::ToolCallDelta { .. } => {}
                    RunEventPayload::ToolCallCompleted { .. } => {}
                    RunEventPayload::Error { message } => {
                        tracing::debug!(%chat_id, %thread_id, %turn_id_clone, "run error event: {message}");
                    }
                }
            }
        });

        Ok(())
    }

    async fn dequeue_next_run(&self, thread_id: &str) -> Option<PendingRun> {
        let mut state = self.state.lock().await;
        let next = state
            .run_queue
            .get_mut(thread_id)
            .and_then(|queue| queue.pop_front());
        if let Some(run) = next.as_ref() {
            state
                .active_threads
                .insert(thread_id.to_string(), run.turn_id.clone());
        }
        next
    }

    pub async fn cancel_run(&self, turn_id: &str) -> bool {
        let mut state = self.state.lock().await;
        if let Some(run) = state.runs.get_mut(turn_id) {
            if let Some(mut handle) = run.handle.take() {
                return handle.abort();
            }
        }
        let mut removed = false;
        for queue in state.run_queue.values_mut() {
            if let Some(idx) = queue.iter().position(|run| run.turn_id == turn_id) {
                queue.remove(idx);
                removed = true;
                break;
            }
        }
        removed
    }

    pub async fn shutdown(&self) {
        let mut state = self.state.lock().await;
        for run in state.runs.values_mut() {
            if let Some(mut handle) = run.handle.take() {
                handle.abort();
            }
        }
        state.runs.clear();
        state.run_queue.clear();
        state.active_threads.clear();
    }

    pub fn parse_model(input: Option<&String>) -> Result<LanguageModel, String> {
        let raw = input
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_roci_model());
        let normalized = if raw.contains(':') {
            raw
        } else {
            format!("openai:{raw}")
        };
        normalized
            .parse::<LanguageModel>()
            .map_err(|e| format!("invalid model: {e}"))
    }

    pub fn parse_settings(
        effort: Option<&String>,
        stream_idle_timeout_ms: Option<u64>,
    ) -> GenerationSettings {
        let mut settings = GenerationSettings::default();
        if let Some(effort) = effort {
            if let Ok(parsed) = effort.parse::<ReasoningEffort>() {
                settings.reasoning_effort = Some(parsed);
            }
        }
        settings.stream_idle_timeout_ms = stream_idle_timeout_ms;
        settings
    }

    pub fn parse_approval_policy(policy: Option<&String>) -> ApprovalPolicy {
        match policy.map(|p| p.trim().to_lowercase()) {
            Some(value) if value == "never" => ApprovalPolicy::Never,
            Some(value) if value == "always" => ApprovalPolicy::Always,
            Some(value) if value == "on-request" => ApprovalPolicy::Ask,
            Some(value) if value == "untrusted" => ApprovalPolicy::Ask,
            _ => ApprovalPolicy::Ask,
        }
    }

    pub fn parse_collaboration_mode(mode: Option<&Value>) -> Option<String> {
        let raw = mode?;
        if let Some(value) = raw.as_str() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_lowercase());
            }
        }
        let obj = raw.as_object()?;
        let value = obj
            .get("mode")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("id").and_then(|v| v.as_str()))?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    }

    pub async fn respond_approval(&self, request_id: &str, decision: ApprovalDecision) -> bool {
        let mut state = self.state.lock().await;
        if let Some(tx) = state.approvals.remove(request_id) {
            return tx.send(decision).is_ok();
        }
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedThreadSnapshot {
    thread: RociThread,
    #[serde(default)]
    messages: Vec<ModelMessage>,
    #[serde(default)]
    last_assistant_item_id: Option<String>,
}

impl PersistedThreadSnapshot {
    fn from_thread_state(state: &RociThreadState) -> Self {
        Self {
            thread: state.thread.clone(),
            messages: state.messages.clone(),
            last_assistant_item_id: state.last_assistant_item_id.clone(),
        }
    }

    fn into_thread_state(self, thread_id: &str) -> RociThreadState {
        let mut thread = self.thread;
        if thread.id != thread_id {
            thread.id = thread_id.to_string();
        }
        let messages = if self.messages.is_empty() && !thread.turns.is_empty() {
            model_messages_from_turns(&thread.turns)
        } else {
            self.messages
        };
        let last_assistant_item_id = self
            .last_assistant_item_id
            .or_else(|| last_assistant_item_id_from_turns(&thread.turns));
        RociThreadState {
            thread,
            messages,
            last_assistant_item_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PersistedThreadSnapshotPayload {
    Snapshot(PersistedThreadSnapshot),
    LegacyThread(RociThread),
}

fn decode_persisted_thread_state(thread_id: &str, value: Value) -> Option<RociThreadState> {
    let payload = match serde_json::from_value::<PersistedThreadSnapshotPayload>(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to decode persisted roci thread state: {error}");
            return None;
        }
    };
    let state = match payload {
        PersistedThreadSnapshotPayload::Snapshot(snapshot) => snapshot.into_thread_state(thread_id),
        PersistedThreadSnapshotPayload::LegacyThread(thread) => PersistedThreadSnapshot {
            messages: model_messages_from_turns(&thread.turns),
            last_assistant_item_id: last_assistant_item_id_from_turns(&thread.turns),
            thread,
        }
        .into_thread_state(thread_id),
    };
    Some(state)
}

fn persist_thread_snapshot(
    store: &Arc<dyn Store>,
    thread_id: &str,
    snapshot: Option<PersistedThreadSnapshot>,
) {
    let Some(snapshot) = snapshot else {
        return;
    };
    let value = match serde_json::to_value(snapshot) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to serialize roci thread state: {error}");
            return;
        }
    };
    if let Err(error) = store.upsert_chat_thread_state(thread_id, &value) {
        tracing::warn!(%thread_id, "failed to persist roci thread state: {error}");
    }
}

fn model_messages_from_turns(turns: &[RociTurn]) -> Vec<ModelMessage> {
    let mut messages = Vec::new();
    for turn in turns {
        for item in &turn.items {
            match item {
                RociItem::UserMessage { content, .. } => {
                    let text = content
                        .iter()
                        .map(|part| match part {
                            RociContent::Text { text } => text.as_str(),
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    messages.push(ModelMessage::user(text));
                }
                RociItem::AgentMessage { text, .. } => {
                    messages.push(ModelMessage::assistant(text.clone()));
                }
                RociItem::ToolCall {
                    id,
                    tool,
                    input,
                    result,
                    error,
                    ..
                } => {
                    messages.push(model_tool_call_message(&AgentToolCall {
                        id: id.clone(),
                        name: tool.clone(),
                        arguments: input.clone(),
                        recipient: None,
                    }));
                    if let Some(result) = result {
                        messages.push(ModelMessage::tool_result(
                            id.clone(),
                            result.clone(),
                            *error,
                        ));
                    }
                }
            }
        }
    }
    messages
}

fn model_tool_call_message(call: &AgentToolCall) -> ModelMessage {
    ModelMessage {
        role: Role::Assistant,
        content: vec![ContentPart::ToolCall(call.clone())],
        name: None,
        timestamp: None,
    }
}

fn backfill_thread_state_from_raw_events(
    store: &Arc<dyn Store>,
    thread_id: &str,
) -> Option<RociThreadState> {
    let events = match store.list_chat_raw_events(thread_id, 8_000) {
        Ok(events) => events,
        Err(error) => {
            tracing::warn!(%thread_id, "failed to read raw events for backfill: {error}");
            return None;
        }
    };
    if events.is_empty() {
        return None;
    }

    let mut thread = RociThread {
        id: thread_id.to_string(),
        created_at: events
            .first()
            .map(|e| e.created_at)
            .unwrap_or_else(now_unix),
        updated_at: events.last().map(|e| e.created_at).unwrap_or_else(now_unix),
        turns: Vec::new(),
    };
    let mut turn_indices: HashMap<String, usize> = HashMap::new();

    for event in &events {
        apply_raw_event_to_thread(&mut thread, &mut turn_indices, event);
    }

    if thread.turns.is_empty() {
        return None;
    }

    let messages = model_messages_from_turns(&thread.turns);
    let last_assistant_item_id = last_assistant_item_id_from_turns(&thread.turns);
    let state = RociThreadState {
        thread,
        messages,
        last_assistant_item_id,
    };
    persist_thread_snapshot(
        store,
        thread_id,
        Some(PersistedThreadSnapshot::from_thread_state(&state)),
    );
    Some(state)
}

fn apply_raw_event_to_thread(
    thread: &mut RociThread,
    turn_indices: &mut HashMap<String, usize>,
    event: &ChatRawEventRecord,
) {
    let params = &event.params;
    let turn_id = raw_event_turn_id(params);

    match event.method.as_str() {
        "turn/started" => {
            if let Some(turn_id) = turn_id {
                ensure_turn(thread, turn_indices, &turn_id);
            }
        }
        "item/started" | "item/completed" => {
            let Some(turn_id) = turn_id else { return };
            let Some(item) = params.get("item").and_then(|v| v.as_object()) else {
                return;
            };
            let Some(item_id) = item.get("id").and_then(|v| v.as_str()) else {
                return;
            };
            let Some(item_type) = item.get("type").and_then(|v| v.as_str()) else {
                return;
            };
            let turn = ensure_turn(thread, turn_indices, &turn_id);
            match item_type {
                "userMessage" => {
                    let text = extract_user_item_text(item);
                    upsert_user_item(turn, item_id, text);
                }
                "agentMessage" => {
                    let text = item
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    upsert_assistant_item(turn, item_id, text, false);
                }
                "mcpToolCall" => {
                    let tool = item
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool")
                        .to_string();
                    let status = item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or(if event.method == "item/completed" {
                            "completed"
                        } else {
                            "running"
                        })
                        .to_string();
                    let input = item
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let result = item.get("result").cloned();
                    let is_error = item.get("error").and_then(|v| v.as_bool()).unwrap_or(false);
                    upsert_tool_item(turn, item_id, tool, status, input, result, is_error);
                }
                _ => {}
            }
        }
        "item/agentMessage/delta" | "chat.message.delta" => {
            let Some(turn_id) = turn_id else { return };
            let Some(item_id) = raw_event_item_id(params) else {
                return;
            };
            let delta = params
                .get("delta")
                .or_else(|| params.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if delta.is_empty() {
                return;
            }
            let turn = ensure_turn(thread, turn_indices, &turn_id);
            upsert_assistant_item(turn, &item_id, delta.to_string(), true);
        }
        _ => {}
    }
}

fn ensure_turn<'a>(
    thread: &'a mut RociThread,
    turn_indices: &mut HashMap<String, usize>,
    turn_id: &str,
) -> &'a mut RociTurn {
    if let Some(index) = turn_indices.get(turn_id).copied() {
        return &mut thread.turns[index];
    }
    let index = thread.turns.len();
    thread
        .turns
        .push(RociTurn::new(turn_id.to_string(), Vec::new()));
    turn_indices.insert(turn_id.to_string(), index);
    &mut thread.turns[index]
}

fn upsert_user_item(turn: &mut RociTurn, item_id: &str, text: String) {
    if let Some(existing) = turn.items.iter_mut().find_map(|item| match item {
        RociItem::UserMessage { id, content } if id == item_id => Some(content),
        _ => None,
    }) {
        existing.clear();
        existing.push(RociContent::Text { text });
        return;
    }
    turn.items.push(RociItem::user(item_id.to_string(), text));
}

fn upsert_assistant_item(turn: &mut RociTurn, item_id: &str, text: String, append: bool) {
    if let Some(existing) = turn.items.iter_mut().find_map(|item| match item {
        RociItem::AgentMessage { id, text } if id == item_id => Some(text),
        _ => None,
    }) {
        if append {
            existing.push_str(&text);
        } else {
            *existing = text;
        }
        return;
    }
    turn.items
        .push(RociItem::assistant(item_id.to_string(), text));
}

fn upsert_tool_item_started(turn: &mut RociTurn, item_id: &str, tool: &str, input: Value) {
    upsert_tool_item(
        turn,
        item_id,
        tool.to_string(),
        "running".to_string(),
        input,
        None,
        false,
    );
}

fn upsert_tool_item_completed(
    turn: &mut RociTurn,
    item_id: &str,
    tool: &str,
    input: Value,
    result: Value,
    is_error: bool,
) {
    let status = if is_error { "failed" } else { "completed" };
    upsert_tool_item(
        turn,
        item_id,
        tool.to_string(),
        status.to_string(),
        input,
        Some(result),
        is_error,
    );
}

fn upsert_tool_item(
    turn: &mut RociTurn,
    item_id: &str,
    tool: String,
    status: String,
    input: Value,
    result: Option<Value>,
    is_error: bool,
) {
    if let Some(existing) = turn.items.iter_mut().find_map(|item| match item {
        RociItem::ToolCall {
            id,
            tool,
            status,
            input,
            result,
            error,
        } if id == item_id => Some((tool, status, input, result, error)),
        _ => None,
    }) {
        *existing.0 = tool;
        *existing.1 = status;
        *existing.2 = input;
        *existing.4 = is_error;
        if let Some(result) = result {
            *existing.3 = Some(result);
        }
        return;
    }

    turn.items.push(RociItem::tool_call(
        item_id.to_string(),
        tool,
        status,
        input,
        result,
        is_error,
    ));
}

fn raw_event_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

fn raw_event_item_id(params: &Value) -> Option<String> {
    params
        .get("itemId")
        .or_else(|| params.get("item_id"))
        .or_else(|| params.get("item").and_then(|i| i.get("id")))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

fn extract_user_item_text(item: &serde_json::Map<String, Value>) -> String {
    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    let Some(parts) = item.get("content").and_then(|v| v.as_array()) else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| {
            if part
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|kind| kind.eq_ignore_ascii_case("text"))
            {
                return part.get("text").and_then(|v| v.as_str());
            }
            None
        })
        .collect::<Vec<_>>()
        .join("")
}

fn last_assistant_item_id_from_turns(turns: &[RociTurn]) -> Option<String> {
    turns.iter().rev().find_map(|turn| {
        turn.items.iter().rev().find_map(|item| match item {
            RociItem::AgentMessage { id, .. } => Some(id.clone()),
            _ => None,
        })
    })
}

#[derive(Default)]
struct RociState {
    threads: HashMap<String, RociThreadState>,
    runs: HashMap<String, RociRunState>,
    run_queue: HashMap<String, VecDeque<PendingRun>>,
    active_threads: HashMap<String, String>,
    approvals: HashMap<String, oneshot::Sender<ApprovalDecision>>,
    approval_cache: HashMap<String, HashSet<String>>,
    tool_output_cache: HashMap<String, VecDeque<ToolOutputRetention>>,
}

struct RociRunState {
    thread_id: String,
    handle: Option<roci::agent_loop::RunHandle>,
}

struct ToolCallInfo {
    name: String,
    input: serde_json::Value,
}

struct ToolOutputRetention {
    turn_id: String,
    process_ids: Vec<String>,
}

#[derive(Clone)]
struct RociThreadState {
    thread: RociThread,
    messages: Vec<ModelMessage>,
    last_assistant_item_id: Option<String>,
}

impl RociThreadState {
    fn new(thread_id: String) -> Self {
        let now = now_unix();
        Self {
            thread: RociThread {
                id: thread_id,
                created_at: now,
                updated_at: now,
                turns: Vec::new(),
            },
            messages: Vec::new(),
            last_assistant_item_id: None,
        }
    }

    fn update_assistant_text(&mut self, item_id: &str, text: &str) {
        for turn in &mut self.thread.turns {
            for item in &mut turn.items {
                if let RociItem::AgentMessage { id, text: body } = item {
                    if id == item_id {
                        *body = text.to_string();
                        return;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RociThread {
    id: String,
    created_at: u64,
    updated_at: u64,
    turns: Vec<RociTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RociTurn {
    id: String,
    items: Vec<RociItem>,
}

impl RociTurn {
    fn new(id: String, items: Vec<RociItem>) -> Self {
        Self { id, items }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum RociItem {
    #[serde(rename = "userMessage")]
    UserMessage {
        id: String,
        content: Vec<RociContent>,
    },
    #[serde(rename = "agentMessage")]
    AgentMessage { id: String, text: String },
    #[serde(rename = "mcpToolCall")]
    ToolCall {
        id: String,
        tool: String,
        status: String,
        #[serde(default)]
        input: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(default)]
        error: bool,
    },
}

impl RociItem {
    fn user(id: String, text: String) -> Self {
        Self::UserMessage {
            id,
            content: vec![RociContent::Text { text }],
        }
    }

    fn assistant(id: String, text: String) -> Self {
        Self::AgentMessage { id, text }
    }

    fn tool_call(
        id: String,
        tool: String,
        status: String,
        input: serde_json::Value,
        result: Option<serde_json::Value>,
        error: bool,
    ) -> Self {
        Self::ToolCall {
            id,
            tool,
            status,
            input,
            result,
            error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum RociContent {
    #[serde(rename = "text")]
    Text { text: String },
}

fn default_roci_model() -> String {
    std::env::var("HOMIE_ROCI_MODEL").unwrap_or_else(|_| DEFAULT_ROCI_MODEL.to_string())
}

fn spawn_next_run(backend: RociBackend, next: PendingRun, chat_id: String, thread_id: String) {
    tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        if let Err(err) = handle.block_on(backend.start_run_inner(next)) {
            if debug_enabled() {
                tracing::debug!(
                    %chat_id,
                    %thread_id,
                    error = %err,
                    "roci queued run start failed"
                );
            }
        }
    });
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn emit_error(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    message: String,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "status": "failed" })),
    );
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.error",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "message": message })),
    );
}

fn emit_turn_started(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id })),
    );
}

fn emit_turn_completed(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    status: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.turn.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "status": status })),
    );
}

fn emit_message_delta(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.message.delta",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": item_id,
            "delta": delta,
        })),
    );
}

fn emit_reasoning_delta(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.reasoning.delta",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": item_id,
            "delta": delta,
        })),
    );
}

fn emit_user_item(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: String,
    text: &str,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "userMessage",
        "content": [{ "type": "text", "text": text }],
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

fn emit_assistant_item(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: String,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "agentMessage",
        "text": "",
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

fn emit_item_completed(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    text: &str,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "agentMessage",
        "text": text,
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStore;
    use roci::auth::{providers::openai_codex::OpenAiCodexAuth, FileTokenStore, TokenStoreConfig};
    use roci::config::RociConfig;
    use roci::types::ContentPart;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    #[tokio::test]
    async fn queue_message_appends_to_active_turn() {
        let (outbound_tx, _outbound_rx) = mpsc::channel(4);
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        let backend = RociBackend::new(
            outbound_tx,
            store.clone(),
            Arc::new(ExecPolicy::empty()),
            Arc::new(crate::HomieConfig::default()),
            None,
        );
        let thread_id = "thread-1";
        let chat_id = "chat-1";

        backend.ensure_thread(thread_id).await;
        let turn_id = Uuid::new_v4().to_string();
        let assistant_id = Uuid::new_v4().to_string();
        let (handle, _abort_rx, _result_tx, mut input_rx) =
            roci::agent_loop::RunHandle::new(Uuid::new_v4());
        {
            let mut state = backend.state.lock().await;
            let thread = state.threads.get_mut(thread_id).expect("thread");
            thread.thread.turns.push(RociTurn::new(
                turn_id.clone(),
                vec![RociItem::assistant(assistant_id, String::new())],
            ));
            state
                .active_threads
                .insert(thread_id.to_string(), turn_id.clone());
            state.runs.insert(
                turn_id.clone(),
                RociRunState {
                    thread_id: thread_id.to_string(),
                    handle: Some(handle),
                },
            );
        }

        let queued = backend
            .queue_message(chat_id, thread_id, "hello world")
            .await;
        assert_eq!(queued.as_deref(), Some(turn_id.as_str()));

        let message = input_rx.try_recv().expect("queued message");
        assert_eq!(message.role, Role::User);
        assert_eq!(
            message.content,
            vec![ContentPart::Text {
                text: "hello world".to_string()
            }]
        );

        let state = backend.state.lock().await;
        let thread = state.threads.get(thread_id).expect("thread");
        let turn = thread
            .thread
            .turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .expect("turn");
        assert_eq!(turn.items.len(), 2);

        let persisted = store
            .get_chat_thread_state(thread_id)
            .expect("persisted state read")
            .expect("persisted state");
        let snapshot: PersistedThreadSnapshot =
            serde_json::from_value(persisted).expect("snapshot decode");
        assert_eq!(snapshot.thread.turns.len(), 1);
        assert_eq!(snapshot.messages.len(), 1);
    }

    #[tokio::test]
    async fn ensure_thread_rehydrates_persisted_snapshot() {
        let thread_id = "persisted-thread";
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        let turn_id = Uuid::new_v4().to_string();
        let user_item_id = Uuid::new_v4().to_string();
        let assistant_item_id = Uuid::new_v4().to_string();
        let snapshot = PersistedThreadSnapshot {
            thread: RociThread {
                id: thread_id.to_string(),
                created_at: 10,
                updated_at: 20,
                turns: vec![RociTurn::new(
                    turn_id.clone(),
                    vec![
                        RociItem::user(user_item_id, "hello".to_string()),
                        RociItem::assistant(assistant_item_id.clone(), "world".to_string()),
                    ],
                )],
            },
            messages: vec![
                ModelMessage::system("system prompt"),
                ModelMessage::user("hello"),
                ModelMessage::assistant("world"),
            ],
            last_assistant_item_id: Some(assistant_item_id.clone()),
        };
        store
            .upsert_chat_thread_state(
                thread_id,
                &serde_json::to_value(snapshot.clone()).expect("snapshot encode"),
            )
            .expect("snapshot write");

        let (outbound_tx, _outbound_rx) = mpsc::channel(4);
        let backend = RociBackend::new(
            outbound_tx,
            store,
            Arc::new(ExecPolicy::empty()),
            Arc::new(crate::HomieConfig::default()),
            None,
        );

        backend.ensure_thread(thread_id).await;

        let loaded = backend.thread_read(thread_id).await.expect("thread");
        assert_eq!(loaded["id"], thread_id);
        assert_eq!(loaded["turns"].as_array().map(|v| v.len()), Some(1));

        let state = backend.state.lock().await;
        let thread = state.threads.get(thread_id).expect("state thread");
        assert_eq!(thread.messages, snapshot.messages);
        assert_eq!(
            thread.last_assistant_item_id.as_deref(),
            Some(assistant_item_id.as_str())
        );
    }

    #[tokio::test]
    async fn ensure_thread_rehydrates_tool_items_into_model_messages() {
        let thread_id = "persisted-tool-thread";
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        let turn_id = Uuid::new_v4().to_string();
        let user_item_id = Uuid::new_v4().to_string();
        let assistant_item_id = Uuid::new_v4().to_string();
        let tool_item_id = "call_123".to_string();
        let snapshot = PersistedThreadSnapshot {
            thread: RociThread {
                id: thread_id.to_string(),
                created_at: 10,
                updated_at: 20,
                turns: vec![RociTurn::new(
                    turn_id,
                    vec![
                        RociItem::user(user_item_id, "hello".to_string()),
                        RociItem::assistant(assistant_item_id.clone(), "let me check".to_string()),
                        RociItem::tool_call(
                            tool_item_id.clone(),
                            "ls".to_string(),
                            "completed".to_string(),
                            serde_json::json!({"path": ".", "limit": 20}),
                            Some(serde_json::json!({"entries": ["src", "Cargo.toml"]})),
                            false,
                        ),
                    ],
                )],
            },
            messages: Vec::new(),
            last_assistant_item_id: Some(assistant_item_id),
        };
        store
            .upsert_chat_thread_state(
                thread_id,
                &serde_json::to_value(snapshot).expect("snapshot encode"),
            )
            .expect("snapshot write");

        let (outbound_tx, _outbound_rx) = mpsc::channel(4);
        let backend = RociBackend::new(
            outbound_tx,
            store,
            Arc::new(ExecPolicy::empty()),
            Arc::new(crate::HomieConfig::default()),
            None,
        );

        backend.ensure_thread(thread_id).await;

        let state = backend.state.lock().await;
        let thread = state.threads.get(thread_id).expect("state thread");
        let has_tool_call = thread.messages.iter().any(|message| {
            message.role == Role::Assistant
                && message.content.iter().any(|part| {
                    matches!(
                        part,
                        ContentPart::ToolCall(call) if call.id == tool_item_id && call.name == "ls"
                    )
                })
        });
        let has_tool_result = thread.messages.iter().any(|message| {
            message.role == Role::Tool
                && message.content.iter().any(|part| {
                    matches!(
                        part,
                        ContentPart::ToolResult(result)
                            if result.tool_call_id == tool_item_id && !result.is_error
                    )
                })
        });
        assert!(
            has_tool_call,
            "expected tool call message in rehydrated context"
        );
        assert!(
            has_tool_result,
            "expected tool result message in rehydrated context"
        );
    }

    #[tokio::test]
    async fn ensure_thread_backfills_from_raw_events() {
        let thread_id = "raw-thread";
        let turn_id = "raw-turn";
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        store
            .insert_chat_raw_event(
                "run-raw",
                thread_id,
                "turn/started",
                &serde_json::json!({"threadId": thread_id, "turnId": turn_id}),
            )
            .expect("turn started");
        store
            .insert_chat_raw_event(
                "run-raw",
                thread_id,
                "item/completed",
                &serde_json::json!({
                    "threadId": thread_id,
                    "turnId": turn_id,
                    "item": {
                        "id": "u1",
                        "type": "userMessage",
                        "content": [{"type": "text", "text": "hello"}]
                    }
                }),
            )
            .expect("user item");
        store
            .insert_chat_raw_event(
                "run-raw",
                thread_id,
                "item/completed",
                &serde_json::json!({
                    "threadId": thread_id,
                    "turnId": turn_id,
                    "item": {
                        "id": "a1",
                        "type": "agentMessage",
                        "text": "world"
                    }
                }),
            )
            .expect("assistant item");

        let (outbound_tx, _outbound_rx) = mpsc::channel(4);
        let backend = RociBackend::new(
            outbound_tx,
            store.clone(),
            Arc::new(ExecPolicy::empty()),
            Arc::new(crate::HomieConfig::default()),
            None,
        );

        backend.ensure_thread(thread_id).await;
        let loaded = backend.thread_read(thread_id).await.expect("thread");
        assert_eq!(loaded["id"], thread_id);
        assert_eq!(loaded["turns"].as_array().map(|v| v.len()), Some(1));

        let state = backend.state.lock().await;
        let thread = state.threads.get(thread_id).expect("state thread");
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.last_assistant_item_id.as_deref(), Some("a1"));

        let persisted = store
            .get_chat_thread_state(thread_id)
            .expect("persisted state query");
        assert!(persisted.is_some());
    }

    #[tokio::test]
    async fn thread_archive_deletes_persisted_state() {
        let thread_id = "archive-thread";
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        let snapshot = PersistedThreadSnapshot {
            thread: RociThread {
                id: thread_id.to_string(),
                created_at: 1,
                updated_at: 1,
                turns: Vec::new(),
            },
            messages: Vec::new(),
            last_assistant_item_id: None,
        };
        store
            .upsert_chat_thread_state(
                thread_id,
                &serde_json::to_value(snapshot).expect("snapshot encode"),
            )
            .expect("snapshot write");

        let (outbound_tx, _outbound_rx) = mpsc::channel(4);
        let backend = RociBackend::new(
            outbound_tx,
            store.clone(),
            Arc::new(ExecPolicy::empty()),
            Arc::new(crate::HomieConfig::default()),
            None,
        );
        backend.ensure_thread(thread_id).await;

        backend.thread_archive(thread_id).await;

        let persisted = store
            .get_chat_thread_state(thread_id)
            .expect("persisted state read");
        assert!(persisted.is_none());
    }

    fn live_enabled() -> bool {
        matches!(std::env::var("HOMIE_LIVE_TESTS").as_deref(), Ok("1"))
    }

    #[tokio::test]
    async fn live_tool_calls() {
        if !live_enabled() {
            eprintln!("skipping live test; set HOMIE_LIVE_TESTS=1");
            return;
        }

        let homie_config = Arc::new(crate::HomieConfig::load().expect("load homie config"));
        let store = Arc::new(SqliteStore::open_memory().expect("store"));
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(128);

        let backend = RociBackend::new(
            outbound_tx,
            store,
            Arc::new(ExecPolicy::empty()),
            homie_config.clone(),
            None,
        );

        let creds_dir = homie_config.credentials_dir().expect("credentials dir");
        let token_store = FileTokenStore::new(TokenStoreConfig::new(creds_dir));
        let auth = OpenAiCodexAuth::new(Arc::new(token_store.clone()));
        let _ = auth.import_codex_auth_json(None);
        let token = auth.get_token().await.expect("codex token");

        let config = RociConfig::from_env();
        config.set_api_key("openai-codex", token.access_token);
        if let Some(account_id) = token.account_id {
            config.set_account_id("openai-codex", account_id);
        }
        if config.get_base_url("openai-codex").is_none() {
            if let Some(base) = config.get_base_url("openai") {
                config.set_base_url("openai-codex", base);
            }
        }

        let model: LanguageModel = "openai-codex:gpt-5.2-codex".parse().expect("model parse");
        let settings = GenerationSettings::default();

        backend
            .start_run(
                "live-chat",
                "live-thread",
                "List the current directory using the ls tool.",
                model,
                settings,
                ApprovalPolicy::Always,
                config,
                None,
                Some(homie_config.chat.system_prompt.clone()),
            )
            .await
            .expect("start run");

        let mut saw_tool_result = false;
        let deadline = Duration::from_secs(60);
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            let msg = timeout(Duration::from_secs(2), outbound_rx.recv()).await;
            let Some(OutboundMessage::Event { topic, params }) = msg.ok().flatten() else {
                continue;
            };
            if topic == "chat.item.completed" {
                if let Some(params) = params.as_ref() {
                    if params
                        .get("item")
                        .and_then(|v| v.get("type"))
                        .and_then(|v| v.as_str())
                        == Some("mcpToolCall")
                    {
                        saw_tool_result = true;
                        break;
                    }
                }
            }
        }

        assert!(
            saw_tool_result,
            "did not receive tool result within timeout"
        );
    }
}

fn emit_tool_item_started(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    tool_name: &str,
    input: &Value,
) {
    let item = serde_json::json!({
        "id": item_id,
        "type": "mcpToolCall",
        "tool": tool_name,
        "status": "running",
        "input": input,
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.started",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

fn emit_tool_item_completed(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    tool_name: &str,
    input: &Value,
    result: &Value,
    is_error: bool,
) {
    let status = if is_error { "failed" } else { "completed" };
    let item = serde_json::json!({
        "id": item_id,
        "type": "mcpToolCall",
        "tool": tool_name,
        "status": status,
        "input": input,
        "result": result,
        "error": is_error,
    });
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.item.completed",
        Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id, "item": item })),
    );
}

fn emit_approval_required(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    request: &ApprovalRequest,
) {
    let (command, cwd) = approval_command_from_payload(&request.payload);
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.approval.required",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "itemId": request.id,
            "request_id": request.id,
            "codex_request_id": request.id,
            "reason": request.reason,
            "command": command,
            "cwd": cwd,
        })),
    );
}

fn emit_plan_updated(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    plan: &str,
) {
    let trimmed = plan.trim();
    let steps = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .lines()
            .filter_map(|line| {
                let step = line.trim().trim_start_matches("- ").trim();
                if step.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "step": step, "status": "pending" }))
                }
            })
            .collect::<Vec<_>>()
    };
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.plan.updated",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "plan": steps,
        })),
    );
}

fn emit_diff_updated(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    thread_id: &str,
    turn_id: &str,
    diff: &str,
) {
    emit_event(
        outbound,
        store,
        chat_id,
        "chat.diff.updated",
        Some(serde_json::json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "diff": diff,
        })),
    );
}

fn approval_command_from_payload(payload: &Value) -> (Option<String>, Option<String>) {
    let obj = match payload.as_object() {
        Some(obj) => obj,
        None => return (None, None),
    };
    let args = payload_arguments(payload)
        .and_then(|v| v.as_object())
        .or(Some(obj))
        .unwrap();
    let command = if let Some(argv) = obj.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    } else if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
        Some(cmd.to_string())
    } else if let Some(argv) = args.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    } else if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
        Some(cmd.to_string())
    } else if let Some(tool) = obj.get("tool_name").and_then(|v| v.as_str()) {
        Some(tool.to_string())
    } else {
        None
    };
    let cwd = obj
        .get("cwd")
        .and_then(|v| v.as_str())
        .or_else(|| args.get("cwd").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    (command, cwd)
}

fn approval_command_argv(payload: &Value) -> Option<Vec<String>> {
    let obj = payload.as_object()?;
    let args = payload_arguments(payload)
        .and_then(|v| v.as_object())
        .unwrap_or(obj);
    if let Some(argv) = obj.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    if let Some(argv) = args.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .or_else(|| args.get("command").and_then(|v| v.as_str()))?
        .trim();
    if command.is_empty() {
        return None;
    }
    shell_words::split(command).ok()
}

fn payload_arguments<'a>(payload: &'a Value) -> Option<&'a Value> {
    let obj = payload.as_object()?;
    obj.get("arguments")
        .or_else(|| obj.get("args"))
        .or_else(|| obj.get("input"))
}

fn approval_cache_key(request: &ApprovalRequest) -> Option<String> {
    let mut payload = request.payload.clone();
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("tool_call_id");
    }
    let normalized = normalize_json(payload);
    let kind = match request.kind {
        roci::agent_loop::ApprovalKind::CommandExecution => "command",
        roci::agent_loop::ApprovalKind::FileChange => "file",
        roci::agent_loop::ApprovalKind::Other => "other",
    };
    Some(format!("{kind}|{}", canonical_json(&normalized)))
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn normalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut normalized = serde_json::Map::new();
            for (key, value) in entries {
                normalized.insert(key, normalize_json(value));
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize_json).collect()),
        other => other,
    }
}

fn compact_messages(messages: &[ModelMessage]) -> Option<Vec<ModelMessage>> {
    if messages.len() <= 80 {
        return None;
    }
    Some(messages[messages.len().saturating_sub(80)..].to_vec())
}

fn trim_tool_result(mut result: roci::types::AgentToolResult) -> roci::types::AgentToolResult {
    if let Some(text) = result.result.as_str() {
        let truncated: String = text.chars().take(8000).collect();
        result.result = serde_json::Value::String(truncated);
        return result;
    }
    if let Some(obj) = result.result.as_object_mut() {
        if let Some(val) = obj.get_mut("output") {
            if let Some(text) = val.as_str() {
                let truncated: String = text.chars().take(8000).collect();
                *val = serde_json::Value::String(truncated);
            }
        }
    }
    result
}

fn exec_process_id_from_result(tool_name: &str, result: &serde_json::Value) -> Option<String> {
    if tool_name != "exec" {
        return None;
    }
    let status = result.get("status").and_then(|v| v.as_str());
    if status != Some("completed") {
        return None;
    }
    let truncated = result
        .get("stdout_truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || result
            .get("stderr_truncated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    if !truncated {
        return None;
    }
    result
        .get("process_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn debug_enabled() -> bool {
    matches!(std::env::var("HOMIE_DEBUG").as_deref(), Ok("1"))
        || matches!(std::env::var("HOME_DEBUG").as_deref(), Ok("1"))
}

fn emit_event(
    outbound: &mpsc::Sender<OutboundMessage>,
    store: &Arc<dyn Store>,
    chat_id: &str,
    topic: &str,
    params: Option<Value>,
) {
    if let Ok(Some(chat)) = store.get_chat(chat_id) {
        let next = chat.event_pointer.saturating_add(1);
        let _ = store.update_event_pointer(chat_id, next);
    }
    match outbound.try_send(OutboundMessage::event(topic, params)) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!(topic = topic, "backpressure: dropping chat event");
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {}
    }
}

fn persist_roci_raw_event(
    store: &Arc<dyn Store>,
    run_id: &str,
    thread_id: &str,
    method: &str,
    params: Value,
) {
    if let Err(error) = store.insert_chat_raw_event(run_id, thread_id, method, &params) {
        tracing::warn!(
            %thread_id,
            %method,
            "failed to persist roci raw event: {error}"
        );
    }
}

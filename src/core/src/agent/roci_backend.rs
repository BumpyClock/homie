use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use roci::agent_loop::{
    ApprovalDecision, ApprovalPolicy, ApprovalRequest, LoopRunner, RunEvent, RunEventPayload,
    RunLifecycle, RunRequest, Runner,
};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::tools::Tool;
use roci::types::{ModelMessage, Role};
use roci::types::{GenerationSettings, ReasoningEffort};

use crate::agent::tools::{build_tools, ToolContext};
use crate::outbound::OutboundMessage;
use crate::storage::Store;
use crate::ExecPolicy;

const DEFAULT_ROCI_MODEL: &str = "openai-codex:gpt-5.1-codex";

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
    exec_policy: Arc<ExecPolicy>,
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
}

impl RociBackend {
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        let tool_ctx = ToolContext::new();
        let tools = build_tools(tool_ctx);
        Self {
            state: Arc::new(Mutex::new(RociState::default())),
            outbound_tx,
            store,
            tools,
            exec_policy,
        }
    }

    pub async fn ensure_thread(&self, thread_id: &str) {
        let mut state = self.state.lock().await;
        state.threads.entry(thread_id.to_string()).or_insert_with(|| {
            RociThreadState::new(thread_id.to_string())
        });
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
        let mut state = self.state.lock().await;
        state.threads.remove(thread_id);
        state.runs.retain(|_, run| run.thread_id != thread_id);
        state.run_queue.remove(thread_id);
        state.active_threads.remove(thread_id);
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
                let has_system = thread
                    .messages
                    .iter()
                    .any(|msg| msg.role == Role::System);
                if !has_system {
                    thread.messages.insert(0, ModelMessage::system(prompt.clone()));
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
            thread.messages.push(ModelMessage::user(message.to_string()));
            thread.last_assistant_item_id = Some(assistant_item_id.clone());
            (turn_id, user_item_id, assistant_item_id)
        };

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

        if let Err(err) = self.start_run_inner(pending.take().unwrap()).await {
            {
                let mut state = self.state.lock().await;
                if state.active_threads.get(thread_id) == Some(&turn_id) {
                    state.active_threads.remove(thread_id);
                }
            }
            if let Some(next) = self.dequeue_next_run(thread_id).await {
                if let Err(next_err) = self.start_run_inner(next).await {
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

    async fn start_run_inner(&self, pending: PendingRun) -> Result<(), String> {
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
        let approval_handler: roci::agent_loop::ApprovalHandler = Arc::new(move |request| {
            let state = state.clone();
            let exec_policy = exec_policy.clone();
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
                let (tx, rx) = oneshot::channel();
                {
                    let mut guard = state.lock().await;
                    guard.approvals.insert(request.id.clone(), tx);
                }
                let decision = rx.await.unwrap_or(ApprovalDecision::Decline);
                let mut guard = state.lock().await;
                guard.approvals.remove(&request.id);
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
                        let info = tool_calls
                            .remove(&result.tool_call_id)
                            .unwrap_or_else(|| ToolCallInfo {
                                name: "tool".to_string(),
                                input: serde_json::Value::Null,
                            });
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
                                {
                                    let mut guard = state.lock().await;
                                    if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                        thread.update_assistant_text(&assistant_item_id_clone, &assistant_text);
                                        thread
                                            .messages
                                            .push(ModelMessage::assistant(assistant_text.clone()));
                                        thread.thread.updated_at = now_unix();
                                    }
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                        guard.active_threads.remove(&thread_id);
                                    }
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
                                emit_turn_completed(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    "completed",
                                );
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    if let Err(err) = backend.start_run_inner(next).await {
                                        if debug_enabled() {
                                            tracing::debug!(
                                                %chat_id,
                                                %thread_id,
                                                error = %err,
                                                "roci queued run start failed"
                                            );
                                        }
                                    }
                                }
                                break;
                            }
                            RunLifecycle::Failed { error } => {
                                emit_error(&outbound, &store, &chat_id, &thread_id, &turn_id_clone, error);
                                {
                                    let mut guard = state.lock().await;
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                        guard.active_threads.remove(&thread_id);
                                    }
                                }
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    if let Err(err) = backend.start_run_inner(next).await {
                                        if debug_enabled() {
                                            tracing::debug!(
                                                %chat_id,
                                                %thread_id,
                                                error = %err,
                                                "roci queued run start failed"
                                            );
                                        }
                                    }
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
                                {
                                    let mut guard = state.lock().await;
                                    guard.runs.remove(&turn_id_clone);
                                    if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                        guard.active_threads.remove(&thread_id);
                                    }
                                }
                                if let Some(next) = backend.dequeue_next_run(&thread_id).await {
                                    if let Err(err) = backend.start_run_inner(next).await {
                                        if debug_enabled() {
                                            tracing::debug!(
                                                %chat_id,
                                                %thread_id,
                                                error = %err,
                                                "roci queued run start failed"
                                            );
                                        }
                                    }
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

    pub fn parse_settings(effort: Option<&String>) -> GenerationSettings {
        let mut settings = GenerationSettings::default();
        if let Some(effort) = effort {
            if let Ok(parsed) = effort.parse::<ReasoningEffort>() {
                settings.reasoning_effort = Some(parsed);
            }
        }
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

    pub async fn respond_approval(&self, request_id: &str, decision: ApprovalDecision) -> bool {
        let mut state = self.state.lock().await;
        if let Some(tx) = state.approvals.remove(request_id) {
            return tx.send(decision).is_ok();
        }
        false
    }
}

#[derive(Default)]
struct RociState {
    threads: HashMap<String, RociThreadState>,
    runs: HashMap<String, RociRunState>,
    run_queue: HashMap<String, VecDeque<PendingRun>>,
    active_threads: HashMap<String, String>,
    approvals: HashMap<String, oneshot::Sender<ApprovalDecision>>,
}

struct RociRunState {
    thread_id: String,
    handle: Option<roci::agent_loop::RunHandle>,
}

struct ToolCallInfo {
    name: String,
    input: serde_json::Value,
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
    UserMessage { id: String, content: Vec<RociContent> },
    #[serde(rename = "agentMessage")]
    AgentMessage { id: String, text: String },
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
    } else if let Some(tool) = obj.get("tool_name").and_then(|v| v.as_str()) {
        Some(tool.to_string())
    } else {
        None
    };
    let cwd = obj
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (command, cwd)
}

fn approval_command_argv(payload: &Value) -> Option<Vec<String>> {
    let obj = payload.as_object()?;
    if let Some(argv) = obj.get("argv").and_then(|v| v.as_array()) {
        let parts: Vec<String> = argv
            .iter()
            .filter_map(|value| value.as_str().map(|s| s.to_string()))
            .collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    let command = obj.get("command")?.as_str()?.trim();
    if command.is_empty() {
        return None;
    }
    shell_words::split(command).ok()
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

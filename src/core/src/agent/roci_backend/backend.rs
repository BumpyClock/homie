use std::collections::HashMap;
use std::sync::Arc;

use roci::agent::{
    AgentConfig, AgentRuntime, AgentRuntimeEvent, AgentRuntimeEventPayload, ApprovalSnapshot,
    ChatRuntimeConfig, CollaborationMode, DiffSnapshot, EnqueueTurnRequest, ImportedThread,
    MessageSnapshot, PlanSnapshot, ReasoningSnapshot, ThreadId, ThreadSnapshot,
    ToolExecutionSnapshot, TurnId, TurnSnapshot,
};
use roci::agent_loop::{ApprovalDecision, ApprovalHandler, ApprovalPolicy};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::types::{GenerationSettings, ModelMessage, Role};
use serde_json::{json, Value};
use tokio::sync::{broadcast, oneshot, Mutex};
use uuid::Uuid;

use crate::agent::tools::{build_tools, ToolContext};
use crate::router::ReapEvent;
use crate::storage::Store;
use crate::ExecPolicy;

use super::events::{approval_cache_key, approval_command_argv};
use super::persistence::{
    decode_persisted_runtime_thread, delete_persisted_runtime_thread, persist_runtime_thread,
};

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
    inner: Arc<RociRuntimeRegistry>,
}

pub struct StartRunRequest<'a> {
    pub chat_id: &'a str,
    pub thread_id: &'a str,
    pub message: &'a str,
    pub model: LanguageModel,
    pub settings: GenerationSettings,
    pub approval_policy: ApprovalPolicy,
    pub config: RociConfig,
    pub collaboration_mode: Option<CollaborationMode>,
    pub system_prompt: Option<String>,
    pub tool_channel: Option<&'a str>,
}

struct RociRuntimeRegistry {
    store: Arc<dyn Store>,
    event_tx: broadcast::Sender<ReapEvent>,
    exec_policy: Arc<ExecPolicy>,
    homie_config: Arc<crate::HomieConfig>,
    entries: Mutex<HashMap<String, Arc<RuntimeEntry>>>,
}

struct RuntimeEntry {
    chat_id: Mutex<String>,
    thread_id: String,
    runtime: Arc<AgentRuntime>,
    approvals: Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalDecision>>>>,
    message_text: Mutex<HashMap<String, String>>,
    turn_ids: Mutex<HashMap<String, TurnId>>,
}

impl RociBackend {
    pub fn new(
        store: Arc<dyn Store>,
        event_tx: broadcast::Sender<ReapEvent>,
        exec_policy: Arc<ExecPolicy>,
        homie_config: Arc<crate::HomieConfig>,
    ) -> Self {
        Self {
            inner: Arc::new(RociRuntimeRegistry {
                store,
                event_tx,
                exec_policy,
                homie_config,
                entries: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn ensure_thread(
        &self,
        chat_id: &str,
        thread_id: &str,
        model: LanguageModel,
        config: RociConfig,
        system_prompt: Option<String>,
        tool_channel: Option<&str>,
    ) -> Result<(), String> {
        self.ensure_entry(
            chat_id,
            thread_id,
            model,
            config,
            system_prompt,
            tool_channel,
        )
        .await
        .map(|_| ())
    }

    async fn ensure_entry(
        &self,
        chat_id: &str,
        thread_id: &str,
        model: LanguageModel,
        config: RociConfig,
        system_prompt: Option<String>,
        tool_channel: Option<&str>,
    ) -> Result<Arc<RuntimeEntry>, String> {
        if let Some(entry) = self.inner.entries.lock().await.get(thread_id).cloned() {
            *entry.chat_id.lock().await = chat_id.to_string();
            return Ok(entry);
        }

        let thread_uuid = Uuid::parse_str(thread_id)
            .map_err(|_| format!("thread_id must be a uuid for roci runtime: {thread_id}"))?;
        let runtime_thread_id = ThreadId::from(thread_uuid);
        let approvals = Arc::new(Mutex::new(HashMap::new()));
        let approval_handler =
            build_approval_handler(approvals.clone(), self.inner.exec_policy.clone());
        let tools = {
            let processes = Arc::new(crate::agent::tools::ProcessRegistry::new());
            let tool_ctx = ToolContext::with_processes_and_channel(
                processes,
                self.inner.homie_config.clone(),
                tool_channel,
            )
            .with_store(self.inner.store.clone());
            build_tools(tool_ctx, &self.inner.homie_config)
                .map_err(|error| format!("failed to build roci tools: {error}"))?
        };
        let agent = Arc::new(AgentRuntime::new(
            Arc::new(roci::default_registry()),
            config,
            AgentConfig {
                model,
                system_prompt,
                tools,
                dynamic_tool_providers: Vec::new(),
                settings: GenerationSettings::default(),
                transform_context: None,
                convert_to_llm: None,
                before_agent_start: None,
                event_sink: None,
                approval_policy: ApprovalPolicy::Ask,
                approval_handler: Some(approval_handler),
                session_id: None,
                steering_mode: roci::agent::QueueDrainMode::All,
                follow_up_mode: roci::agent::QueueDrainMode::All,
                transport: None,
                max_retry_delay_ms: None,
                retry_backoff: Default::default(),
                api_key_override: None,
                provider_headers: Default::default(),
                provider_metadata: HashMap::new(),
                provider_payload_callback: None,
                get_api_key: None,
                compaction: Default::default(),
                session_before_compact: None,
                session_before_tree: None,
                pre_tool_use: None,
                post_tool_use: None,
                user_input_timeout_ms: None,
                context_budget: None,
                chat: ChatRuntimeConfig {
                    default_thread_id: Some(runtime_thread_id),
                    ..Default::default()
                },
                user_input_coordinator: None,
            },
        ));

        if let Some(imported) = self.load_imported_thread(thread_id)? {
            agent
                .import_thread(imported)
                .await
                .map_err(|e| e.to_string())?;
        }

        let entry = Arc::new(RuntimeEntry {
            chat_id: Mutex::new(chat_id.to_string()),
            thread_id: thread_id.to_string(),
            runtime: agent.clone(),
            approvals,
            message_text: Mutex::new(HashMap::new()),
            turn_ids: Mutex::new(HashMap::new()),
        });
        entry.rebuild_turn_index().await;
        self.spawn_event_bridge(entry.clone()).await;
        self.inner
            .entries
            .lock()
            .await
            .insert(thread_id.to_string(), entry.clone());
        Ok(entry)
    }

    fn load_imported_thread(&self, thread_id: &str) -> Result<Option<ImportedThread>, String> {
        let Some(value) = self.inner.store.get_chat_thread_state(thread_id)? else {
            return Ok(None);
        };
        match decode_persisted_runtime_thread(value) {
            Ok(Some(imported)) => Ok(Some(imported)),
            Ok(None) => {
                delete_persisted_runtime_thread(&self.inner.store, thread_id);
                Ok(None)
            }
            Err(error) => {
                delete_persisted_runtime_thread(&self.inner.store, thread_id);
                tracing::warn!(%thread_id, "invalid roci runtime snapshot deleted: {error}");
                Ok(None)
            }
        }
    }

    async fn spawn_event_bridge(&self, entry: Arc<RuntimeEntry>) {
        let mut subscription = entry.runtime.subscribe(None).await;
        let store = self.inner.store.clone();
        let event_tx = self.inner.event_tx.clone();
        tokio::spawn(async move {
            while let Ok(event) = subscription.recv().await {
                entry.handle_runtime_event(event, &store, &event_tx).await;
            }
        });
    }

    pub async fn start_run(&self, request: StartRunRequest<'_>) -> Result<String, String> {
        let entry = self
            .ensure_entry(
                request.chat_id,
                request.thread_id,
                request.model,
                request.config,
                request.system_prompt.clone(),
                request.tool_channel,
            )
            .await?;
        entry
            .runtime
            .set_generation_settings(request.settings.clone())
            .await
            .ok();
        entry
            .runtime
            .set_approval_policy(request.approval_policy)
            .await
            .ok();
        let mut messages = Vec::new();
        if entry.runtime.messages().await.is_empty() {
            if let Some(system_prompt) = request.system_prompt.as_ref() {
                let prompt = system_prompt.trim();
                if !prompt.is_empty() {
                    messages.push(ModelMessage::system(prompt.to_string()));
                }
            }
        }
        messages.push(ModelMessage::user(request.message.to_string()));
        let turn_id = entry
            .runtime
            .enqueue_turn(EnqueueTurnRequest {
                messages,
                generation_settings: Some(request.settings),
                approval_policy: Some(request.approval_policy),
                collaboration_mode: request.collaboration_mode,
            })
            .await
            .map_err(|e| e.to_string())?;
        entry
            .turn_ids
            .lock()
            .await
            .insert(turn_id.to_string(), turn_id);
        Ok(turn_id.to_string())
    }

    pub async fn queue_message(
        &self,
        chat_id: &str,
        thread_id: &str,
        message: &str,
    ) -> Option<String> {
        let entry = self.inner.entries.lock().await.get(thread_id).cloned()?;
        *entry.chat_id.lock().await = chat_id.to_string();
        let turn_id = entry
            .runtime
            .enqueue_turn(EnqueueTurnRequest {
                messages: vec![ModelMessage::user(message.to_string())],
                generation_settings: None,
                approval_policy: None,
                collaboration_mode: None,
            })
            .await
            .ok()?;
        entry
            .turn_ids
            .lock()
            .await
            .insert(turn_id.to_string(), turn_id);
        Some(turn_id.to_string())
    }

    pub async fn cancel_run(&self, turn_id: &str) -> bool {
        let entries = self
            .inner
            .entries
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for entry in entries {
            let Some(id) = entry.turn_ids.lock().await.get(turn_id).copied() else {
                continue;
            };
            return entry.runtime.cancel_turn(id).await.is_ok();
        }
        false
    }

    pub async fn respond_approval(&self, request_id: &str, decision: ApprovalDecision) -> bool {
        let entries = self
            .inner
            .entries
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for entry in entries {
            if let Some(tx) = entry.approvals.lock().await.remove(request_id) {
                return tx.send(decision).is_ok();
            }
        }
        false
    }

    pub async fn thread_read(&self, thread_id: &str) -> Option<Value> {
        let entry = self.inner.entries.lock().await.get(thread_id).cloned()?;
        let snapshot = entry
            .runtime
            .read_thread(entry.runtime.default_thread_id())
            .await
            .ok()?;
        Some(thread_snapshot_to_view(&snapshot))
    }

    pub async fn thread_list(&self) -> Vec<Value> {
        let entries = self
            .inner
            .entries
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut threads = Vec::with_capacity(entries.len());
        for entry in entries {
            if let Ok(snapshot) = entry
                .runtime
                .read_thread(entry.runtime.default_thread_id())
                .await
            {
                threads.push(thread_snapshot_to_view(&snapshot));
            }
        }
        threads
    }

    pub async fn thread_archive(&self, thread_id: &str) {
        self.inner.entries.lock().await.remove(thread_id);
        delete_persisted_runtime_thread(&self.inner.store, thread_id);
    }

    pub async fn shutdown_connection(&self) {}
}

impl RuntimeEntry {
    async fn rebuild_turn_index(&self) {
        if let Ok(thread) = self
            .runtime
            .read_thread(self.runtime.default_thread_id())
            .await
        {
            let mut ids = self.turn_ids.lock().await;
            for turn in &thread.turns {
                ids.insert(turn.turn_id.to_string(), turn.turn_id);
            }
        }
    }

    async fn handle_runtime_event(
        &self,
        event: AgentRuntimeEvent,
        store: &Arc<dyn Store>,
        event_tx: &broadcast::Sender<ReapEvent>,
    ) {
        if let Some(turn_id) = event.turn_id {
            self.turn_ids
                .lock()
                .await
                .insert(turn_id.to_string(), turn_id);
        }
        let chat_id = self.chat_id.lock().await.clone();
        for (topic, params) in runtime_event_to_wire(&event, &chat_id, self).await {
            let _ = event_tx.send(ReapEvent::new(topic, Some(params)));
            advance_event_pointer(store, &chat_id);
        }
        if let Ok(thread) = self.runtime.read_thread(event.thread_id).await {
            let messages = self.runtime.messages().await;
            persist_runtime_thread(store, &self.thread_id, thread, messages);
        }
    }
}

fn build_approval_handler(
    approvals: Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalDecision>>>>,
    exec_policy: Arc<ExecPolicy>,
) -> ApprovalHandler {
    Arc::new(move |request| {
        let approvals = approvals.clone();
        let exec_policy = exec_policy.clone();
        Box::pin(async move {
            if let Some(argv) = approval_command_argv(&request.payload) {
                if exec_policy.is_allowed(&argv) {
                    return ApprovalDecision::Accept;
                }
            }
            let cache_key = approval_cache_key(&request);
            if cache_key.is_none() {
                return ApprovalDecision::Decline;
            }
            let (tx, rx) = oneshot::channel();
            approvals.lock().await.insert(request.id.clone(), tx);
            rx.await.unwrap_or(ApprovalDecision::Decline)
        })
    })
}

async fn runtime_event_to_wire(
    event: &AgentRuntimeEvent,
    chat_id: &str,
    entry: &RuntimeEntry,
) -> Vec<(&'static str, Value)> {
    let thread_id = event.thread_id.to_string();
    let turn_id = event.turn_id.map(|id| id.to_string());
    let mut out = Vec::new();
    match &event.payload {
        AgentRuntimeEventPayload::TurnQueued { turn } => {
            out.extend(dual(
                "chat.turn.started",
                "agent.chat.turn.started",
                turn_payload(chat_id, &thread_id, &turn.turn_id.to_string()),
            ));
        }
        AgentRuntimeEventPayload::TurnStarted { .. } => {}
        AgentRuntimeEventPayload::MessageStarted { message } => {
            if let Some(item) = message_item(message) {
                out.extend(dual(
                    "chat.item.started",
                    "agent.chat.item.started",
                    json!({
                        "threadId": thread_id,
                        "turnId": message.turn_id.to_string(),
                        "item": item,
                    }),
                ));
            }
        }
        AgentRuntimeEventPayload::MessageUpdated { message } => {
            if message.payload.role == Role::Assistant {
                let text = message.payload.text();
                let item_id = message.message_id.to_string();
                let mut cache = entry.message_text.lock().await;
                let previous = cache
                    .insert(item_id.clone(), text.clone())
                    .unwrap_or_default();
                let delta = text.strip_prefix(&previous).unwrap_or(&text).to_string();
                if !delta.is_empty() {
                    out.extend(dual(
                        "chat.message.delta",
                        "agent.chat.delta",
                        json!({
                            "threadId": thread_id,
                            "turnId": message.turn_id.to_string(),
                            "itemId": item_id,
                            "delta": delta,
                        }),
                    ));
                }
            }
        }
        AgentRuntimeEventPayload::MessageCompleted { message } => {
            if message.payload.role == Role::Assistant {
                out.extend(dual(
                    "chat.item.completed",
                    "agent.chat.item.completed",
                    json!({
                        "threadId": thread_id,
                        "turnId": message.turn_id.to_string(),
                        "item": message_item(message).unwrap_or_else(|| json!({})),
                    }),
                ));
            }
        }
        AgentRuntimeEventPayload::ToolStarted { tool } => {
            out.extend(dual(
                "chat.item.started",
                "agent.chat.item.started",
                json!({
                    "threadId": thread_id,
                    "turnId": tool.turn_id.to_string(),
                    "item": tool_item(tool),
                }),
            ));
        }
        AgentRuntimeEventPayload::ToolUpdated { .. } => {}
        AgentRuntimeEventPayload::ToolCompleted { tool } => {
            out.extend(dual(
                "chat.item.completed",
                "agent.chat.item.completed",
                json!({
                    "threadId": thread_id,
                    "turnId": tool.turn_id.to_string(),
                    "item": tool_item(tool),
                }),
            ));
        }
        AgentRuntimeEventPayload::ApprovalRequired { approval } => {
            out.extend(approval_events(&thread_id, approval));
        }
        AgentRuntimeEventPayload::ApprovalResolved { .. }
        | AgentRuntimeEventPayload::ApprovalCanceled { .. } => {}
        AgentRuntimeEventPayload::ReasoningUpdated { reasoning, delta } => {
            out.extend(reasoning_events(&thread_id, reasoning, delta));
        }
        AgentRuntimeEventPayload::PlanUpdated { plan } => {
            out.extend(plan_events(&thread_id, plan));
        }
        AgentRuntimeEventPayload::DiffUpdated { diff } => {
            out.extend(diff_events(&thread_id, diff));
        }
        AgentRuntimeEventPayload::TurnCompleted { turn } => {
            out.extend(turn_completed_events(&thread_id, turn, "completed"));
        }
        AgentRuntimeEventPayload::TurnFailed { turn, error } => {
            out.extend(turn_completed_events(&thread_id, turn, "failed"));
            out.extend(dual(
                "chat.error",
                "agent.chat.error",
                json!({
                    "threadId": thread_id,
                    "turnId": turn.turn_id.to_string(),
                    "message": error,
                }),
            ));
        }
        AgentRuntimeEventPayload::TurnCanceled { turn } => {
            out.extend(turn_completed_events(&thread_id, turn, "canceled"));
        }
    }
    if turn_id.is_none() {
        return out;
    }
    out
}

fn dual(
    chat_topic: &'static str,
    agent_topic: &'static str,
    params: Value,
) -> Vec<(&'static str, Value)> {
    vec![(chat_topic, params.clone()), (agent_topic, params)]
}

fn turn_payload(chat_id: &str, thread_id: &str, turn_id: &str) -> Value {
    json!({ "chatId": chat_id, "threadId": thread_id, "turnId": turn_id })
}

fn message_item(message: &MessageSnapshot) -> Option<Value> {
    match message.payload.role {
        Role::User => Some(json!({
            "id": message.message_id.to_string(),
            "type": "userMessage",
            "content": [{ "type": "text", "text": message.payload.text() }],
        })),
        Role::Assistant => Some(json!({
            "id": message.message_id.to_string(),
            "type": "agentMessage",
            "text": message.payload.text(),
        })),
        Role::System | Role::Tool => None,
    }
}

fn tool_item(tool: &ToolExecutionSnapshot) -> Value {
    let is_error = tool
        .final_result
        .as_ref()
        .map(|result| result.is_error)
        .unwrap_or(false);
    let status = if tool.completed_at.is_some() {
        if is_error {
            "failed"
        } else {
            "completed"
        }
    } else {
        "running"
    };
    json!({
        "id": tool.tool_call_id,
        "type": "mcpToolCall",
        "tool": tool.tool_name,
        "status": status,
        "input": tool.args,
        "result": tool.final_result.as_ref().map(|result| result.result.clone()),
        "error": is_error,
    })
}

fn approval_events(thread_id: &str, approval: &ApprovalSnapshot) -> Vec<(&'static str, Value)> {
    let (command, cwd) = super::events::approval_command_from_payload(&approval.request.payload);
    dual(
        "chat.approval.required",
        "agent.chat.approval.required",
        json!({
            "threadId": thread_id,
            "turnId": approval.turn_id.to_string(),
            "itemId": approval.request.id,
            "request_id": approval.request.id,
            "codex_request_id": approval.request.id,
            "reason": approval.request.reason,
            "command": command,
            "cwd": cwd,
        }),
    )
}

fn reasoning_events(
    thread_id: &str,
    reasoning: &ReasoningSnapshot,
    delta: &str,
) -> Vec<(&'static str, Value)> {
    dual(
        "chat.reasoning.delta",
        "agent.chat.reasoning.delta",
        json!({
            "threadId": thread_id,
            "turnId": reasoning.turn_id.to_string(),
            "itemId": reasoning
                .message_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| reasoning.turn_id.to_string()),
            "delta": delta,
        }),
    )
}

fn plan_events(thread_id: &str, plan: &PlanSnapshot) -> Vec<(&'static str, Value)> {
    let steps = plan
        .plan
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|step| json!({ "step": step.trim_start_matches("- ").trim(), "status": "pending" }))
        .collect::<Vec<_>>();
    dual(
        "chat.plan.updated",
        "agent.chat.plan.updated",
        json!({
            "threadId": thread_id,
            "turnId": plan.turn_id.to_string(),
            "plan": steps,
        }),
    )
}

fn diff_events(thread_id: &str, diff: &DiffSnapshot) -> Vec<(&'static str, Value)> {
    dual(
        "chat.diff.updated",
        "agent.chat.diff.updated",
        json!({
            "threadId": thread_id,
            "turnId": diff.turn_id.to_string(),
            "diff": diff.diff,
        }),
    )
}

fn turn_completed_events(
    thread_id: &str,
    turn: &TurnSnapshot,
    status: &str,
) -> Vec<(&'static str, Value)> {
    dual(
        "chat.turn.completed",
        "agent.chat.turn.completed",
        json!({
            "threadId": thread_id,
            "turnId": turn.turn_id.to_string(),
            "status": status,
        }),
    )
}

fn thread_snapshot_to_view(snapshot: &ThreadSnapshot) -> Value {
    let mut turns = snapshot.turns.clone();
    turns.sort_by_key(|turn| turn.queued_at);
    let turn_views = turns
        .iter()
        .map(|turn| {
            let items = turn_items(snapshot, turn.turn_id);
            json!({ "id": turn.turn_id.to_string(), "items": items })
        })
        .collect::<Vec<_>>();
    json!({
        "id": snapshot.thread_id.to_string(),
        "turns": turn_views,
    })
}

fn turn_items(snapshot: &ThreadSnapshot, turn_id: TurnId) -> Vec<Value> {
    let mut items = Vec::new();
    for message in snapshot.messages.iter().filter(|m| m.turn_id == turn_id) {
        if let Some(item) = message_item(message) {
            items.push((message.created_at, 0, item));
        }
    }
    for tool in snapshot.tools.iter().filter(|tool| tool.turn_id == turn_id) {
        items.push((tool.started_at, 1, tool_item(tool)));
    }
    for approval in snapshot
        .approvals
        .iter()
        .filter(|approval| approval.turn_id == turn_id)
    {
        let mut events = approval_events(&snapshot.thread_id.to_string(), approval);
        if let Some((_, value)) = events.pop() {
            let item = json!({
                "id": approval.request.id,
                "type": "approval",
                "request_id": approval.request.id,
                "codex_request_id": approval.request.id,
                "reason": approval.request.reason,
                "raw": value,
            });
            items.push((approval.requested_at, 2, item));
        }
    }
    for reasoning in snapshot
        .reasoning
        .iter()
        .filter(|reasoning| reasoning.turn_id == turn_id)
    {
        items.push((
            reasoning.updated_at,
            3,
            json!({
                "id": reasoning
                    .message_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| format!("reasoning-{}", reasoning.turn_id)),
                "type": "reasoning",
                "content": [reasoning.text],
                "summary": [],
            }),
        ));
    }
    for plan in snapshot.plans.iter().filter(|plan| plan.turn_id == turn_id) {
        items.push((
            plan.updated_at,
            4,
            json!({
                "id": format!("plan-{}", plan.turn_id),
                "type": "plan",
                "text": plan.plan,
            }),
        ));
    }
    for diff in snapshot.diffs.iter().filter(|diff| diff.turn_id == turn_id) {
        items.push((
            diff.updated_at,
            5,
            json!({
                "id": format!("diff-{}", diff.turn_id),
                "type": "diff",
                "text": diff.diff,
            }),
        ));
    }
    items.sort_by_key(|(timestamp, order, _)| (*timestamp, *order));
    items.into_iter().map(|(_, _, item)| item).collect()
}

fn advance_event_pointer(store: &Arc<dyn Store>, chat_id: &str) {
    if let Ok(Some(chat)) = store.get_chat(chat_id) {
        let next = chat.event_pointer.saturating_add(1);
        if let Err(error) = store.update_event_pointer(chat_id, next) {
            tracing::warn!(%chat_id, pointer = next, error = %error, "failed to update chat pointer");
        }
    }
}

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use roci::agent_loop::{ApprovalDecision, ApprovalPolicy};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::tools::Tool;
use roci::types::{GenerationSettings, ModelMessage, ReasoningEffort, Role};

use crate::agent::tools::{build_tools, ToolContext};
use crate::outbound::OutboundMessage;
use crate::storage::Store;
use crate::ExecPolicy;

mod events;
mod persistence;
mod run;
mod state;

use self::events::{emit_assistant_item, emit_turn_started, emit_user_item};
use self::persistence::{
    backfill_thread_state_from_raw_events, decode_persisted_thread_state, persist_roci_raw_event,
    persist_thread_snapshot, PersistedThreadSnapshot,
};
use self::state::{
    PendingRun, RociItem, RociState, RociThreadState, RociTurn, ToolOutputRetention,
};
#[cfg(test)]
use self::state::{RociRunState, RociThread};

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

pub struct StartRunRequest<'a> {
    pub chat_id: &'a str,
    pub thread_id: &'a str,
    pub message: &'a str,
    pub model: LanguageModel,
    pub settings: GenerationSettings,
    pub approval_policy: ApprovalPolicy,
    pub config: RociConfig,
    pub collaboration_mode: Option<String>,
    pub system_prompt: Option<String>,
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

    pub async fn start_run(&self, request: StartRunRequest<'_>) -> Result<String, String> {
        let StartRunRequest {
            chat_id,
            thread_id,
            message,
            model,
            settings,
            approval_policy,
            config,
            collaboration_mode,
            system_prompt,
        } = request;
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
        run::start_run_inner(self, pending).await
    }

    async fn dequeue_next_run(&self, thread_id: &str) -> Option<PendingRun> {
        run::dequeue_next_run(self, thread_id).await
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
            .unwrap_or_else(default_roci_model);
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

fn default_roci_model() -> String {
    std::env::var("HOMIE_ROCI_MODEL").unwrap_or_else(|_| DEFAULT_ROCI_MODEL.to_string())
}

pub(super) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn debug_enabled() -> bool {
    matches!(std::env::var("HOMIE_DEBUG").as_deref(), Ok("1"))
        || matches!(std::env::var("HOME_DEBUG").as_deref(), Ok("1"))
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
            .start_run(StartRunRequest {
                chat_id: "live-chat",
                thread_id: "live-thread",
                message: "List the current directory using the ls tool.",
                model,
                settings,
                approval_policy: ApprovalPolicy::Always,
                config,
                collaboration_mode: None,
                system_prompt: Some(homie_config.chat.system_prompt.clone()),
            })
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

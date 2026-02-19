use std::collections::{HashMap, HashSet, VecDeque};

use roci::agent_loop::{ApprovalDecision, ApprovalPolicy};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::types::{AgentToolCall, ContentPart, GenerationSettings, ModelMessage, Role};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub(super) struct PendingRun {
    pub(super) chat_id: String,
    pub(super) thread_id: String,
    pub(super) turn_id: String,
    pub(super) assistant_item_id: String,
    pub(super) messages: Vec<ModelMessage>,
    pub(super) model: LanguageModel,
    pub(super) settings: GenerationSettings,
    pub(super) approval_policy: ApprovalPolicy,
    pub(super) config: RociConfig,
    pub(super) collaboration_mode: Option<String>,
}

#[derive(Default)]
pub(super) struct RociState {
    pub(super) threads: HashMap<String, RociThreadState>,
    pub(super) runs: HashMap<String, RociRunState>,
    pub(super) run_queue: HashMap<String, VecDeque<PendingRun>>,
    pub(super) active_threads: HashMap<String, String>,
    pub(super) approvals: HashMap<String, oneshot::Sender<ApprovalDecision>>,
    pub(super) approval_cache: HashMap<String, HashSet<String>>,
    pub(super) tool_output_cache: HashMap<String, VecDeque<ToolOutputRetention>>,
}

pub(super) struct RociRunState {
    pub(super) thread_id: String,
    pub(super) handle: Option<roci::agent_loop::RunHandle>,
}

pub(super) struct ToolCallInfo {
    pub(super) name: String,
    pub(super) input: serde_json::Value,
}

pub(super) struct ToolOutputRetention {
    pub(super) turn_id: String,
    pub(super) process_ids: Vec<String>,
}

#[derive(Clone)]
pub(super) struct RociThreadState {
    pub(super) thread: RociThread,
    pub(super) messages: Vec<ModelMessage>,
    pub(super) last_assistant_item_id: Option<String>,
}

impl RociThreadState {
    pub(super) fn new(thread_id: String) -> Self {
        let now = super::now_unix();
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

    pub(super) fn update_assistant_text(&mut self, item_id: &str, text: &str) {
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
pub(super) struct RociThread {
    pub(super) id: String,
    pub(super) created_at: u64,
    pub(super) updated_at: u64,
    pub(super) turns: Vec<RociTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RociTurn {
    pub(super) id: String,
    pub(super) items: Vec<RociItem>,
}

impl RociTurn {
    pub(super) fn new(id: String, items: Vec<RociItem>) -> Self {
        Self { id, items }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(super) enum RociItem {
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
    pub(super) fn user(id: String, text: String) -> Self {
        Self::UserMessage {
            id,
            content: vec![RociContent::Text { text }],
        }
    }

    pub(super) fn assistant(id: String, text: String) -> Self {
        Self::AgentMessage { id, text }
    }

    pub(super) fn tool_call(
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
pub(super) enum RociContent {
    #[serde(rename = "text")]
    Text { text: String },
}

pub(super) fn model_messages_from_turns(turns: &[RociTurn]) -> Vec<ModelMessage> {
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

pub(super) fn model_tool_call_message(call: &AgentToolCall) -> ModelMessage {
    ModelMessage {
        role: Role::Assistant,
        content: vec![ContentPart::ToolCall(call.clone())],
        name: None,
        timestamp: None,
    }
}

pub(super) fn last_assistant_item_id_from_turns(turns: &[RociTurn]) -> Option<String> {
    turns.iter().rev().find_map(|turn| {
        turn.items.iter().rev().find_map(|item| match item {
            RociItem::AgentMessage { id, .. } => Some(id.clone()),
            _ => None,
        })
    })
}

pub(super) fn upsert_user_item(turn: &mut RociTurn, item_id: &str, text: String) {
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

pub(super) fn upsert_assistant_item(
    turn: &mut RociTurn,
    item_id: &str,
    text: String,
    append: bool,
) {
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

pub(super) fn upsert_tool_item_started(
    turn: &mut RociTurn,
    item_id: &str,
    tool: &str,
    input: Value,
) {
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

pub(super) fn upsert_tool_item_completed(
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

pub(super) fn upsert_tool_item(
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

use crate::agent::process::CodexRequestId;
use crate::agent::service::dispatch::{AgentService, ChatService};
use crate::agent::service::events::codex_method_to_topics;
use crate::agent::service::models::{chrono_now, roci_model_catalog};
use crate::agent::service::params::{
    normalize_model_selector, parse_approval_params, parse_cancel_params, parse_message_params,
    parse_tool_channel, MessageParams,
};
use crate::agent::tools::TOOL_CHANNEL_DENIED_CODE;
use crate::execpolicy::ExecPolicy;
use crate::homie_config::{HomieConfig, ProvidersConfig};
use crate::outbound::OutboundMessage;
use crate::storage::{ChatRecord, SessionStatus, SqliteStore, Store};
use crate::ServiceHandler;
use homie_protocol::error_codes;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

fn make_store() -> Arc<dyn Store> {
    Arc::new(SqliteStore::open_memory().unwrap())
}

mod approvals;
mod chat;
mod dispatch;
mod events;
mod params;

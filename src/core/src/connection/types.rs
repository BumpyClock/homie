use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::roci_backend::RociBackend;
use crate::config::ServerConfig;
use crate::cron::CronRunner;
use crate::router::ServiceRegistry;
use crate::storage::Store;
use crate::terminal::TerminalRegistry;
use crate::{ExecPolicy, HomieConfig};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Represents an authenticated WS connection after handshake.
#[derive(Debug)]
pub struct Connection {
    pub id: Uuid,
    pub identity: Option<String>,
    pub negotiated_version: u16,
}

/// Parameters required to run a connection session.
#[derive(Clone)]
pub struct ConnectionParams {
    pub config: ServerConfig,
    pub heartbeat_interval: Duration,
    pub idle_timeout: Duration,
    pub registry: ServiceRegistry,
    pub store: Arc<dyn Store>,
    pub nodes: Arc<Mutex<crate::presence::NodeRegistry>>,
    pub terminal_registry: Arc<Mutex<TerminalRegistry>>,
    pub event_tx: broadcast::Sender<crate::router::ReapEvent>,
    pub cron_runner: Arc<CronRunner>,
    pub homie_config: Arc<HomieConfig>,
    pub exec_policy: Arc<ExecPolicy>,
    pub roci: RociBackend,
    pub pairing_default_ttl_secs: u64,
    pub pairing_retention_secs: u64,
}

// Parameters required for the message loop lifecycle.
pub(crate) struct MessageLoopParams {
    pub conn_id: Uuid,
    pub heartbeat_interval: Duration,
    pub idle_timeout: Duration,
    pub authz: crate::authz::AuthContext,
    pub store: Arc<dyn Store>,
    pub nodes: Arc<Mutex<crate::presence::NodeRegistry>>,
    pub terminal_registry: Arc<Mutex<TerminalRegistry>>,
    pub event_tx: broadcast::Sender<crate::router::ReapEvent>,
    pub cron_runner: Arc<CronRunner>,
    pub homie_config: Arc<HomieConfig>,
    pub exec_policy: Arc<ExecPolicy>,
    pub roci: RociBackend,
    pub pairing_default_ttl_secs: u64,
    pub pairing_retention_secs: u64,
    pub tool_channel: Option<String>,
}

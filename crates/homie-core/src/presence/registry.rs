use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use homie_protocol::ServiceCapability;
use serde::{Deserialize, Serialize};

/// Stored node metadata for presence tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub services: Vec<ServiceCapability>,
}

#[derive(Debug, Clone)]
struct NodeEntry {
    info: NodeInfo,
    last_seen_at: Instant,
    last_seen_unix: u64,
    offline: bool,
}

/// Snapshot returned to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSnapshot {
    pub node_id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub services: Vec<ServiceCapability>,
    pub status: String,
    pub last_seen: u64,
}

/// In-memory node registry with heartbeat tracking.
pub struct NodeRegistry {
    nodes: HashMap<String, NodeEntry>,
    timeout: Duration,
}

impl NodeRegistry {
    pub fn new(timeout: Duration) -> Self {
        Self {
            nodes: HashMap::new(),
            timeout,
        }
    }

    pub fn register(&mut self, info: NodeInfo) {
        let now = Instant::now();
        let unix = now_unix();
        self.nodes.insert(
            info.node_id.clone(),
            NodeEntry {
                info,
                last_seen_at: now,
                last_seen_unix: unix,
                offline: false,
            },
        );
    }

    pub fn heartbeat(&mut self, node_id: &str) -> Result<(), String> {
        let now = Instant::now();
        let unix = now_unix();
        match self.nodes.get_mut(node_id) {
            Some(entry) => {
                entry.last_seen_at = now;
                entry.last_seen_unix = unix;
                entry.offline = false;
                Ok(())
            }
            None => Err(format!("unknown node: {node_id}")),
        }
    }

    pub fn unregister(&mut self, node_id: &str) -> bool {
        match self.nodes.get_mut(node_id) {
            Some(entry) => {
                entry.offline = true;
                true
            }
            None => false,
        }
    }

    pub fn list(&self) -> Vec<NodeSnapshot> {
        let now = Instant::now();
        self.nodes
            .values()
            .map(|entry| {
                let overdue = now.duration_since(entry.last_seen_at) > self.timeout;
                let status = if entry.offline || overdue {
                    "offline"
                } else {
                    "online"
                };
                NodeSnapshot {
                    node_id: entry.info.node_id.clone(),
                    name: entry.info.name.clone(),
                    version: entry.info.version.clone(),
                    services: entry.info.services.clone(),
                    status: status.to_string(),
                    last_seen: entry.last_seen_unix,
                }
            })
            .collect()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

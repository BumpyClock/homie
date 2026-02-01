use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use crate::authz::Role;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Primary bind address (default: 127.0.0.1:9800).
    pub bind: SocketAddr,
    /// Optional second bind for tailnet IP.
    pub tailnet_bind: Option<SocketAddr>,
    /// Whether Tailscale Serve headers should be validated.
    pub tailscale_serve: bool,
    /// Allow non-loopback LAN connections without Tailscale Serve.
    pub allow_lan: bool,
    /// Interval between server→client pings.
    pub heartbeat_interval: Duration,
    /// Close the connection after this duration without any message.
    pub idle_timeout: Duration,
    /// Role assigned to loopback clients.
    pub local_role: Role,
    /// Role assigned to authenticated Tailscale clients.
    pub tailscale_role: Role,
    /// Consider nodes offline after this interval without heartbeat.
    pub node_timeout: Duration,
    /// Job retention window in days.
    pub job_retention_days: u64,
    /// Maximum number of jobs to retain.
    pub job_max_records: usize,
    /// Retention window for expired pairings, in seconds.
    pub pairing_retention_secs: u64,
    /// Default TTL for pairing requests, in seconds.
    pub pairing_default_ttl_secs: u64,
    /// Retention window for notification records, in days.
    pub notification_retention_days: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9800),
            tailnet_bind: None,
            tailscale_serve: false,
            allow_lan: false,
            heartbeat_interval: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(120),
            local_role: Role::Owner,
            tailscale_role: Role::User,
            node_timeout: Duration::from_secs(60),
            job_retention_days: 7,
            job_max_records: 500,
            pairing_retention_secs: 86_400,
            pairing_default_ttl_secs: 300,
            notification_retention_days: 30,
        }
    }
}

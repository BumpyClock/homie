use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Primary bind address (default: 127.0.0.1:9800).
    pub bind: SocketAddr,
    /// Optional second bind for tailnet IP.
    pub tailnet_bind: Option<SocketAddr>,
    /// Whether Tailscale Serve headers should be validated.
    pub tailscale_serve: bool,
    /// Interval between server→client pings.
    pub heartbeat_interval: Duration,
    /// Close the connection after this duration without any message.
    pub idle_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9800),
            tailnet_bind: None,
            tailscale_serve: false,
            heartbeat_interval: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(120),
        }
    }
}

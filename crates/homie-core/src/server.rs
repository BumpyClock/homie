use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::{HeaderMap, Request};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::auth::{authenticate, AuthOutcome, TailscaleWhois};
use crate::config::ServerConfig;
use crate::connection::{run_connection, ConnectionParams};
use crate::presence::NodeRegistry;
use crate::router::ServiceRegistry;
use crate::storage::Store;

/// Shared state accessible by handlers.
#[derive(Clone)]
pub(crate) struct AppState {
    pub config: ServerConfig,
    pub whois: Arc<dyn TailscaleWhois>,
    pub registry: ServiceRegistry,
    pub store: Arc<dyn Store>,
    pub nodes: Arc<Mutex<NodeRegistry>>,
}

/// Build the axum router for the WS server.
///
/// The router exposes `/ws` (WebSocket upgrade) and `/health`.
/// Callers should use `into_make_service_with_connect_info::<SocketAddr>()`
/// when binding to get remote address extraction.
///
/// On startup, marks all previously-active sessions as inactive so clients
/// see them as stale until reattached.
pub fn build_router(
    config: ServerConfig,
    whois: impl TailscaleWhois,
    store: Arc<dyn Store>,
) -> Router {
    // Mark previous sessions inactive on restart.
    if let Err(e) = store.mark_all_inactive() {
        tracing::warn!("failed to mark sessions inactive on startup: {e}");
    }
    if let Err(e) = store.prune_jobs(config.job_retention_days, config.job_max_records) {
        tracing::warn!("failed to prune jobs on startup: {e}");
    }
    if let Err(e) = store.prune_pairings(config.pairing_retention_secs) {
        tracing::warn!("failed to prune pairings on startup: {e}");
    }
    if let Err(e) = store.prune_notifications(config.notification_retention_days) {
        tracing::warn!("failed to prune notifications on startup: {e}");
    }

    let mut registry = ServiceRegistry::new();
    registry.register("terminal", "1.0");
    registry.register("agent", "1.0");
    registry.register("presence", "1.0");
    registry.register("jobs", "0.1");
    registry.register("pairing", "0.1");
    registry.register("notifications", "0.1");

    let nodes = Arc::new(Mutex::new(NodeRegistry::new(config.node_timeout)));

    let state = AppState {
        config,
        whois: Arc::new(whois),
        registry,
        store,
        nodes,
    };

    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/health", get(health))
        .layer(middleware::from_fn(extract_remote_ip))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Extension type for the remote IP, injected by middleware.
#[derive(Debug, Clone, Copy)]
struct RemoteIp(IpAddr);

async fn extract_remote_ip(
    req: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let remote_ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    let mut req = req;
    req.extensions_mut().insert(RemoteIp(remote_ip));
    next.run(req).await
}

async fn health() -> &'static str {
    "ok"
}

async fn ws_upgrade(
    State(state): State<AppState>,
    axum::Extension(RemoteIp(remote_ip)): axum::Extension<RemoteIp>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let auth = authenticate(
        &headers,
        remote_ip,
        state.config.tailscale_serve,
        &state.whois,
    )
    .await;

    if let AuthOutcome::Rejected(reason) = &auth {
        tracing::warn!(%remote_ip, %reason, "ws upgrade rejected");
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    let heartbeat = state.config.heartbeat_interval;
    let idle = state.config.idle_timeout;
    let registry = state.registry.clone();
    let store = state.store.clone();
    let config = state.config.clone();
    let nodes = state.nodes.clone();
    let params = ConnectionParams {
        config,
        heartbeat_interval: heartbeat,
        idle_timeout: idle,
        registry,
        store,
        nodes,
        pairing_default_ttl_secs: state.config.pairing_default_ttl_secs,
        pairing_retention_secs: state.config.pairing_retention_secs,
    };

    ws.on_upgrade(move |socket| run_connection(socket, auth, params))
        .into_response()
}

use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::{HeaderMap, Request};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::auth::{authenticate, AuthOutcome, TailscaleWhois};
use crate::config::ServerConfig;
use crate::connection::run_connection;
use crate::router::ServiceRegistry;
use crate::storage::Store;

/// Shared state accessible by handlers.
#[derive(Clone)]
pub(crate) struct AppState {
    pub config: ServerConfig,
    pub whois: Arc<dyn TailscaleWhois>,
    pub registry: ServiceRegistry,
    pub store: Arc<dyn Store>,
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

    let mut registry = ServiceRegistry::new();
    registry.register("terminal", "1.0");
    registry.register("agent", "1.0");

    let state = AppState {
        config,
        whois: Arc::new(whois),
        registry,
        store,
    };

    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/health", get(health))
        .layer(middleware::from_fn(extract_remote_ip))
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

    ws.on_upgrade(move |socket| run_connection(socket, auth, heartbeat, idle, registry, store))
        .into_response()
}

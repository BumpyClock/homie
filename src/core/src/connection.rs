use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use homie_protocol::{
    decode_message, encode_message, error_codes, ClientHello, HandshakeResponse, HelloReject,
    HelloRejectCode, Message as ProtoMessage, Response, ServerHello, VersionRange,
    PROTOCOL_VERSION,
};

use crate::agent::ChatService;
use crate::auth::AuthOutcome;
use crate::authz::{context_for_outcome, scope_for_method, AuthContext, Scope};
use crate::config::ServerConfig;
use crate::debug_bytes::{fmt_bytes, terminal_debug_enabled_for};
use crate::jobs::JobsService;
use crate::notifications::NotificationsService;
use crate::outbound::OutboundMessage;
use crate::pairing::PairingService;
use crate::presence::{NodeRegistry, PresenceService};
use crate::router::{MessageRouter, ServiceRegistry, SubscriptionManager};
use crate::storage::Store;
use crate::terminal::{TerminalRegistry, TerminalService};
use crate::{ExecPolicy, HomieConfig};

/// Represents an authenticated WS connection after handshake.
#[derive(Debug)]
pub struct Connection {
    pub id: Uuid,
    pub identity: Option<String>,
    pub negotiated_version: u16,
}

/// Parameters required to run a connection session.
/// Parameters required to run a connection session.
#[derive(Clone)]
pub struct ConnectionParams {
    pub config: ServerConfig,
    pub heartbeat_interval: Duration,
    pub idle_timeout: Duration,
    pub registry: ServiceRegistry,
    pub store: Arc<dyn Store>,
    pub nodes: Arc<Mutex<NodeRegistry>>,
    pub terminal_registry: Arc<Mutex<TerminalRegistry>>,
    pub event_tx: broadcast::Sender<crate::router::ReapEvent>,
    pub homie_config: Arc<HomieConfig>,
    pub exec_policy: Arc<ExecPolicy>,
    pub pairing_default_ttl_secs: u64,
    pub pairing_retention_secs: u64,
}

/// Parameters required for the message loop lifecycle.
#[derive(Clone)]
struct MessageLoopParams {
    conn_id: Uuid,
    heartbeat_interval: Duration,
    idle_timeout: Duration,
    authz: AuthContext,
    store: Arc<dyn Store>,
    nodes: Arc<Mutex<NodeRegistry>>,
    terminal_registry: Arc<Mutex<TerminalRegistry>>,
    event_tx: broadcast::Sender<crate::router::ReapEvent>,
    homie_config: Arc<HomieConfig>,
    exec_policy: Arc<ExecPolicy>,
    pairing_default_ttl_secs: u64,
    pairing_retention_secs: u64,
}

/// Run the full connection lifecycle: handshake → message loop with
/// heartbeat + idle timeout.
pub async fn run_connection(socket: WebSocket, auth: AuthOutcome, params: ConnectionParams) {
    let ConnectionParams {
        config,
        heartbeat_interval,
        idle_timeout,
        registry,
        store,
        nodes,
        terminal_registry,
        event_tx,
        homie_config,
        exec_policy,
        pairing_default_ttl_secs,
        pairing_retention_secs,
    } = params;
    let conn_id = Uuid::new_v4();
    let span = tracing::info_span!("conn", id = %conn_id);
    let _enter = span.enter();

    let (mut sink, mut stream) = socket.split();

    // ── Phase 1: Handshake ───────────────────────────────────────────
    let hello = match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<ClientHello>(&text) {
            Ok(h) => h,
            Err(e) => {
                send_reject(
                    &mut sink,
                    HelloRejectCode::ServerError,
                    &format!("invalid handshake: {e}"),
                )
                .await;
                return;
            }
        },
        _ => {
            send_reject(
                &mut sink,
                HelloRejectCode::ServerError,
                "expected text handshake frame",
            )
            .await;
            return;
        }
    };

    let server_range = VersionRange::new(PROTOCOL_VERSION, PROTOCOL_VERSION);
    let negotiated = match server_range.negotiate(&hello.protocol) {
        Some(v) => v,
        None => {
            send_reject(
                &mut sink,
                HelloRejectCode::VersionMismatch,
                &format!(
                    "no common version: server={}-{} client={}-{}",
                    server_range.min, server_range.max, hello.protocol.min, hello.protocol.max,
                ),
            )
            .await;
            return;
        }
    };

    let identity = auth.identity_string();
    let authz = context_for_outcome(&auth, &config);

    let server_hello = HandshakeResponse::Hello(ServerHello {
        protocol_version: negotiated,
        server_id: format!("homie-gateway/{}", env!("CARGO_PKG_VERSION")),
        identity: identity.clone(),
        services: registry.capabilities(),
    });

    let json = match serde_json::to_string(&server_hello) {
        Ok(j) => j,
        Err(_) => return,
    };
    if sink.send(Message::Text(json.into())).await.is_err() {
        return;
    }

    let conn = Connection {
        id: conn_id,
        identity,
        negotiated_version: negotiated,
    };

    tracing::info!(
        conn_id = %conn.id,
        identity = ?conn.identity,
        version = conn.negotiated_version,
        "handshake complete"
    );

    // ── Phase 2: Message loop with heartbeat + idle timeout ──────────
    drop(_enter);
    let loop_params = MessageLoopParams {
        conn_id,
        heartbeat_interval,
        idle_timeout,
        authz,
        store,
        nodes,
        terminal_registry,
        event_tx,
        homie_config,
        exec_policy,
        pairing_default_ttl_secs,
        pairing_retention_secs,
    };

    run_message_loop(&mut sink, &mut stream, loop_params).await;

    tracing::info!(conn_id = %conn.id, "connection closed");
}

async fn run_message_loop(
    sink: &mut SplitSink<WebSocket, Message>,
    stream: &mut futures::stream::SplitStream<WebSocket>,
    params: MessageLoopParams,
) {
    let MessageLoopParams {
        conn_id,
        heartbeat_interval,
        idle_timeout,
        authz,
        store,
        nodes,
        terminal_registry,
        event_tx,
        homie_config,
        exec_policy,
        pairing_default_ttl_secs,
        pairing_retention_secs,
    } = params;
    let mut idle_deadline = tokio::time::Instant::now() + idle_timeout;
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.tick().await; // consume immediate first tick

    // Outbound channel: services push PTY output + events here.
    // Bounded for backpressure — services use try_send to avoid blocking.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(256);

    // Build the router with services.
    let mut router = MessageRouter::new();
    router.register(Box::new(TerminalService::new(
        conn_id,
        terminal_registry,
        outbound_tx.clone(),
        event_tx.clone(),
    )));
    let (chat_service, agent_service) = ChatService::new_shared(
        outbound_tx.clone(),
        store.clone(),
        homie_config,
        exec_policy,
    );
    router.register(Box::new(chat_service));
    router.register(Box::new(agent_service));
    router.register(Box::new(PresenceService::new(nodes)));
    router.register(Box::new(JobsService::new(store.clone())));
    router.register(Box::new(PairingService::new(
        store.clone(),
        pairing_default_ttl_secs,
        pairing_retention_secs,
    )));
    router.register(Box::new(NotificationsService::new(
        store.clone(),
        outbound_tx.clone(),
    )));

    // Per-connection subscription manager.
    let mut subscriptions = SubscriptionManager::new();

    let mut event_rx = event_tx.subscribe();

    loop {
        tokio::select! {
            // Incoming WS message.
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        handle_text_message(
                            sink,
                            &text,
                            authz,
                            &mut router,
                            &mut subscriptions,
                        ).await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        if authz.allows(Scope::TerminalWrite) {
                            let decoded = homie_protocol::BinaryFrame::decode(&data);
                            match decoded {
                                Ok(frame) => {
                                    if terminal_debug_enabled_for(frame.session_id) {
                                        tracing::info!(
                                            session = %frame.session_id,
                                            stream = ?frame.stream,
                                            msg = %fmt_bytes(&frame.payload, 80),
                                            "terminal ws in binary"
                                        );
                                    }
                                    router.route_binary(&frame);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        err = %e,
                                        msg = %fmt_bytes(&data, 64),
                                        "invalid binary frame"
                                    );
                                }
                            }
                        } else {
                            tracing::debug!("unauthorized binary frame ignored");
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                    }
                    Some(Ok(Message::Close(frame))) => {
                        if let Some(frame) = frame {
                            tracing::info!(code = %frame.code, reason = %frame.reason, "ws close");
                        } else {
                            tracing::info!("ws close");
                        }
                        break;
                    }
                    None => {
                        tracing::info!("ws stream ended");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("ws error: {e}");
                        break;
                    }
                }
            }
            // Outbound messages from services (PTY output frames).
            msg = outbound_rx.recv() => {
                match msg {
                    Some(OutboundMessage::Raw(m)) => {
                        if let Message::Binary(data) = &m {
                            match homie_protocol::BinaryFrame::decode(data) {
                                Ok(frame) => {
                                    if terminal_debug_enabled_for(frame.session_id) {
                                        tracing::info!(
                                            session = %frame.session_id,
                                            stream = ?frame.stream,
                                            msg = %fmt_bytes(&frame.payload, 80),
                                            "terminal ws out binary"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        err = %e,
                                        msg = %fmt_bytes(data, 64),
                                        "invalid outbound binary frame"
                                    );
                                }
                            }
                        }
                        let _ = sink.send(m).await;
                    }
                    Some(OutboundMessage::Event { topic, params }) => {
                        if subscriptions.matches(&topic) {
                            let evt = ProtoMessage::Event(homie_protocol::Event {
                                topic,
                                params,
                            });
                            if let Ok(json) = encode_message(&evt) {
                                let _ = sink.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    None => break,
                }
            }
            // Heartbeat ping.
            _ = heartbeat.tick() => {
                if sink.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            // Idle timeout.
            _ = tokio::time::sleep_until(idle_deadline) => {
                tracing::info!("idle timeout");
                let _ = sink
                    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 4000,
                        reason: "idle timeout".into(),
                    })))
                    .await;
                break;
            }
            // Broadcast events (filtered by subscriptions).
            evt = event_rx.recv() => {
                match evt {
                    Ok(reap_event) => {
                        tracing::info!(topic = %reap_event.topic, "broadcast event");
                        if subscriptions.matches(&reap_event.topic) {
                            let evt = ProtoMessage::Event(homie_protocol::Event {
                                topic: reap_event.topic,
                                params: reap_event.params,
                            });
                            if let Ok(json) = encode_message(&evt) {
                                let _ = sink.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        tracing::warn!("event receiver lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    // Connection closing — clean up all services.
    router.shutdown_all();
}

async fn handle_text_message(
    sink: &mut SplitSink<WebSocket, Message>,
    text: &str,
    authz: AuthContext,
    router: &mut MessageRouter,
    subscriptions: &mut SubscriptionManager,
) {
    match decode_message(text) {
        Ok(ProtoMessage::Request(req)) => {
            tracing::debug!(method = %req.method, id = %req.id, "request");
            route_and_respond(
                sink,
                authz,
                router,
                subscriptions,
                req.id,
                req.method,
                req.params,
                None,
            )
            .await;
        }
        Ok(other) => {
            tracing::debug!(?other, "non-request message from client (ignored)");
        }
        Err(e) => {
            if let Some(legacy) = decode_legacy_request(text) {
                tracing::debug!(
                    method = %legacy.method,
                    client_id = %legacy.response_id,
                    internal_id = %legacy.req_id,
                    "legacy request id accepted"
                );
                route_and_respond(
                    sink,
                    authz,
                    router,
                    subscriptions,
                    legacy.req_id,
                    legacy.method,
                    legacy.params,
                    Some(legacy.response_id),
                )
                .await;
            } else {
                tracing::warn!("failed to decode message: {e}");
            }
        }
    }
}

async fn route_and_respond(
    sink: &mut SplitSink<WebSocket, Message>,
    authz: AuthContext,
    router: &mut MessageRouter,
    subscriptions: &mut SubscriptionManager,
    req_id: Uuid,
    method: String,
    params: Option<Value>,
    response_id_override: Option<Value>,
) {
    if let Some(scope) = scope_for_method(&method) {
        if !authz.allows(scope) {
            let resp = Response::error(req_id, error_codes::UNAUTHORIZED, "unauthorized");
            send_response(sink, resp, response_id_override).await;
            return;
        }
    }

    // Handle built-in subscription methods.
    let resp = match method.as_str() {
        "events.subscribe" => handle_subscribe(req_id, params, subscriptions),
        "events.unsubscribe" => handle_unsubscribe(req_id, params, subscriptions),
        "agent.chat.event.subscribe" | "agent.codex.event.subscribe" => {
            let params = agent_subscribe_params(params);
            handle_subscribe(req_id, params, subscriptions)
        }
        "chat.event.subscribe" => {
            let params = chat_subscribe_params(params);
            handle_subscribe(req_id, params, subscriptions)
        }
        _ => router.route_request(req_id, &method, params).await,
    };

    send_response(sink, resp, response_id_override).await;
}

async fn send_response(
    sink: &mut SplitSink<WebSocket, Message>,
    resp: Response,
    response_id_override: Option<Value>,
) {
    match response_id_override {
        None => {
            let msg = ProtoMessage::Response(resp);
            if let Ok(json) = encode_message(&msg) {
                let _ = sink.send(Message::Text(json.into())).await;
            }
        }
        Some(override_id) => {
            let mut payload = match serde_json::to_value(&resp) {
                Ok(v) => v,
                Err(_) => return,
            };
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("type".into(), json!("response"));
                obj.insert("id".into(), override_id);
                if obj.get("result").is_some_and(Value::is_null) {
                    obj.remove("result");
                }
                if obj.get("error").is_some_and(Value::is_null) {
                    obj.remove("error");
                }
            }
            if let Ok(text) = serde_json::to_string(&payload) {
                let _ = sink.send(Message::Text(text.into())).await;
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct LooseRequest {
    #[serde(rename = "type")]
    message_type: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug)]
struct LegacyRequest {
    req_id: Uuid,
    response_id: Value,
    method: String,
    params: Option<Value>,
}

fn decode_legacy_request(text: &str) -> Option<LegacyRequest> {
    let req = serde_json::from_str::<LooseRequest>(text).ok()?;
    if req.message_type != "request" {
        return None;
    }

    let response_id = match &req.id {
        Value::String(_) | Value::Number(_) => req.id.clone(),
        _ => return None,
    };

    let req_id = match &req.id {
        Value::String(value) => Uuid::parse_str(value).unwrap_or_else(|_| Uuid::new_v4()),
        Value::Number(_) => Uuid::new_v4(),
        _ => return None,
    };

    Some(LegacyRequest {
        req_id,
        response_id,
        method: req.method,
        params: req.params,
    })
}

/// Handle `events.subscribe` — add a topic subscription.
///
/// Params: `{ "topic": "terminal.*" }` or `{ "topic": "*" }`
/// Returns: `{ "subscription_id": "<uuid>" }`
fn handle_subscribe(
    req_id: Uuid,
    params: Option<serde_json::Value>,
    subs: &mut SubscriptionManager,
) -> Response {
    let topic = params
        .as_ref()
        .and_then(|p| p.get("topic"))
        .and_then(|v| v.as_str());

    match topic {
        Some(pattern) => {
            let sub_id = subs.subscribe(pattern);
            tracing::debug!(%sub_id, pattern, "subscribed");
            Response::success(req_id, json!({ "subscription_id": sub_id }))
        }
        None => Response::error(
            req_id,
            error_codes::INVALID_PARAMS,
            "missing 'topic' parameter",
        ),
    }
}

/// Handle `events.unsubscribe` — remove a topic subscription.
///
/// Params: `{ "subscription_id": "<uuid>" }`
/// Returns: `{ "ok": true }` or error if not found.
fn handle_unsubscribe(
    req_id: Uuid,
    params: Option<serde_json::Value>,
    subs: &mut SubscriptionManager,
) -> Response {
    let sub_id = params
        .as_ref()
        .and_then(|p| p.get("subscription_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok());

    match sub_id {
        Some(id) => {
            if subs.unsubscribe(id) {
                tracing::debug!(%id, "unsubscribed");
                Response::success(req_id, json!({ "ok": true }))
            } else {
                Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "subscription not found",
                )
            }
        }
        None => Response::error(
            req_id,
            error_codes::INVALID_PARAMS,
            "missing or invalid 'subscription_id'",
        ),
    }
}

fn agent_subscribe_params(params: Option<serde_json::Value>) -> Option<serde_json::Value> {
    let mut params = params.unwrap_or_else(|| json!({}));
    match params.as_object_mut() {
        Some(map) => {
            map.entry("topic".to_string())
                .or_insert_with(|| json!("agent.chat.*"));
            Some(params)
        }
        None => Some(json!({ "topic": "agent.chat.*" })),
    }
}

fn chat_subscribe_params(params: Option<serde_json::Value>) -> Option<serde_json::Value> {
    let mut params = params.unwrap_or_else(|| json!({}));
    match params.as_object_mut() {
        Some(map) => {
            map.entry("topic".to_string())
                .or_insert_with(|| json!("chat.*"));
            Some(params)
        }
        None => Some(json!({ "topic": "chat.*" })),
    }
}

async fn send_reject(
    sink: &mut SplitSink<WebSocket, Message>,
    code: HelloRejectCode,
    reason: &str,
) {
    let reject = HandshakeResponse::Reject(HelloReject {
        code,
        reason: reason.into(),
    });
    if let Ok(json) = serde_json::to_string(&reject) {
        let _ = sink.send(Message::Text(json.into())).await;
    }
    let _ = sink
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code: 4001,
            reason: reason.into(),
        })))
        .await;
}

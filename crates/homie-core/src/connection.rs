use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{
    decode_message, encode_message, error_codes, ClientHello, HandshakeResponse, HelloReject,
    HelloRejectCode, Message as ProtoMessage, Response, ServerHello, VersionRange,
    PROTOCOL_VERSION,
};

use crate::auth::AuthOutcome;
use crate::router::{MessageRouter, ServiceRegistry, SubscriptionManager};
use crate::terminal::TerminalService;

/// Represents an authenticated WS connection after handshake.
#[derive(Debug)]
pub struct Connection {
    pub id: Uuid,
    pub identity: Option<String>,
    pub negotiated_version: u16,
}

/// Run the full connection lifecycle: handshake → message loop with
/// heartbeat + idle timeout.
pub async fn run_connection(
    socket: WebSocket,
    auth: AuthOutcome,
    heartbeat_interval: Duration,
    idle_timeout: Duration,
    registry: ServiceRegistry,
) {
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
    run_message_loop(&mut sink, &mut stream, heartbeat_interval, idle_timeout).await;

    tracing::info!(conn_id = %conn.id, "connection closed");
}

async fn run_message_loop(
    sink: &mut SplitSink<WebSocket, Message>,
    stream: &mut futures::stream::SplitStream<WebSocket>,
    heartbeat_interval: Duration,
    idle_timeout: Duration,
) {
    let mut idle_deadline = tokio::time::Instant::now() + idle_timeout;
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.tick().await; // consume immediate first tick

    // Outbound channel: services push PTY output + events here.
    // Bounded for backpressure — services use try_send to avoid blocking.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(256);

    // Build the router with the terminal service.
    let mut router = MessageRouter::new();
    router.register(Box::new(TerminalService::new(outbound_tx)));

    // Per-connection subscription manager.
    let mut subscriptions = SubscriptionManager::new();

    // Reap interval: check for exited sessions periodically.
    let mut reap_interval = tokio::time::interval(Duration::from_secs(2));
    reap_interval.tick().await;

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
                            &mut router,
                            &mut subscriptions,
                        ).await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        router.route_binary(
                            &homie_protocol::BinaryFrame::decode(&data)
                                .unwrap_or_else(|e| {
                                    tracing::warn!("invalid binary frame: {e}");
                                    // Return a dummy frame that will be ignored.
                                    homie_protocol::BinaryFrame {
                                        session_id: Uuid::nil(),
                                        stream: homie_protocol::StreamType::Stdout,
                                        payload: vec![],
                                    }
                                }),
                        );
                    }
                    Some(Ok(Message::Ping(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        tracing::warn!("ws error: {e}");
                        break;
                    }
                }
            }
            // Outbound messages from services (PTY output frames).
            msg = outbound_rx.recv() => {
                match msg {
                    Some(m) => { let _ = sink.send(m).await; }
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
            // Reap exited sessions → emit events (filtered by subscriptions).
            _ = reap_interval.tick() => {
                let events = router.reap_all();
                for reap_event in events {
                    tracing::info!(topic = %reap_event.topic, "reap event");
                    // Only send if client has a matching subscription.
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
            }
        }
    }

    // Connection closing — clean up all services.
    router.shutdown_all();
}

async fn handle_text_message(
    sink: &mut SplitSink<WebSocket, Message>,
    text: &str,
    router: &mut MessageRouter,
    subscriptions: &mut SubscriptionManager,
) {
    match decode_message(text) {
        Ok(ProtoMessage::Request(req)) => {
            tracing::debug!(method = %req.method, id = %req.id, "request");

            // Handle built-in subscription methods.
            let resp = match req.method.as_str() {
                "events.subscribe" => handle_subscribe(req.id, req.params, subscriptions),
                "events.unsubscribe" => handle_unsubscribe(req.id, req.params, subscriptions),
                _ => router.route_request(req.id, &req.method, req.params).await,
            };

            let msg = ProtoMessage::Response(resp);
            if let Ok(json) = encode_message(&msg) {
                let _ = sink.send(Message::Text(json.into())).await;
            }
        }
        Ok(other) => {
            tracing::debug!(?other, "non-request message from client (ignored)");
        }
        Err(e) => {
            tracing::warn!("failed to decode message: {e}");
        }
    }
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

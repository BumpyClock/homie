use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use homie_protocol::{
    decode_message, encode_message, ClientHello, HandshakeResponse, HelloReject, HelloRejectCode,
    Message as ProtoMessage, ServerHello, ServiceCapability, VersionRange, PROTOCOL_VERSION,
};

use crate::auth::AuthOutcome;

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
        services: vec![ServiceCapability {
            service: "terminal".into(),
            version: "1.0".into(),
        }],
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

    // Outbound channel for sending messages from handlers.
    let (_outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(64);

    loop {
        tokio::select! {
            // Incoming WS message.
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        handle_text_message(sink, &text).await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        handle_binary_message(sink, &data).await;
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
            // Outbound messages from internal handlers.
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
        }
    }
}

async fn handle_text_message(sink: &mut SplitSink<WebSocket, Message>, text: &str) {
    match decode_message(text) {
        Ok(ProtoMessage::Request(req)) => {
            tracing::debug!(method = %req.method, id = %req.id, "request");
            // Placeholder: method routing will be added in US-004.
            let resp = ProtoMessage::Response(homie_protocol::Response::error(
                req.id,
                homie_protocol::error_codes::METHOD_NOT_FOUND,
                format!("unknown method: {}", req.method),
            ));
            if let Ok(json) = encode_message(&resp) {
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

async fn handle_binary_message(_sink: &mut SplitSink<WebSocket, Message>, data: &[u8]) {
    // Placeholder: binary frame routing will be added in US-003.
    match homie_protocol::BinaryFrame::decode(data) {
        Ok(frame) => {
            tracing::debug!(session = %frame.session_id, stream = ?frame.stream, len = frame.payload.len(), "binary frame");
        }
        Err(e) => {
            tracing::warn!("invalid binary frame: {e}");
        }
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

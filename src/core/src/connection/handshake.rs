use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use homie_protocol::{
    ClientHello, HandshakeResponse, HelloReject, HelloRejectCode, ServerHello, VersionRange,
    PROTOCOL_VERSION,
};

use crate::auth::AuthOutcome;
use crate::authz::context_for_outcome;
use crate::connection::message_loop::run_message_loop;
use crate::connection::types::{Connection, ConnectionParams, MessageLoopParams};
use tokio::time::timeout;

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
        cron_runner,
        homie_config,
        exec_policy,
        roci,
        pairing_default_ttl_secs,
        pairing_retention_secs,
    } = params;

    let conn_id = uuid::Uuid::new_v4();
    let span = tracing::info_span!("conn", id = %conn_id);
    let _enter = span.enter();

    let (mut sink, mut stream): (SplitSink<WebSocket, Message>, SplitStream<WebSocket>) =
        socket.split();

    // ── Phase 1: Handshake ───────────────────────────────────────────
    let hello = match timeout(Duration::from_secs(5), stream.next()).await {
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
    let tool_channel = infer_tool_channel_from_client_id(&hello.client_id);

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
        cron_runner,
        homie_config,
        exec_policy,
        roci,
        pairing_default_ttl_secs,
        pairing_retention_secs,
        tool_channel,
    };

    run_message_loop(&mut sink, &mut stream, loop_params).await;

    tracing::info!(conn_id = %conn.id, "connection closed");
}

fn infer_tool_channel_from_client_id(client_id: &str) -> Option<String> {
    let normalized = client_id.trim().to_lowercase();
    if normalized.starts_with("homie-web/") {
        Some("web".to_string())
    } else if normalized.starts_with("homie-mobile/") {
        Some("mobile".to_string())
    } else if normalized.starts_with("homie-whatsapp/") {
        Some("whatsapp".to_string())
    } else {
        None
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
        .send(Message::Close(Some(CloseFrame {
            code: 4001,
            reason: reason.into(),
        })))
        .await;
}

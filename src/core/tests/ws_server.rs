use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use homie_core::{Role, ServerConfig, SqliteStore, TailscaleIdentity, TailscaleWhois};
use homie_protocol::{
    ClientHello, HandshakeResponse, HelloRejectCode, Request, VersionRange, PROTOCOL_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ── Helpers ──────────────────────────────────────────────────────────

struct NoopWhois;

impl TailscaleWhois for NoopWhois {
    fn whois(&self, _ip: &str) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>> {
        Box::pin(async { None })
    }
}

async fn start_server(config: ServerConfig) -> SocketAddr {
    let store = Arc::new(SqliteStore::open_memory().unwrap());
    let app = homie_core::build_router(config, NoopWhois, store);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    addr
}

async fn connect_ws(addr: SocketAddr) -> WsStream {
    let url = format!("ws://{addr}/ws");
    let (stream, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    stream
}

fn client_hello(min: u16, max: u16) -> String {
    serde_json::to_string(&ClientHello {
        protocol: VersionRange::new(min, max),
        client_id: "test-client/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap()
}

fn text_msg(s: String) -> tungstenite::Message {
    tungstenite::Message::Text(s.into())
}

/// Read the next text message from the WS stream, automatically
/// replying to Ping frames and skipping Pong frames.
async fn next_text(ws: &mut WsStream) -> String {
    loop {
        match ws.next().await {
            Some(Ok(tungstenite::Message::Text(t))) => return t.to_string(),
            Some(Ok(tungstenite::Message::Ping(data))) => {
                let _ = ws.send(tungstenite::Message::Pong(data)).await;
            }
            Some(Ok(tungstenite::Message::Pong(_))) => continue,
            Some(Ok(other)) => panic!("unexpected message: {other:?}"),
            Some(Err(e)) => panic!("ws error: {e}"),
            None => panic!("ws stream ended unexpectedly"),
        }
    }
}

/// Read from the WS stream until a Close frame, EOF, or error.
/// Replies to pings along the way. Returns true if closed.
async fn expect_close(ws: &mut WsStream, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Close(_))) | None | Some(Err(_)) => return true,
                    // Don't reply to pings — we want the server to see us as idle.
                    Some(Ok(tungstenite::Message::Ping(_))) => continue,
                    Some(Ok(tungstenite::Message::Pong(_))) => continue,
                    Some(Ok(_)) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                return false;
            }
        }
    }
}

async fn rpc_err(
    ws: &mut WsStream,
    method: &str,
    params: Option<serde_json::Value>,
) -> homie_protocol::RpcError {
    let req = homie_protocol::Message::Request(Request::new(method, params));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let t = next_text(ws).await;
    let msg: homie_protocol::Message = serde_json::from_str(&t).unwrap();
    match msg {
        homie_protocol::Message::Response(r) => r.error.expect("expected error response"),
        other => panic!("expected response, got {other:?}"),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_endpoint() {
    let addr = start_server(ServerConfig::default()).await;

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /health HTTP/1.1\r\nHost: {addr}\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("200"));
    assert!(response.contains("ok"));
}

#[tokio::test]
async fn successful_handshake() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(1, 1))).await.unwrap();

    let t = next_text(&mut ws).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();

    match resp {
        HandshakeResponse::Hello(hello) => {
            assert_eq!(hello.protocol_version, PROTOCOL_VERSION);
            assert_eq!(hello.identity.as_deref(), Some("local"));
            assert!(!hello.services.is_empty());
        }
        HandshakeResponse::Reject(r) => panic!("unexpected reject: {r:?}"),
    }
}

#[tokio::test]
async fn version_mismatch_rejected() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(99, 100))).await.unwrap();

    let t = next_text(&mut ws).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();

    match resp {
        HandshakeResponse::Reject(r) => {
            assert_eq!(r.code, HelloRejectCode::VersionMismatch);
        }
        HandshakeResponse::Hello(_) => panic!("expected reject"),
    }
}

#[tokio::test]
async fn method_not_found_after_handshake() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(1, 1))).await.unwrap();
    let _ = next_text(&mut ws).await; // consume ServerHello

    let req = homie_protocol::Message::Request(Request::new("nonexistent.method", None));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let t = next_text(&mut ws).await;
    let resp: homie_protocol::Message = serde_json::from_str(&t).unwrap();

    match resp {
        homie_protocol::Message::Response(r) => {
            assert!(r.error.is_some());
            assert_eq!(
                r.error.as_ref().unwrap().code,
                homie_protocol::error_codes::METHOD_NOT_FOUND
            );
        }
        other => panic!("expected response, got {other:?}"),
    }
}

#[tokio::test]
async fn server_sends_ping_connection_stays_alive() {
    let config = ServerConfig {
        heartbeat_interval: Duration::from_millis(100),
        idle_timeout: Duration::from_secs(60),
        ..Default::default()
    };
    let addr = start_server(config).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(1, 1))).await.unwrap();
    let _ = next_text(&mut ws).await;

    // Wait past several heartbeat intervals.
    tokio::time::sleep(Duration::from_millis(350)).await;

    // Confirm connection is alive by sending a request.
    let req = homie_protocol::Message::Request(Request::new("ping.test", None));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let t = next_text(&mut ws).await;
    let resp: homie_protocol::Message = serde_json::from_str(&t).unwrap();
    assert!(matches!(resp, homie_protocol::Message::Response(_)));
}

#[tokio::test]
async fn idle_timeout_closes_connection() {
    let config = ServerConfig {
        heartbeat_interval: Duration::from_secs(600), // no pings
        idle_timeout: Duration::from_millis(200),
        ..Default::default()
    };
    let addr = start_server(config).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(1, 1))).await.unwrap();
    let _ = next_text(&mut ws).await;

    // Should close within idle timeout + buffer.
    assert!(
        expect_close(&mut ws, Duration::from_secs(5)).await,
        "expected connection to close due to idle timeout"
    );
}

#[tokio::test]
async fn unauthorized_requests_are_rejected_for_viewer_role() {
    let config = ServerConfig {
        local_role: Role::Viewer,
        ..Default::default()
    };
    let addr = start_server(config).await;
    let mut ws = connect_ws(addr).await;

    ws.send(text_msg(client_hello(1, 1))).await.unwrap();
    let _ = next_text(&mut ws).await;

    let err = rpc_err(&mut ws, "terminal.session.start", None).await;
    assert_eq!(err.code, homie_protocol::error_codes::UNAUTHORIZED);
}

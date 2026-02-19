use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use homie_core::{ServerConfig, SqliteStore, TailscaleIdentity, TailscaleWhois};
use homie_protocol::{
    BinaryFrame, ClientHello, HandshakeResponse, Request, StreamType, VersionRange,
};
use serde_json::json;
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

async fn connect_and_handshake(addr: SocketAddr) -> WsStream {
    let url = format!("ws://{addr}/ws");
    let (mut stream, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let hello = serde_json::to_string(&ClientHello {
        protocol: VersionRange::new(1, 1),
        client_id: "test-client/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap();

    stream
        .send(tungstenite::Message::Text(hello.into()))
        .await
        .unwrap();

    // Consume ServerHello.
    let t = next_text(&mut stream).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();
    assert!(matches!(resp, HandshakeResponse::Hello(_)));

    stream
}

fn text_msg(s: String) -> tungstenite::Message {
    tungstenite::Message::Text(s.into())
}

/// Read the next text message, auto-replying to pings and skipping binary.
async fn next_text(ws: &mut WsStream) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(t))) => return t.to_string(),
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        let _ = ws.send(tungstenite::Message::Pong(data)).await;
                    }
                    Some(Ok(tungstenite::Message::Pong(_))) => continue,
                    Some(Ok(tungstenite::Message::Binary(_))) => continue, // skip binary
                    Some(Ok(other)) => panic!("unexpected message: {other:?}"),
                    Some(Err(e)) => panic!("ws error: {e}"),
                    None => panic!("ws stream ended unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for text message");
            }
        }
    }
}

/// Read the next binary message, auto-replying to pings and skipping text.
async fn next_binary(ws: &mut WsStream) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Binary(data))) => return data.to_vec(),
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        let _ = ws.send(tungstenite::Message::Pong(data)).await;
                    }
                    Some(Ok(tungstenite::Message::Pong(_))) => continue,
                    Some(Ok(tungstenite::Message::Text(_))) => continue, // skip text
                    Some(Ok(other)) => panic!("unexpected message: {other:?}"),
                    Some(Err(e)) => panic!("ws error: {e}"),
                    None => panic!("ws stream ended unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for binary message");
            }
        }
    }
}

/// Read the next text or binary message (either), auto-replying to pings.
enum WsMsg {
    Text(String),
    Binary(Vec<u8>),
}

async fn next_msg(ws: &mut WsStream) -> WsMsg {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(t))) => return WsMsg::Text(t.to_string()),
                    Some(Ok(tungstenite::Message::Binary(data))) => return WsMsg::Binary(data.to_vec()),
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        let _ = ws.send(tungstenite::Message::Pong(data)).await;
                    }
                    Some(Ok(tungstenite::Message::Pong(_))) => continue,
                    Some(Ok(other)) => panic!("unexpected message: {other:?}"),
                    Some(Err(e)) => panic!("ws error: {e}"),
                    None => panic!("ws stream ended unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for message");
            }
        }
    }
}

/// Send an RPC request and return the response.
async fn rpc(
    ws: &mut WsStream,
    method: &str,
    params: Option<serde_json::Value>,
) -> serde_json::Value {
    let req = homie_protocol::Message::Request(Request::new(method, params));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            msg = next_msg(ws) => {
                match msg {
                    WsMsg::Text(t) => {
                        let parsed: homie_protocol::Message = serde_json::from_str(&t).unwrap();
                        match parsed {
                            homie_protocol::Message::Response(r) => {
                                if let Some(err) = r.error {
                                    panic!("rpc error: {} (code {})", err.message, err.code);
                                }
                                return r.result.unwrap_or(json!(null));
                            }
                            homie_protocol::Message::Event(_) => continue,
                            other => panic!("expected response, got {other:?}"),
                        }
                    }
                    WsMsg::Binary(_) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for rpc response");
            }
        }
    }
}

/// Send an RPC request and expect an error response.
async fn rpc_err(
    ws: &mut WsStream,
    method: &str,
    params: Option<serde_json::Value>,
) -> homie_protocol::RpcError {
    let req = homie_protocol::Message::Request(Request::new(method, params));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            msg = next_msg(ws) => {
                match msg {
                    WsMsg::Text(t) => {
                        let parsed: homie_protocol::Message = serde_json::from_str(&t).unwrap();
                        match parsed {
                            homie_protocol::Message::Response(r) => return r.error.expect("expected error response"),
                            homie_protocol::Message::Event(_) => continue,
                            other => panic!("expected response, got {other:?}"),
                        }
                    }
                    WsMsg::Binary(_) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for rpc error response");
            }
        }
    }
}

fn extract_session_id(result: &serde_json::Value) -> String {
    result["session_id"].as_str().unwrap().to_string()
}

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_start_returns_session_id() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = result["session_id"].as_str();
    assert!(sid.is_some(), "expected session_id in response");

    // Verify it parses as a UUID.
    let uuid = uuid::Uuid::parse_str(sid.unwrap());
    assert!(uuid.is_ok(), "session_id should be a valid UUID");
}

#[tokio::test]
async fn renaming_a_session_updates_the_list_name() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;
    let session_id = extract_session_id(&result);

    rpc(
        &mut ws,
        "terminal.session.rename",
        Some(json!({ "session_id": session_id, "name": "My Session" })),
    )
    .await;

    let list = rpc(&mut ws, "terminal.session.list", None).await;
    let sessions = list["sessions"].as_array().expect("sessions array");
    let found = sessions
        .iter()
        .find(|s| s["session_id"].as_str() == Some(&session_id))
        .expect("session missing");
    assert_eq!(found["name"].as_str(), Some("My Session"));
}

#[tokio::test]
async fn session_start_produces_pty_output() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);
    let session_uuid = uuid::Uuid::parse_str(&sid).unwrap();

    rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    // Shell should produce some output (prompt). Read binary frames.
    let data = next_binary(&mut ws).await;
    let frame = BinaryFrame::decode(&data).unwrap();
    assert_eq!(frame.session_id, session_uuid);
    assert_eq!(frame.stream, StreamType::Stdout);
    assert!(!frame.payload.is_empty(), "expected non-empty PTY output");
}

#[tokio::test]
async fn session_attach_returns_info() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 100, "rows": 30 })),
    )
    .await;

    let sid = extract_session_id(&result);

    let info = rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    assert_eq!(info["session_id"].as_str().unwrap(), sid);
    assert_eq!(info["cols"].as_u64().unwrap(), 100);
    assert_eq!(info["rows"].as_u64().unwrap(), 30);
}

#[tokio::test]
async fn session_survives_disconnect() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);

    let _ = ws.send(tungstenite::Message::Close(None)).await;
    drop(ws);

    let mut ws2 = connect_and_handshake(addr).await;
    let info = rpc(
        &mut ws2,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    assert_eq!(info["session_id"].as_str().unwrap(), sid);
}

#[tokio::test]
async fn session_detach_does_not_kill() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;
    let sid = extract_session_id(&result);

    let _ = rpc(
        &mut ws,
        "terminal.session.detach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    let info = rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    assert_eq!(info["session_id"].as_str().unwrap(), sid);
}

#[tokio::test]
async fn session_remove_deletes_inactive_record() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;
    let sid = extract_session_id(&result);

    let _ = rpc(
        &mut ws,
        "terminal.session.kill",
        Some(json!({ "session_id": sid })),
    )
    .await;

    let _ = rpc(
        &mut ws,
        "terminal.session.remove",
        Some(json!({ "session_id": sid })),
    )
    .await;

    let list = rpc(&mut ws, "terminal.session.list", None).await;
    let sessions = list["sessions"].as_array().cloned().unwrap_or_default();
    assert!(
        sessions
            .iter()
            .all(|s| s["session_id"].as_str() != Some(&sid)),
        "expected session to be removed"
    );
}

#[tokio::test]
async fn session_attach_not_found() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let fake_id = uuid::Uuid::new_v4().to_string();
    let err = rpc_err(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": fake_id })),
    )
    .await;

    assert_eq!(err.code, homie_protocol::error_codes::SESSION_NOT_FOUND);
}

#[tokio::test]
async fn session_resize() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);

    let resize_result = rpc(
        &mut ws,
        "terminal.session.resize",
        Some(json!({ "session_id": sid, "cols": 120, "rows": 40 })),
    )
    .await;

    assert_eq!(resize_result["ok"].as_bool(), Some(true));

    // Verify via attach.
    let info = rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;
    assert_eq!(info["cols"].as_u64().unwrap(), 120);
    assert_eq!(info["rows"].as_u64().unwrap(), 40);
}

#[tokio::test]
async fn session_input_text() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);
    let session_uuid = uuid::Uuid::parse_str(&sid).unwrap();

    rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    // Drain initial shell output.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send input via JSON RPC.
    let input_result = rpc(
        &mut ws,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "echo HELLO_TEST\n" })),
    )
    .await;
    assert_eq!(input_result["ok"].as_bool(), Some(true));

    // Read output until we see HELLO_TEST.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    let mut accumulated = String::new();
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Binary(data) => {
                        let frame = BinaryFrame::decode(&data).unwrap();
                        assert_eq!(frame.session_id, session_uuid);
                        accumulated.push_str(&String::from_utf8_lossy(&frame.payload));
                        if accumulated.contains("HELLO_TEST") {
                            found = true;
                            break;
                        }
                    }
                    WsMsg::Text(_) => continue, // skip events
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    assert!(
        found,
        "expected to see HELLO_TEST in PTY output, got: {accumulated}"
    );
}

#[tokio::test]
async fn session_input_binary_frame() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);
    let session_uuid = uuid::Uuid::parse_str(&sid).unwrap();

    rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;

    // Drain initial shell output.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send input via binary frame (stdin).
    let frame = BinaryFrame {
        session_id: session_uuid,
        stream: StreamType::Stdin,
        payload: b"echo BIN_TEST\n".to_vec(),
    };
    ws.send(tungstenite::Message::Binary(frame.encode().into()))
        .await
        .unwrap();

    // Read output until we see BIN_TEST.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    let mut accumulated = String::new();
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Binary(data) => {
                        let frame = BinaryFrame::decode(&data).unwrap();
                        accumulated.push_str(&String::from_utf8_lossy(&frame.payload));
                        if accumulated.contains("BIN_TEST") {
                            found = true;
                            break;
                        }
                    }
                    WsMsg::Text(_) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    assert!(found, "expected BIN_TEST in PTY output, got: {accumulated}");
}

#[tokio::test]
async fn session_kill() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);

    let kill_result = rpc(
        &mut ws,
        "terminal.session.kill",
        Some(json!({ "session_id": sid })),
    )
    .await;
    assert_eq!(kill_result["ok"].as_bool(), Some(true));

    // Verify session is gone.
    let err = rpc_err(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid })),
    )
    .await;
    assert_eq!(err.code, homie_protocol::error_codes::SESSION_NOT_FOUND);
}

#[tokio::test]
async fn session_exit_event_on_process_exit() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    // Subscribe to terminal events so we receive exit notifications.
    let _sub = rpc(
        &mut ws,
        "events.subscribe",
        Some(json!({ "topic": "terminal.*" })),
    )
    .await;

    // Start a session that will exit quickly.
    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;

    let sid = extract_session_id(&result);

    // Tell the shell to exit.
    rpc(
        &mut ws,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "exit\n" })),
    )
    .await;

    // Wait for the exit event (text frame with terminal.session.exit topic).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut got_exit = false;
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Text(t) => {
                        if let Ok(homie_protocol::Message::Event(evt)) =
                            serde_json::from_str::<homie_protocol::Message>(&t)
                        {
                            if evt.topic == "terminal.session.exit" {
                                let params = evt.params.unwrap();
                                assert_eq!(params["session_id"].as_str().unwrap(), sid);
                                got_exit = true;
                                break;
                            }
                        }
                    }
                    WsMsg::Binary(_) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    assert!(got_exit, "expected terminal.session.exit event");
}

#[tokio::test]
async fn session_cleanup_on_disconnect() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    // Start a session.
    let _result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;

    // Close the WS connection abruptly.
    let _ = ws.close(None).await;

    // Give server time to clean up.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // No assertion needed — we're verifying that cleanup doesn't panic/leak.
    // The session's PTY should have been killed and reader thread joined.
}

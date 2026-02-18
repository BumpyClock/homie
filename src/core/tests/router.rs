use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use homie_core::{ServerConfig, SqliteStore, TailscaleIdentity, TailscaleWhois};
use homie_protocol::{ClientHello, HandshakeResponse, Request, VersionRange};
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

    let t = next_text(&mut stream).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();
    assert!(matches!(resp, HandshakeResponse::Hello(_)));

    stream
}

fn text_msg(s: String) -> tungstenite::Message {
    tungstenite::Message::Text(s.into())
}

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
                    Some(Ok(tungstenite::Message::Binary(_))) => continue,
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

/// Text or binary WS message.
enum WsMsg {
    Text(String),
    Binary(#[allow(dead_code)] Vec<u8>),
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
                            // Events can arrive interleaved with responses.
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

// ── Tests: Router dispatch ──────────────────────────────────────────

#[tokio::test]
async fn router_dispatches_terminal_methods() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    // terminal.session.start should be handled by terminal service.
    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "cols": 80, "rows": 24 })),
    )
    .await;
    assert!(result["session_id"].is_string());
}

#[tokio::test]
async fn router_rejects_unknown_service() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let err = rpc_err(&mut ws, "files.list", None).await;
    assert_eq!(err.code, homie_protocol::error_codes::METHOD_NOT_FOUND);
    assert!(err.message.contains("files"));
}

#[tokio::test]
async fn router_rejects_invalid_method_format() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let err = rpc_err(&mut ws, "", None).await;
    assert_eq!(err.code, homie_protocol::error_codes::METHOD_NOT_FOUND);
}

#[tokio::test]
async fn presence_register_and_list_returns_nodes() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let _ = rpc(
        &mut ws,
        "presence.register",
        Some(json!({
            "node_id": "node-1",
            "name": "test-node",
            "version": "0.1.0",
            "services": [{"service": "terminal", "version": "1.0"}]
        })),
    )
    .await;

    let result = rpc(&mut ws, "presence.list", None).await;
    let nodes = result["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|n| n["node_id"] == "node-1"));
}

#[tokio::test]
async fn jobs_start_and_status_returns_job() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(&mut ws, "jobs.start", Some(json!({ "name": "sample-job" }))).await;

    let job_id = result["job"]["job_id"].as_str().unwrap().to_string();
    let status = rpc(&mut ws, "jobs.status", Some(json!({ "job_id": job_id }))).await;
    assert_eq!(status["job"]["name"], "sample-job");
}

#[tokio::test]
async fn cron_start_and_status_is_routed() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "cron.start",
        Some(json!({
            "name": "nightly-clean",
            "schedule": "* * * * * *",
            "command": "echo tidy",
        })),
    )
    .await;

    let cron_id = result["cron"]["cron_id"].as_str().unwrap().to_string();
    let status = rpc(&mut ws, "cron.status", Some(json!({ "cron_id": cron_id }))).await;
    assert_eq!(status["cron"]["status"], "active");
}

#[tokio::test]
async fn handshake_includes_cron_service() {
    let addr = start_server(ServerConfig::default()).await;
    let url = format!("ws://{addr}/ws");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let hello = serde_json::to_string(&ClientHello {
        protocol: VersionRange::new(1, 1),
        client_id: "test-client/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap();
    ws.send(text_msg(hello)).await.unwrap();

    let text = next_text(&mut ws).await;
    let resp: HandshakeResponse = serde_json::from_str(&text).unwrap();
    let services = match resp {
        HandshakeResponse::Hello(h) => h.services,
        _ => panic!("expected server hello"),
    };
    assert!(services.iter().any(|service| service.service == "cron"));
}

#[tokio::test]
async fn pairing_request_and_approve_updates_status() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(&mut ws, "pairing.request", None).await;
    let pairing_id = result["pairing"]["pairing_id"]
        .as_str()
        .unwrap()
        .to_string();

    let approved = rpc(
        &mut ws,
        "pairing.approve",
        Some(json!({ "pairing_id": pairing_id })),
    )
    .await;
    assert_eq!(approved["pairing"]["status"], "approved");
}

#[tokio::test]
async fn notifications_register_and_send_emits_event() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let _ = rpc(
        &mut ws,
        "notifications.register",
        Some(json!({ "target": "device-1" })),
    )
    .await;

    let _ = rpc(
        &mut ws,
        "events.subscribe",
        Some(json!({ "topic": "notifications.*" })),
    )
    .await;

    let _ = rpc(
        &mut ws,
        "notifications.send",
        Some(json!({ "title": "hi", "body": "there", "target": "device-1" })),
    )
    .await;

    let msg = next_msg(&mut ws).await;
    match msg {
        WsMsg::Text(t) => {
            let evt: homie_protocol::Message = serde_json::from_str(&t).unwrap();
            match evt {
                homie_protocol::Message::Event(event) => {
                    assert_eq!(event.topic, "notifications.sent");
                }
                other => panic!("expected event, got {other:?}"),
            }
        }
        WsMsg::Binary(_) => panic!("expected text event"),
    }
}

// ── Tests: Subscriptions ─────────────────────────────────────────────

#[tokio::test]
async fn subscribe_returns_subscription_id() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "events.subscribe",
        Some(json!({ "topic": "terminal.*" })),
    )
    .await;

    let sub_id = result["subscription_id"].as_str();
    assert!(sub_id.is_some(), "expected subscription_id");
    // Verify it parses as UUID.
    assert!(uuid::Uuid::parse_str(sub_id.unwrap()).is_ok());
}

#[tokio::test]
async fn subscribe_missing_topic_returns_error() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let err = rpc_err(&mut ws, "events.subscribe", Some(json!({}))).await;
    assert_eq!(err.code, homie_protocol::error_codes::INVALID_PARAMS);
}

#[tokio::test]
async fn unsubscribe_removes_subscription() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let result = rpc(
        &mut ws,
        "events.subscribe",
        Some(json!({ "topic": "terminal.*" })),
    )
    .await;
    let sub_id = result["subscription_id"].as_str().unwrap().to_string();

    let unsub = rpc(
        &mut ws,
        "events.unsubscribe",
        Some(json!({ "subscription_id": sub_id })),
    )
    .await;
    assert_eq!(unsub["ok"].as_bool(), Some(true));
}

#[tokio::test]
async fn unsubscribe_nonexistent_returns_error() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    let fake_id = uuid::Uuid::new_v4().to_string();
    let err = rpc_err(
        &mut ws,
        "events.unsubscribe",
        Some(json!({ "subscription_id": fake_id })),
    )
    .await;
    assert_eq!(err.code, homie_protocol::error_codes::INVALID_PARAMS);
}

#[tokio::test]
async fn exit_event_only_when_subscribed() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    // Do NOT subscribe. Start a session that exits quickly.
    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;
    let sid = result["session_id"].as_str().unwrap().to_string();

    rpc(
        &mut ws,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "exit\n" })),
    )
    .await;

    // Wait for a bit — should NOT receive an exit event.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut got_exit = false;
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Text(t) => {
                        if let Ok(m) = serde_json::from_str::<homie_protocol::Message>(&t) {
                            if let homie_protocol::Message::Event(evt) = m {
                                if evt.topic == "terminal.session.exit" {
                                    got_exit = true;
                                    break;
                                }
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

    assert!(
        !got_exit,
        "should NOT receive exit event without subscription"
    );
}

#[tokio::test]
async fn exit_event_with_wildcard_subscription() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

    // Subscribe to all events.
    rpc(&mut ws, "events.subscribe", Some(json!({ "topic": "*" }))).await;

    let result = rpc(
        &mut ws,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;
    let sid = result["session_id"].as_str().unwrap().to_string();

    rpc(
        &mut ws,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "exit\n" })),
    )
    .await;

    // Should receive exit event.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut got_exit = false;
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Text(t) => {
                        if let Ok(m) = serde_json::from_str::<homie_protocol::Message>(&t) {
                            if let homie_protocol::Message::Event(evt) = m {
                                if evt.topic == "terminal.session.exit" {
                                    let params = evt.params.unwrap();
                                    assert_eq!(params["session_id"].as_str().unwrap(), sid);
                                    got_exit = true;
                                    break;
                                }
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

    assert!(
        got_exit,
        "expected terminal.session.exit event with * subscription"
    );
}

#[tokio::test]
async fn exit_events_are_broadcast_to_all_connections() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws1 = connect_and_handshake(addr).await;
    let mut ws2 = connect_and_handshake(addr).await;

    rpc(
        &mut ws1,
        "events.subscribe",
        Some(json!({ "topic": "terminal.*" })),
    )
    .await;
    rpc(
        &mut ws2,
        "events.subscribe",
        Some(json!({ "topic": "terminal.*" })),
    )
    .await;

    let result = rpc(
        &mut ws1,
        "terminal.session.start",
        Some(json!({ "shell": "/bin/sh", "cols": 80, "rows": 24 })),
    )
    .await;
    let sid = result["session_id"].as_str().unwrap().to_string();

    rpc(
        &mut ws1,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "exit\n" })),
    )
    .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut got_exit_1 = false;
    let mut got_exit_2 = false;

    loop {
        if got_exit_1 && got_exit_2 {
            break;
        }

        tokio::select! {
            msg = next_msg(&mut ws1), if !got_exit_1 => {
                if let WsMsg::Text(t) = msg {
                    if let Ok(m) = serde_json::from_str::<homie_protocol::Message>(&t) {
                        if let homie_protocol::Message::Event(evt) = m {
                            if evt.topic == "terminal.session.exit" {
                                got_exit_1 = true;
                            }
                        }
                    }
                }
            }
            msg = next_msg(&mut ws2), if !got_exit_2 => {
                if let WsMsg::Text(t) = msg {
                    if let Ok(m) = serde_json::from_str::<homie_protocol::Message>(&t) {
                        if let homie_protocol::Message::Event(evt) = m {
                            if evt.topic == "terminal.session.exit" {
                                got_exit_2 = true;
                            }
                        }
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    assert!(got_exit_1, "expected ws1 to receive terminal.session.exit");
    assert!(got_exit_2, "expected ws2 to receive terminal.session.exit");
}

// ── Tests: Handshake advertises services from registry ──────────────

#[tokio::test]
async fn handshake_advertises_registered_services() {
    let addr = start_server(ServerConfig::default()).await;
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

    let t = next_text(&mut stream).await;
    let resp: HandshakeResponse = serde_json::from_str(&t).unwrap();

    match resp {
        HandshakeResponse::Hello(hello) => {
            assert!(!hello.services.is_empty());
            assert_eq!(hello.services[0].service, "terminal");
            assert_eq!(hello.services[0].version, "1.0");
        }
        HandshakeResponse::Reject(r) => panic!("unexpected reject: {r:?}"),
    }
}

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use homie_core::{ServerConfig, SqliteStore, TailscaleIdentity, TailscaleWhois};
use homie_protocol::{ClientHello, HandshakeResponse, Request, VersionRange};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite;

pub type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ── Test Infrastructure ──────────────────────────────────────────────

pub struct NoopWhois;

impl TailscaleWhois for NoopWhois {
    fn whois(&self, _ip: &str) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>> {
        Box::pin(async { None })
    }
}

pub async fn start_server(config: ServerConfig) -> SocketAddr {
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

pub async fn connect_and_handshake(addr: SocketAddr) -> WsStream {
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

pub fn text_msg(s: String) -> tungstenite::Message {
    tungstenite::Message::Text(s.into())
}

/// Read the next text message, auto-replying to pings and skipping binary.
pub async fn next_text(ws: &mut WsStream) -> String {
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
pub async fn next_binary(ws: &mut WsStream) -> Vec<u8> {
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
pub enum WsMsg {
    Text(String),
    Binary(Vec<u8>),
}

pub async fn next_msg(ws: &mut WsStream) -> WsMsg {
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
pub async fn rpc(
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
                                return r.result.unwrap_or(serde_json::json!(null));
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
pub async fn rpc_err(
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

pub fn extract_session_id(result: &serde_json::Value) -> String {
    result["session_id"].as_str().unwrap().to_string()
}

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use homie_core::{HomieConfig, ServerConfig, SqliteStore, TailscaleIdentity, TailscaleWhois};
use homie_protocol::{ClientHello, HandshakeResponse, Request, VersionRange};
use roci::auth::{FileTokenStore, TokenStoreConfig};
use roci::roci_providers::auth::openai_codex::OpenAiCodexAuth;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;

pub type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

static LIVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct NoopWhois;

impl TailscaleWhois for NoopWhois {
    fn whois(&self, _ip: &str) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>> {
        Box::pin(async { None })
    }
}

pub fn live_enabled() -> bool {
    matches!(std::env::var("HOMIE_LIVE_TESTS").as_deref(), Ok("1"))
}

pub fn lock_live_tests() -> &'static Mutex<()> {
    LIVE_LOCK.get_or_init(|| Mutex::new(()))
}

fn env_non_empty(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub fn core_tool_enabled(config: &HomieConfig, tool_name: &str) -> bool {
    let Some(core) = config.tools.providers.get("core") else {
        return true;
    };
    if matches!(core.enabled, Some(false)) {
        return false;
    }
    let in_allow = if core.allow_tools.is_empty() {
        true
    } else {
        core.allow_tools.iter().any(|entry| entry == tool_name)
    };
    in_allow && !core.deny_tools.iter().any(|entry| entry == tool_name)
}

pub async fn pick_model(config: &HomieConfig) -> Option<String> {
    if env_non_empty("OPENAI_API_KEY") {
        return Some("openai:gpt-4o-mini".to_string());
    }
    let creds_dir = config.credentials_dir().ok()?;
    let store = FileTokenStore::new(TokenStoreConfig::new(creds_dir));
    let auth = OpenAiCodexAuth::new(Arc::new(store.clone()));
    let _ = auth.import_codex_auth_json(None);
    if auth.get_token().await.is_ok() {
        return Some("openai-codex:gpt-5.1-codex".to_string());
    }
    None
}

pub fn search_provider_ready(config: &HomieConfig) -> Result<String, String> {
    if !config.tools.web.search.enabled {
        return Err("tools.web.search.enabled=false".to_string());
    }
    if !core_tool_enabled(config, "web_search") {
        return Err("core provider disables web_search".to_string());
    }
    let provider = if config
        .tools
        .web
        .search
        .provider
        .trim()
        .eq_ignore_ascii_case("searxng")
    {
        "searxng"
    } else {
        "brave"
    };
    if provider == "brave" {
        let has_key = !config.tools.web.search.brave.api_key.trim().is_empty()
            || env_non_empty("BRAVE_API_KEY");
        if !has_key {
            return Err("brave provider missing api key".to_string());
        }
    } else {
        let has_base = !config.tools.web.search.searxng.base_url.trim().is_empty()
            || env_non_empty("SEARXNG_BASE_URL");
        if !has_base {
            return Err("searxng provider missing base_url".to_string());
        }
    }
    Ok(provider.to_string())
}

pub fn fetch_backend_ready(config: &HomieConfig) -> Result<(), String> {
    if !config.tools.web.fetch.enabled {
        return Err("tools.web.fetch.enabled=false".to_string());
    }
    if !core_tool_enabled(config, "web_fetch") {
        return Err("core provider disables web_fetch".to_string());
    }
    let firecrawl_ready = config.tools.web.fetch.firecrawl.enabled
        || !config.tools.web.fetch.firecrawl.api_key.trim().is_empty()
        || env_non_empty("FIRECRAWL_API_KEY");
    if !config.tools.web.fetch.readability && !firecrawl_ready {
        return Err("web_fetch readability disabled and firecrawl unavailable".to_string());
    }
    Ok(())
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
        client_id: "live-tools-test/0.1.0".into(),
        auth_token: None,
        capabilities: vec![],
    })
    .unwrap();

    stream
        .send(tungstenite::Message::Text(hello.into()))
        .await
        .unwrap();

    let text = next_text(&mut stream).await;
    let resp: HandshakeResponse = serde_json::from_str(&text).unwrap();
    assert!(matches!(resp, HandshakeResponse::Hello(_)));

    stream
}

pub fn text_msg(s: String) -> tungstenite::Message {
    tungstenite::Message::Text(s.into())
}

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
                    Some(Ok(tungstenite::Message::Binary(_))) => continue,
                    Some(Ok(other)) => panic!("unexpected message: {other:?}"),
                    Some(Err(err)) => panic!("ws error: {err}"),
                    None => panic!("ws stream ended unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for text");
            }
        }
    }
}

pub enum WsMsg {
    Text(String),
    Binary(#[allow(dead_code)] Vec<u8>),
}

pub async fn next_msg(ws: &mut WsStream) -> WsMsg {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
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
                    Some(Err(err)) => panic!("ws error: {err}"),
                    None => panic!("ws stream ended unexpectedly"),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for ws message");
            }
        }
    }
}

pub async fn rpc(ws: &mut WsStream, method: &str, params: Option<Value>) -> Value {
    let req = homie_protocol::Message::Request(Request::new(method, params));
    let json = homie_protocol::encode_message(&req).unwrap();
    ws.send(text_msg(json)).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        tokio::select! {
            msg = next_msg(ws) => {
                match msg {
                    WsMsg::Text(text) => {
                        let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
                        match parsed {
                            homie_protocol::Message::Response(response) => {
                                if let Some(err) = response.error {
                                    panic!("rpc error: {} (code {})", err.message, err.code);
                                }
                                return response.result.unwrap_or(json!(null));
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

pub async fn start_chat(ws: &mut WsStream) -> String {
    let _ = rpc(ws, "events.subscribe", Some(json!({ "topic": "chat.*" }))).await;
    let created = rpc(ws, "chat.create", None).await;
    created["chat_id"].as_str().expect("chat_id").to_string()
}

pub async fn send_and_wait_for_tool(
    ws: &mut WsStream,
    chat_id: &str,
    model: &str,
    message: &str,
    tool_name: &str,
) -> Value {
    let _ = rpc(
        ws,
        "chat.message.send",
        Some(json!({
            "chat_id": chat_id,
            "message": message,
            "model": model,
            "approval_policy": "always"
        })),
    )
    .await;

    wait_for_tool_result(ws, tool_name, Duration::from_secs(120)).await
}

pub async fn wait_for_tool_result(ws: &mut WsStream, tool_name: &str, timeout: Duration) -> Value {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::select! {
            msg = next_msg(ws) => {
                let WsMsg::Text(text) = msg else { continue };
                let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
                let homie_protocol::Message::Event(event) = parsed else { continue };
                if event.topic != "chat.item.completed" {
                    continue;
                }
                let Some(params) = event.params else { continue };
                let Some(item) = params.get("item") else { continue };
                if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
                    continue;
                }
                if item.get("tool").and_then(Value::as_str) != Some(tool_name) {
                    continue;
                }
                let status = item.get("status").and_then(Value::as_str).unwrap_or_default();
                assert_eq!(status, "completed", "tool status not completed: {item}");
                return item.get("result").cloned().unwrap_or(Value::Null);
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("timeout waiting for tool result: {tool_name}");
            }
        }
    }
}

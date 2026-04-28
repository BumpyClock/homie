use homie_core::ServerConfig;
use homie_protocol::BinaryFrame;
use serde_json::json;

use super::helpers::*;

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
    assert_eq!(frame.stream, homie_protocol::StreamType::Stdout);
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

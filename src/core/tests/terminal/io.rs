use futures::SinkExt;
use homie_core::ServerConfig;
use homie_protocol::BinaryFrame;
use serde_json::json;
use std::time::Duration;

use super::helpers::*;

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
        stream: homie_protocol::StreamType::Stdin,
        payload: b"echo BIN_TEST\n".to_vec(),
    };
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(
        frame.encode().into(),
    ))
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
async fn session_output_before_attach_replays_from_history() {
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

    let input_result = rpc(
        &mut ws,
        "terminal.session.input",
        Some(json!({ "session_id": sid, "data": "echo HISTORY_TEST\n" })),
    )
    .await;
    assert_eq!(input_result["ok"].as_bool(), Some(true));

    tokio::time::sleep(Duration::from_millis(200)).await;

    rpc(
        &mut ws,
        "terminal.session.attach",
        Some(json!({ "session_id": sid, "replay": true })),
    )
    .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut accumulated = String::new();
    loop {
        tokio::select! {
            msg = next_msg(&mut ws) => {
                match msg {
                    WsMsg::Binary(data) => {
                        let frame = BinaryFrame::decode(&data).unwrap();
                        assert_eq!(frame.session_id, session_uuid);
                        accumulated.push_str(&String::from_utf8_lossy(&frame.payload));
                        if accumulated.contains("HISTORY_TEST") {
                            return;
                        }
                    }
                    WsMsg::Text(_) => continue,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!("expected HISTORY_TEST in replayed PTY history, got: {accumulated}");
            }
        }
    }
}

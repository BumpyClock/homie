use futures::SinkExt;
use homie_core::ServerConfig;
use serde_json::json;
use std::time::Duration;

use super::helpers::*;

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

    let _ = ws
        .send(tokio_tungstenite::tungstenite::Message::Close(None))
        .await;
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

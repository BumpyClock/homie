use super::*;
use serde_json::json;
use std::time::Duration;

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

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
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

    assert!(
        !got_exit,
        "should NOT receive exit event without subscription"
    );
}

#[tokio::test]
async fn exit_event_with_wildcard_subscription() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

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
                    if let Ok(homie_protocol::Message::Event(evt)) =
                        serde_json::from_str::<homie_protocol::Message>(&t)
                    {
                        if evt.topic == "terminal.session.exit" {
                            got_exit_1 = true;
                        }
                    }
                }
            }
            msg = next_msg(&mut ws2), if !got_exit_2 => {
                if let WsMsg::Text(t) = msg {
                    if let Ok(homie_protocol::Message::Event(evt)) =
                        serde_json::from_str::<homie_protocol::Message>(&t)
                    {
                        if evt.topic == "terminal.session.exit" {
                            got_exit_2 = true;
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

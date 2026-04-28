use super::*;
use serde_json::json;

#[tokio::test]
async fn router_dispatches_terminal_methods() {
    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;

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

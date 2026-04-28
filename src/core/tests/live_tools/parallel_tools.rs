use std::time::Duration;

use homie_core::{HomieConfig, ServerConfig};
use serde_json::{json, Value};

use super::setup::*;

#[tokio::test]
async fn live_parallel_tool_calls_ls_and_find_single_turn() {
    if !live_enabled() {
        eprintln!("skipping live test; set HOMIE_LIVE_TESTS=1");
        return;
    }
    let _guard = lock_live_tests().lock().await;

    let config = match HomieConfig::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("skipping live test; homie config unavailable: {err}");
            return;
        }
    };
    if !core_tool_enabled(&config, "ls") || !core_tool_enabled(&config, "find") {
        eprintln!("skipping live test; core provider disables ls or find");
        return;
    }
    let Some(model) = pick_model(&config).await else {
        eprintln!("skipping live test; no OPENAI_API_KEY or Codex auth");
        return;
    };

    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;
    let chat_id = start_chat(&mut ws).await;

    let send_result = rpc(
        &mut ws,
        "chat.message.send",
        Some(json!({
            "chat_id": chat_id,
            "message": "In one step, call these tools exactly once each: 1) ls with default arguments 2) find with pattern '*.rs', path '.', limit 5. Then provide a short final answer and stop.",
            "model": model,
            "approval_policy": "always"
        })),
    )
    .await;
    let turn_id = send_result["turn_id"]
        .as_str()
        .expect("turn_id")
        .to_string();

    let mut started_ls = false;
    let mut started_find = false;
    let mut completed_ls = false;
    let mut completed_find = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);

    while tokio::time::Instant::now() < deadline {
        let msg = next_msg(&mut ws).await;
        let WsMsg::Text(text) = msg else { continue };
        let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
        let homie_protocol::Message::Event(event) = parsed else {
            continue;
        };
        let Some(params) = event.params else { continue };
        if params.get("turnId").and_then(Value::as_str) != Some(turn_id.as_str()) {
            continue;
        }

        match event.topic.as_str() {
            "chat.item.started" => {
                let Some(item) = params.get("item") else {
                    continue;
                };
                if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
                    continue;
                }
                match item.get("tool").and_then(Value::as_str) {
                    Some("ls") => started_ls = true,
                    Some("find") => started_find = true,
                    _ => {}
                }
            }
            "chat.item.completed" => {
                let Some(item) = params.get("item") else {
                    continue;
                };
                if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
                    continue;
                }
                if item.get("status").and_then(Value::as_str) != Some("completed") {
                    continue;
                }
                match item.get("tool").and_then(Value::as_str) {
                    Some("ls") => completed_ls = true,
                    Some("find") => completed_find = true,
                    _ => {}
                }
            }
            "chat.turn.completed" => {
                if params.get("status").and_then(Value::as_str) == Some("completed") {
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(started_ls, "expected ls tool call to start");
    assert!(started_find, "expected find tool call to start");
    assert!(completed_ls, "expected ls tool call to complete");
    assert!(completed_find, "expected find tool call to complete");

    // After turn completion, no additional tool calls should start for this turn.
    let quiet_until = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < quiet_until {
        let timeout_left = quiet_until.saturating_duration_since(tokio::time::Instant::now());
        let msg = tokio::time::timeout(timeout_left, next_msg(&mut ws)).await;
        let Ok(WsMsg::Text(text)) = msg else { break };
        let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
        let homie_protocol::Message::Event(event) = parsed else {
            continue;
        };
        if event.topic != "chat.item.started" {
            continue;
        }
        let Some(params) = event.params else { continue };
        if params.get("turnId").and_then(Value::as_str) != Some(turn_id.as_str()) {
            continue;
        }
        let Some(item) = params.get("item") else {
            continue;
        };
        if item.get("type").and_then(Value::as_str) == Some("mcpToolCall") {
            panic!("unexpected extra tool call after turn completion: {item}");
        }
    }
}

#[tokio::test]
async fn live_parallel_tool_calls_web_search_and_web_fetch_single_turn() {
    if !live_enabled() {
        eprintln!("skipping live test; set HOMIE_LIVE_TESTS=1");
        return;
    }
    let _guard = lock_live_tests().lock().await;

    let config = match HomieConfig::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("skipping live test; homie config unavailable: {err}");
            return;
        }
    };
    let search_provider = match search_provider_ready(&config) {
        Ok(provider) => provider,
        Err(reason) => {
            eprintln!("skipping live test; {reason}");
            return;
        }
    };
    if let Err(reason) = fetch_backend_ready(&config) {
        eprintln!("skipping live test; {reason}");
        return;
    }
    let Some(model) = pick_model(&config).await else {
        eprintln!("skipping live test; no OPENAI_API_KEY or Codex auth");
        return;
    };

    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;
    let chat_id = start_chat(&mut ws).await;

    let send_result = rpc(
        &mut ws,
        "chat.message.send",
        Some(json!({
            "chat_id": chat_id,
            "message": "In one step, call exactly once each: 1) web_search query 'OpenAI' count 3, 2) web_fetch url 'https://example.com' with extractMode 'text'. Then provide a short final answer and stop.",
            "model": model,
            "approval_policy": "always"
        })),
    )
    .await;
    let turn_id = send_result["turn_id"]
        .as_str()
        .expect("turn_id")
        .to_string();

    let mut started_search = false;
    let mut started_fetch = false;
    let mut completed_search = false;
    let mut completed_fetch = false;
    let mut search_result: Option<Value> = None;
    let mut fetch_result: Option<Value> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(150);

    while tokio::time::Instant::now() < deadline {
        let msg = next_msg(&mut ws).await;
        let WsMsg::Text(text) = msg else { continue };
        let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
        let homie_protocol::Message::Event(event) = parsed else {
            continue;
        };
        let Some(params) = event.params else { continue };
        if params.get("turnId").and_then(Value::as_str) != Some(turn_id.as_str()) {
            continue;
        }

        match event.topic.as_str() {
            "chat.item.started" => {
                let Some(item) = params.get("item") else {
                    continue;
                };
                if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
                    continue;
                }
                match item.get("tool").and_then(Value::as_str) {
                    Some("web_search") => started_search = true,
                    Some("web_fetch") => started_fetch = true,
                    _ => {}
                }
            }
            "chat.item.completed" => {
                let Some(item) = params.get("item") else {
                    continue;
                };
                if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
                    continue;
                }
                if item.get("status").and_then(Value::as_str) != Some("completed") {
                    continue;
                }
                match item.get("tool").and_then(Value::as_str) {
                    Some("web_search") => {
                        completed_search = true;
                        search_result = item.get("result").cloned();
                    }
                    Some("web_fetch") => {
                        completed_fetch = true;
                        fetch_result = item.get("result").cloned();
                    }
                    _ => {}
                }
            }
            "chat.turn.completed" => {
                if params.get("status").and_then(Value::as_str) == Some("completed") {
                    break;
                }
            }
            _ => {}
        }
    }

    assert!(started_search, "expected web_search tool call to start");
    assert!(started_fetch, "expected web_fetch tool call to start");
    assert!(
        completed_search,
        "expected web_search tool call to complete"
    );
    assert!(completed_fetch, "expected web_fetch tool call to complete");

    let search_payload = search_result.expect("web_search result payload");
    assert_eq!(search_payload["ok"].as_bool(), Some(true));
    assert_eq!(search_payload["tool"].as_str(), Some("web_search"));
    assert_eq!(
        search_payload["data"]["provider"].as_str(),
        Some(search_provider.as_str())
    );
    assert!(search_payload["data"]["results"].is_array());

    let fetch_payload = fetch_result.expect("web_fetch result payload");
    assert_eq!(fetch_payload["ok"].as_bool(), Some(true));
    assert_eq!(fetch_payload["tool"].as_str(), Some("web_fetch"));
    let text = fetch_payload["data"]["text"]
        .as_str()
        .expect("web_fetch result text");
    assert!(!text.trim().is_empty());

    // After turn completion, no additional tool calls should start for this turn.
    let quiet_until = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < quiet_until {
        let timeout_left = quiet_until.saturating_duration_since(tokio::time::Instant::now());
        let msg = tokio::time::timeout(timeout_left, next_msg(&mut ws)).await;
        let Ok(WsMsg::Text(text)) = msg else { break };
        let parsed: homie_protocol::Message = serde_json::from_str(&text).unwrap();
        let homie_protocol::Message::Event(event) = parsed else {
            continue;
        };
        if event.topic != "chat.item.started" {
            continue;
        }
        let Some(params) = event.params else { continue };
        if params.get("turnId").and_then(Value::as_str) != Some(turn_id.as_str()) {
            continue;
        }
        let Some(item) = params.get("item") else {
            continue;
        };
        if item.get("type").and_then(Value::as_str) == Some("mcpToolCall") {
            panic!("unexpected extra tool call after turn completion: {item}");
        }
    }
}

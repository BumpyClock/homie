use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration, Instant};

use super::reader::dispatch_line;
use super::*;

#[test]
fn dispatch_line_routes_response_to_pending_waiter() {
    let (tx, mut rx) = oneshot::channel();
    let mut pending = HashMap::new();
    pending.insert(42u64, tx);

    let (event_tx, _event_rx) = mpsc::channel(16);
    let line = r#"{"id":42,"result":{"ok":true}}"#;
    dispatch_line(line, &mut pending, &event_tx);

    let result = rx.try_recv().expect("should receive response");
    assert_eq!(result, serde_json::json!({"ok": true}));
    assert!(pending.is_empty());
}

#[test]
fn dispatch_line_routes_notification_to_event_channel() {
    let mut pending = HashMap::new();
    let (event_tx, mut event_rx) = mpsc::channel(16);

    let line = r#"{"method":"turn/started","params":{"threadId":"t1"}}"#;
    dispatch_line(line, &mut pending, &event_tx);

    let event = event_rx.try_recv().expect("should receive event");
    assert_eq!(event.method, "turn/started");
    assert!(event.id.is_none());
    assert!(event.params.is_some());
}

#[test]
fn dispatch_line_routes_request_from_codex_to_event_channel() {
    let mut pending = HashMap::new();
    let (event_tx, mut event_rx) = mpsc::channel(16);

    let line = r#"{"method":"item/commandExecution/requestApproval","id":7,"params":{"command":"rm -rf /"}}"#;
    dispatch_line(line, &mut pending, &event_tx);

    let event = event_rx
        .try_recv()
        .expect("should receive approval request");
    assert_eq!(event.method, "item/commandExecution/requestApproval");
    assert_eq!(event.id, Some(CodexRequestId::Number(7)));
}

#[test]
fn dispatch_line_ignores_empty_and_whitespace() {
    let mut pending = HashMap::new();
    let (event_tx, mut event_rx) = mpsc::channel(16);

    dispatch_line("", &mut pending, &event_tx);
    dispatch_line("   \n", &mut pending, &event_tx);

    assert!(event_rx.try_recv().is_err());
}

#[test]
fn dispatch_line_handles_malformed_json() {
    let mut pending = HashMap::new();
    let (event_tx, mut event_rx) = mpsc::channel(16);

    dispatch_line("not json at all", &mut pending, &event_tx);

    assert!(event_rx.try_recv().is_err());
}

#[test]
fn dispatch_line_response_without_waiter_does_not_panic() {
    let mut pending: HashMap<u64, oneshot::Sender<Value>> = HashMap::new();
    let (event_tx, _event_rx) = mpsc::channel(16);

    let line = r#"{"id":999,"result":"orphan"}"#;
    dispatch_line(line, &mut pending, &event_tx);
}

#[tokio::test]
async fn codex_app_server_smoke() {
    if !should_run_codex_e2e() {
        eprintln!("skipping codex e2e; set HOMIE_CODEX_E2E=1 to run");
        return;
    }

    let (process, mut event_rx) = CodexProcess::spawn().await.expect("spawn codex app-server");

    with_timeout(Duration::from_secs(15), process.initialize())
        .await
        .expect("initialize codex");

    let account = with_timeout(
        Duration::from_secs(10),
        process.send_request("account/read", Some(serde_json::json!({}))),
    )
    .await
    .expect("account/read");
    assert!(account.is_object(), "account/read returns an object");

    let thread_params = serde_json::json!({ "model": codex_model() });
    let thread = with_timeout(
        Duration::from_secs(10),
        process.send_request("thread/start", Some(thread_params)),
    )
    .await
    .expect("thread/start");
    let thread_id = extract_id(&thread, &["threadId", "thread_id"], &[("thread", "id")])
        .unwrap_or_else(|| panic!("thread/start returned no thread id: {thread}"));

    let params = serde_json::json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": "Reply with the single word ping."}],
    });
    let turn = with_timeout(
        Duration::from_secs(10),
        process.send_request("turn/start", Some(params)),
    )
    .await
    .expect("turn/start");
    let turn_id = extract_id(&turn, &["turnId", "turn_id"], &[("turn", "id")])
        .unwrap_or_else(|| panic!("turn/start returned no turn id: {turn}"));

    let deadline = Instant::now() + Duration::from_secs(90);
    let mut saw_delta = false;
    let mut saw_completed = false;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let next = timeout(remaining, event_rx.recv())
            .await
            .expect("event recv timeout");
        let Some(event) = next else { break };

        if event.method == "item/agentMessage/delta" {
            saw_delta = true;
        }
        if event.method == "turn/completed" {
            if let Some(params) = event.params.as_ref() {
                if event_thread_id(params).as_deref() == Some(&thread_id)
                    || event_turn_id(params).as_deref() == Some(&turn_id)
                {
                    saw_completed = true;
                    break;
                }
            }
        }
    }

    assert!(
        saw_delta || saw_completed,
        "expected at least one delta or completed event"
    );
}

fn should_run_codex_e2e() -> bool {
    matches!(std::env::var("HOMIE_CODEX_E2E").as_deref(), Ok("1"))
}

fn codex_model() -> String {
    std::env::var("HOMIE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.1-codex".to_string())
}

async fn with_timeout<T>(
    dur: Duration,
    fut: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    match timeout(dur, fut).await {
        Ok(res) => res,
        Err(_) => Err("timeout waiting for codex app-server".to_string()),
    }
}

fn event_thread_id(params: &Value) -> Option<String> {
    params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn event_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_id(value: &Value, direct_keys: &[&str], nested_keys: &[(&str, &str)]) -> Option<String> {
    for key in direct_keys {
        if let Some(id) = value.get(*key).and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    for (outer, inner) in nested_keys {
        if let Some(id) = value
            .get(*outer)
            .and_then(|v| v.get(*inner))
            .and_then(|v| v.as_str())
        {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

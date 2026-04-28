use homie_core::{HomieConfig, ServerConfig};
use serde_json::Value;

use super::setup::*;

#[tokio::test]
async fn live_tool_call_path_ls() {
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
    if !core_tool_enabled(&config, "ls") {
        eprintln!("skipping live test; core provider disables ls");
        return;
    }
    let Some(model) = pick_model(&config).await else {
        eprintln!("skipping live test; no OPENAI_API_KEY or Codex auth");
        return;
    };

    let addr = start_server(ServerConfig::default()).await;
    let mut ws = connect_and_handshake(addr).await;
    let chat_id = start_chat(&mut ws).await;
    let result = send_and_wait_for_tool(
        &mut ws,
        &chat_id,
        &model,
        "Use the ls tool once on the current directory and then stop.",
        "ls",
    )
    .await;

    let entries = result
        .get("entries")
        .and_then(Value::as_array)
        .expect("ls result entries array");
    assert!(
        !entries.is_empty(),
        "expected ls to return at least one entry"
    );
}

use homie_core::{HomieConfig, ServerConfig};

use super::setup::*;

#[tokio::test]
async fn live_web_fetch_uses_configured_backend() {
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
    let result = send_and_wait_for_tool(
        &mut ws,
        &chat_id,
        &model,
        "Use web_fetch exactly once for url https://example.com with extractMode text and then stop.",
        "web_fetch",
    )
    .await;

    assert_eq!(
        result["ok"].as_bool(),
        Some(true),
        "web_fetch should succeed"
    );
    assert_eq!(result["tool"].as_str(), Some("web_fetch"));
    let text = result["data"]["text"]
        .as_str()
        .expect("web_fetch result text");
    assert!(
        !text.trim().is_empty(),
        "web_fetch should return non-empty content"
    );
}

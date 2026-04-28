use homie_core::{HomieConfig, ServerConfig};

use super::setup::*;

#[tokio::test]
async fn live_web_search_uses_configured_provider() {
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
    let provider = match search_provider_ready(&config) {
        Ok(provider) => provider,
        Err(reason) => {
            eprintln!("skipping live test; {reason}");
            return;
        }
    };
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
        "Use web_search exactly once with query 'OpenAI' and count 3, then stop.",
        "web_search",
    )
    .await;

    assert_eq!(
        result["ok"].as_bool(),
        Some(true),
        "web_search should succeed"
    );
    assert_eq!(result["tool"].as_str(), Some("web_search"));
    assert_eq!(
        result["data"]["provider"].as_str(),
        Some(provider.as_str()),
        "web_search provider mismatch"
    );
    assert!(
        result["data"]["results"].is_array(),
        "web_search should return results array"
    );
}

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use homie_core::HomieConfig;
use roci::auth::{providers::openai_codex::OpenAiCodexAuth, FileTokenStore, TokenStoreConfig};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::provider::{create_provider, ProviderRequest};
use roci::types::{generation::GenerationSettings, message::ModelMessage};
use tokio::time::timeout;

fn live_enabled() -> bool {
    matches!(std::env::var("HOMIE_LIVE_TESTS").as_deref(), Ok("1"))
}

#[tokio::test]
async fn live_openai_codex_generate_text() {
    if !live_enabled() {
        eprintln!("skipping live test; set HOMIE_LIVE_TESTS=1");
        return;
    }

    let homie = HomieConfig::load().expect("load homie config");
    let creds_dir = homie.credentials_dir().expect("credentials dir");
    let store = FileTokenStore::new(TokenStoreConfig::new(creds_dir));
    let auth = OpenAiCodexAuth::new(Arc::new(store.clone()));

    // Import CLI token if needed.
    let _ = auth.import_codex_auth_json(None);

    let token = auth.get_token().await.expect("codex token");

    let config = RociConfig::from_env();
    config.set_api_key("openai-codex", token.access_token);
    if let Some(account_id) = token.account_id {
        config.set_account_id("openai-codex", account_id);
    }
    if config.get_base_url("openai-codex").is_none() {
        if let Some(base) = config.get_base_url("openai") {
            config.set_base_url("openai-codex", base);
        }
    }

    let model: LanguageModel = "openai-codex:gpt-5.2-codex".parse().expect("model parse");
    let provider = create_provider(&model, &config).expect("provider");

    let request = ProviderRequest {
        messages: vec![ModelMessage::user("Say 'ok' then stop.")],
        settings: GenerationSettings::default(),
        tools: None,
        response_format: None,
    };

    let mut stream = timeout(Duration::from_secs(60), provider.stream_text(&request))
        .await
        .expect("timeout")
        .expect("provider stream");

    let mut text = String::new();
    let mut saw_tool_call = false;
    while let Some(delta) = stream.next().await {
        let delta = delta.expect("stream delta");
        if !delta.text.is_empty() {
            text.push_str(&delta.text);
        }
        if delta.tool_call.is_some() {
            saw_tool_call = true;
        }
    }

    let has_text = !text.trim().is_empty();
    assert!(has_text || saw_tool_call, "expected text or tool calls");
}

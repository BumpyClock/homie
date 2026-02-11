use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use homie_core::HomieConfig;
use roci::auth::{providers::github_copilot::GitHubCopilotAuth, FileTokenStore, TokenStoreConfig};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::provider::{create_provider, ProviderRequest};
use roci::types::{generation::GenerationSettings, message::ModelMessage};
use tokio::time::timeout;

fn live_enabled() -> bool {
    matches!(std::env::var("HOMIE_LIVE_TESTS").as_deref(), Ok("1"))
}

#[tokio::test]
async fn live_github_copilot_generate_text() {
    if !live_enabled() {
        eprintln!("skipping live test; set HOMIE_LIVE_TESTS=1");
        return;
    }

    let homie = HomieConfig::load().expect("load homie config");
    if !homie.providers.github_copilot.enabled {
        eprintln!("skipping live test; github-copilot provider disabled");
        return;
    }

    let creds_dir = homie.credentials_dir().expect("credentials dir");
    let store = FileTokenStore::new(TokenStoreConfig::new(creds_dir));
    let mut auth = GitHubCopilotAuth::new(Arc::new(store.clone()));
    let token_url = homie.providers.github_copilot.copilot_token_url.trim();
    if !token_url.is_empty() {
        auth = auth.with_copilot_token_url(token_url.to_string());
    }

    let token = auth
        .exchange_copilot_token()
        .await
        .expect("copilot token exchange");

    let config = RociConfig::from_env();
    config.set_api_key("github-copilot", token.token);
    config.set_base_url("github-copilot", token.base_url);

    let model_id = std::env::var("HOMIE_GITHUB_COPILOT_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gpt-4.1".to_string());
    let model: LanguageModel = format!("github-copilot:{model_id}")
        .parse()
        .expect("model parse");
    let provider = create_provider(&model, &config).expect("provider");

    let request = ProviderRequest {
        messages: vec![ModelMessage::user("Reply with exactly: ok")],
        settings: GenerationSettings::default(),
        tools: None,
        response_format: None,
    };

    let mut stream = timeout(Duration::from_secs(60), provider.stream_text(&request))
        .await
        .expect("timeout")
        .expect("provider stream");

    let mut text = String::new();
    while let Some(delta) = stream.next().await {
        let delta = delta.expect("stream delta");
        text.push_str(&delta.text);
    }

    assert!(!text.trim().is_empty(), "expected non-empty response text");
}

use std::sync::Arc;

use roci::auth::{FileTokenStore, TokenStore, TokenStoreConfig};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use roci::roci_providers::auth::{
    claude_code::ClaudeCodeAuth, github_copilot::GitHubCopilotAuth, openai_codex::OpenAiCodexAuth,
};

use crate::homie_config::ProvidersConfig;

use super::super::core::CodexChatCore;

impl CodexChatCore {
    pub(crate) fn roci_token_store(&self) -> Result<FileTokenStore, String> {
        let base = self.homie_config.credentials_dir()?;
        Ok(FileTokenStore::new(TokenStoreConfig::new(base)))
    }

    pub(crate) fn provider_enabled(&self, provider_id: &str) -> bool {
        let cfg = &self.homie_config.providers;
        match provider_id {
            "openai-codex" => cfg.openai_codex.enabled,
            "github-copilot" => cfg.github_copilot.enabled,
            "claude-code" => cfg.claude_code.enabled,
            _ => false,
        }
    }

    pub(crate) fn openai_codex_auth(
        &self,
        store: FileTokenStore,
        profile: &str,
    ) -> OpenAiCodexAuth {
        let mut auth = OpenAiCodexAuth::new(Arc::new(store)).with_profile(profile);
        let cfg = &self.homie_config.providers.openai_codex;
        if !cfg.issuer.trim().is_empty() {
            auth = auth.with_issuer(cfg.issuer.clone());
        }
        if !cfg.refresh_token_url_override.trim().is_empty() {
            auth = auth.with_refresh_token_url_override(cfg.refresh_token_url_override.clone());
        }
        auth
    }

    pub(crate) fn github_copilot_auth(
        &self,
        store: FileTokenStore,
        profile: &str,
    ) -> GitHubCopilotAuth {
        let mut auth = GitHubCopilotAuth::new(Arc::new(store)).with_profile(profile);
        let cfg = &self.homie_config.providers.github_copilot;
        if !cfg.device_code_url.trim().is_empty() {
            auth = auth.with_device_code_url(cfg.device_code_url.clone());
        }
        if !cfg.token_url.trim().is_empty() {
            auth = auth.with_access_token_url(cfg.token_url.clone());
        }
        if !cfg.copilot_token_url.trim().is_empty() {
            auth = auth.with_copilot_token_url(cfg.copilot_token_url.clone());
        }
        auth
    }

    pub(crate) fn claude_code_auth(&self, store: FileTokenStore, profile: &str) -> ClaudeCodeAuth {
        ClaudeCodeAuth::new(Arc::new(store)).with_profile(profile)
    }

    pub(crate) async fn roci_config_for_model(
        &self,
        model: &LanguageModel,
    ) -> Result<RociConfig, String> {
        let store = self.roci_token_store()?;
        let config = RociConfig::from_env().with_token_store(Some(Arc::new(store.clone())));
        let cfg = &self.homie_config.providers;
        if cfg.openai_codex.enabled {
            self.import_codex_cli_credentials(&store);
        }
        if cfg.claude_code.enabled && cfg.claude_code.import_from_cli {
            self.import_claude_cli_credentials(&store);
        }
        if cfg.openai_compatible.enabled {
            if config.get_base_url("openai-compatible").is_none()
                && !cfg.openai_compatible.base_url.trim().is_empty()
            {
                config.set_base_url(
                    "openai-compatible",
                    cfg.openai_compatible.base_url.trim().to_string(),
                );
            }
            if config.get_api_key("openai-compatible").is_none()
                && !cfg.openai_compatible.api_key.trim().is_empty()
            {
                config.set_api_key(
                    "openai-compatible",
                    cfg.openai_compatible.api_key.trim().to_string(),
                );
            }
        }

        match model.provider_name() {
            "openai" => {
                if config.get_api_key("openai").is_none() {
                    return Err("Missing OPENAI_API_KEY. Codex OAuth is available; use openai-codex/* models or set OPENAI_API_KEY.".to_string());
                }
            }
            "codex" | "openai-codex" => {
                if cfg.openai_codex.enabled {
                    let auth = self.openai_codex_auth(store.clone(), "default");
                    if let Ok(token) = auth.get_token().await {
                        if config.get_api_key("openai-codex").is_none() {
                            config.set_api_key("openai-codex", token.access_token);
                        }
                        if let Some(account_id) = token.account_id {
                            config.set_account_id("openai-codex", account_id);
                            if super::super::models::debug_enabled() {
                                tracing::debug!("openai-codex account_id set");
                            }
                        }
                        if config.get_base_url("openai-codex").is_none() {
                            if let Some(base) = config.get_base_url("openai") {
                                config.set_base_url("openai-codex", base);
                            }
                        }
                    }
                }
            }
            "github-copilot" => {
                if cfg.github_copilot.enabled && config.get_api_key("github-copilot").is_none() {
                    let auth = self.github_copilot_auth(store.clone(), "default");
                    if let Ok(token) = auth.exchange_copilot_token().await {
                        config.set_api_key("github-copilot", token.token.clone());
                        if config.get_base_url("github-copilot").is_none() {
                            config.set_base_url("github-copilot", token.base_url.clone());
                        }
                        if config.get_api_key("openai-compatible").is_none() {
                            config.set_api_key("openai-compatible", token.token.clone());
                        }
                        if config.get_base_url("openai-compatible").is_none() {
                            config.set_base_url("openai-compatible", token.base_url);
                        }
                    }
                }
            }
            "openai-compatible" => {
                if cfg.github_copilot.enabled && config.get_api_key("openai-compatible").is_none() {
                    let auth = self.github_copilot_auth(store.clone(), "default");
                    if let Ok(token) = auth.exchange_copilot_token().await {
                        config.set_api_key("openai-compatible", token.token.clone());
                        if config.get_base_url("openai-compatible").is_none() {
                            config.set_base_url("openai-compatible", token.base_url);
                        }
                    }
                }
            }
            "anthropic" => {
                if cfg.claude_code.enabled && config.get_api_key("anthropic").is_none() {
                    let auth = self.claude_code_auth(store.clone(), "default");
                    if let Ok(token) = auth.get_token().await {
                        config.set_api_key("anthropic", token.access_token);
                    }
                }
            }
            _ => {}
        }

        Ok(config)
    }

    pub(crate) fn import_enabled_provider_credentials(
        &self,
        cfg: &ProvidersConfig,
        store: &FileTokenStore,
    ) {
        if cfg.openai_codex.enabled {
            self.import_codex_cli_credentials(store);
        }
        if cfg.claude_code.enabled && cfg.claude_code.import_from_cli {
            self.import_claude_cli_credentials(store);
        }
    }

    pub(crate) fn import_codex_cli_credentials(&self, store: &FileTokenStore) {
        let existing = match store.load("openai-codex", "default") {
            Ok(token) => token,
            Err(err) => {
                tracing::warn!(error = %err, "codex token load failed");
                return;
            }
        };
        if existing.is_some() {
            return;
        }
        let auth = OpenAiCodexAuth::new(Arc::new(store.clone()));
        match auth.import_codex_auth_json(None) {
            Ok(Some(_)) => {
                tracing::info!("imported codex cli credentials");
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, "codex cli credential import failed");
            }
        }
    }

    pub(crate) fn import_claude_cli_credentials(&self, store: &FileTokenStore) {
        let existing = match store.load("claude-code", "default") {
            Ok(token) => token,
            Err(err) => {
                tracing::warn!(error = %err, "claude token load failed");
                return;
            }
        };
        if existing.is_some() {
            return;
        }
        let auth = ClaudeCodeAuth::new(Arc::new(store.clone()));
        match auth.import_cli_credentials(None) {
            Ok(Some(_)) => {
                tracing::info!("imported claude cli credentials");
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, "claude cli credential import failed");
            }
        }
    }
}

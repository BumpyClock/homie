use homie_protocol::{error_codes, Response};
use roci::auth::{
    providers::claude_code::ClaudeCodeAuth, providers::github_copilot::GitHubCopilotAuth,
    providers::openai_codex::OpenAiCodexAuth, FileTokenStore, TokenStore, TokenStoreConfig,
};
use roci::config::RociConfig;
use roci::models::LanguageModel;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::homie_config::ProvidersConfig;
use crate::agent::service::core::CodexChatCore;

use super::params::{
    device_code_poll_json,
    device_code_session_json,
    parse_account_provider_params,
    parse_device_code_session,
};

impl CodexChatCore {
    pub(super) async fn chat_account_list(&mut self, req_id: Uuid) -> Response {
        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list init failed: {e}"),
                )
            }
        };

        self.import_enabled_provider_credentials(&self.homie_config.providers, &store);
        match self.account_provider_statuses(&store) {
            Ok(providers) => Response::success(req_id, json!({ "providers": providers })),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }

    pub(super) async fn chat_account_login_start(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (provider_id, profile, _param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login start failed: {e}"),
                )
            }
        };

        match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "claude-code" => Response::error(
                req_id,
                error_codes::INVALID_PARAMS,
                "claude-code does not support device-code login",
            ),
            _ => Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider"),
        }
    }

    pub(super) async fn chat_account_login_poll(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (provider_id, profile, param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }
        let session = match parse_device_code_session(&param_map, &provider_id) {
            Some(session) => session,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing session"),
        };

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login poll failed: {e}"),
                )
            }
        };

        let poll = match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "claude-code" => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "claude-code does not support device-code login",
                )
            }
            _ => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider")
            }
        };

        match poll {
            Ok(result) => Response::success(req_id, device_code_poll_json(result)),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("device code poll failed: {e}"),
            ),
        }
    }

    pub(super) async fn chat_account_read(&mut self, req_id: Uuid) -> Response {
        if self.use_roci() {
            let store = match self.roci_token_store() {
                Ok(store) => store,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read init failed: {e}"),
                    )
                }
            };
            self.import_enabled_provider_credentials(&self.homie_config.providers, &store);
            let providers = match self.account_provider_statuses(&store) {
                Ok(providers) => providers,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read failed: {e}"),
                    )
                }
            };
            let any_logged_in = providers.iter().any(|entry| {
                entry
                    .get("logged_in")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            });
            let account = if any_logged_in {
                json!({ "providers": providers })
            } else {
                Value::Null
            };
            return Response::success(
                req_id,
                json!({
                    "account": account,
                    "providers": providers,
                    "requires_openai_auth": !any_logged_in,
                }),
            );
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("account/read", Some(json!({}))).await {
            Ok(result) => {
                tracing::info!(result = %result, "codex account/read");
                Response::success(req_id, result)
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("account/read failed: {e}"),
            ),
        }
    }

    pub(super) fn roci_token_store(&self) -> Result<FileTokenStore, String> {
        let base = self.homie_config.credentials_dir()?;
        Ok(FileTokenStore::new(TokenStoreConfig::new(base)))
    }

    pub(super) fn provider_enabled(&self, provider_id: &str) -> bool {
        let cfg = &self.homie_config.providers;
        match provider_id {
            "openai-codex" => cfg.openai_codex.enabled,
            "github-copilot" => cfg.github_copilot.enabled,
            "claude-code" => cfg.claude_code.enabled,
            _ => false,
        }
    }

    pub(super) fn openai_codex_auth(&self, store: FileTokenStore, profile: &str) -> OpenAiCodexAuth {
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

    pub(super) fn github_copilot_auth(&self, store: FileTokenStore, profile: &str) -> GitHubCopilotAuth {
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

    pub(super) fn claude_code_auth(&self, store: FileTokenStore, profile: &str) -> ClaudeCodeAuth {
        ClaudeCodeAuth::new(Arc::new(store)).with_profile(profile)
    }

    pub(super) async fn roci_config_for_model(
        &self,
        model: &LanguageModel,
    ) -> Result<RociConfig, String> {
        let config = RociConfig::from_env();
        let store = self.roci_token_store()?;
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
                config.set_api_key("openai-compatible", cfg.openai_compatible.api_key.trim().to_string());
            }
        }

        match model.provider_name() {
            "openai" => {
                if config.get_api_key("openai").is_none() {
                    return Err("Missing OPENAI_API_KEY. Codex OAuth is available; use openai-codex/* models or set OPENAI_API_KEY.".to_string());
                }
            }
            "openai-codex" => {
                if cfg.openai_codex.enabled {
                    let auth = self.openai_codex_auth(store.clone(), "default");
                    if let Ok(token) = auth.get_token().await {
                        if config.get_api_key("openai-codex").is_none() {
                            config.set_api_key("openai-codex", token.access_token);
                        }
                        if let Some(account_id) = token.account_id {
                            config.set_account_id("openai-codex", account_id);
                            if super::models::debug_enabled() {
                                tracing::debug!(
                                    "openai-codex account_id set"
                                );
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

    pub(super) fn import_enabled_provider_credentials(
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

    pub(super) fn import_codex_cli_credentials(&self, store: &FileTokenStore) {
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

    pub(super) fn import_claude_cli_credentials(&self, store: &FileTokenStore) {
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

    pub(super) fn build_provider_status(
        &self,
        store: &FileTokenStore,
        provider_id: &str,
        provider_key: &str,
        enabled: bool,
    ) -> Result<Value, String> {
        let mut map = serde_json::Map::new();
        map.insert("id".into(), json!(provider_id));
        map.insert("key".into(), json!(provider_key));
        map.insert("enabled".into(), json!(enabled));
        if !enabled {
            map.insert("logged_in".into(), json!(false));
            return Ok(Value::Object(map));
        }
        let token = store
            .load(provider_id, "default")
            .map_err(|e| format!("load {provider_id} token: {e}"))?;
        map.insert("logged_in".into(), json!(token.is_some()));
        if let Some(token) = token {
            if let Some(expires_at) = token.expires_at {
                map.insert("expires_at".into(), json!(expires_at.to_rfc3339()));
            }
            if let Some(scopes) = token.scopes {
                map.insert("scopes".into(), json!(scopes));
            }
            map.insert("has_refresh_token".into(), json!(token.refresh_token.is_some()));
        }
        Ok(Value::Object(map))
    }

    pub(super) fn account_provider_statuses(
        &self,
        store: &FileTokenStore,
    ) -> Result<Vec<Value>, String> {
        let cfg = &self.homie_config.providers;
        let mut providers = Vec::new();
        let openai = self.build_provider_status(
            store,
            "openai-codex",
            "openai_codex",
            cfg.openai_codex.enabled,
        )?;
        let github = self.build_provider_status(
            store,
            "github-copilot",
            "github_copilot",
            cfg.github_copilot.enabled,
        )?;
        let claude = self.build_provider_status(
            store,
            "claude-code",
            "claude_code",
            cfg.claude_code.enabled,
        )?;
        providers.push(openai);
        providers.push(github);
        providers.push(claude);
        Ok(providers)
    }
}

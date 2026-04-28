use roci::auth::{FileTokenStore, TokenStore};
use serde_json::{json, Value};

use super::super::core::CodexChatCore;

impl CodexChatCore {
    pub(crate) fn build_provider_status(
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
            map.insert(
                "has_refresh_token".into(),
                json!(token.refresh_token.is_some()),
            );
        }
        Ok(Value::Object(map))
    }

    pub(crate) fn account_provider_statuses(
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

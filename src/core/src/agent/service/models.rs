use std::collections::HashSet;
use std::time::Duration;

use roci::auth::providers::github_copilot::GitHubCopilotAuth;
use roci::config::RociConfig;
use serde_json::{json, Value};

use crate::homie_config::{OpenAiCompatibleProviderConfig, ProvidersConfig};

pub(super) const COPILOT_FALLBACK_MODELS: &[&str] = &[
    "gpt-4.1",
    "gpt-5",
    "gpt-5-mini",
    "gpt-5.3-codex",
    "gpt-5-codex",
    "gpt-5.1",
    "gpt-5.1-codex",
    "gpt-5.1-codex-mini",
    "gpt-5.1-codex-max",
    "gpt-5.2",
    "gpt-5.2-codex",
    "claude-haiku-4.5",
    "claude-opus-4.1",
    "claude-opus-4.5",
    "claude-opus-4.6",
    "claude-opus-4.6-fast",
    "claude-sonnet-4",
    "claude-sonnet-4.5",
    "gemini-2.5-pro",
    "gemini-3-flash",
    "gemini-3-pro",
    "grok-code-fast-1",
    "raptor-mini",
];

pub(super) fn debug_enabled() -> bool {
    matches!(
        std::env::var("HOMIE_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    ) || matches!(
        std::env::var("HOME_DEBUG").as_deref(),
        Ok("1" | "true" | "TRUE")
    )
}

pub(super) fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

pub(super) fn codex_model() -> String {
    std::env::var("HOMIE_CODEX_MODEL").unwrap_or_else(|_| "gpt-5.1-codex".to_string())
}

pub(super) fn extract_id_from_result(
    value: &Value,
    direct_keys: &[&str],
    nested_keys: &[(&str, &str)],
) -> Option<String> {
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

pub(super) fn is_known_copilot_model(model_id: &str) -> bool {
    COPILOT_FALLBACK_MODELS
        .iter()
        .any(|known| *known == model_id.trim())
}

pub(super) fn parse_openai_compat_models_csv(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn append_openai_compatible_models(models: &mut Vec<Value>, compat_models: Vec<String>) {
    if compat_models.is_empty() {
        return;
    }

    let mut seen = HashSet::new();
    let mut has_default = false;
    for entry in models.iter() {
        if let Some(model_id) = entry.get("model").and_then(|value| value.as_str()) {
            seen.insert(model_id.to_string());
        }
        if entry
            .get("is_default")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            has_default = true;
        }
    }

    for model_id in compat_models {
        let selector = format!("openai-compatible:{model_id}");
        if !seen.insert(selector.clone()) {
            continue;
        }
        let is_default = !has_default;
        if is_default {
            has_default = true;
        }
        models.push(json!({
            "id": selector,
            "model": selector,
            "provider": "openai-compatible",
            "display_name": format!("{model_id} (Local)"),
            "is_default": is_default,
        }));
    }
}

pub(super) fn replace_github_copilot_models(models: &mut Vec<Value>, copilot_models: Vec<String>) {
    if copilot_models.is_empty() {
        return;
    }
    models.retain(|entry| {
        entry
            .get("provider")
            .and_then(|value| value.as_str())
            .map(|provider| provider != "github-copilot")
            .unwrap_or(true)
    });

    let mut seen = HashSet::new();
    let mut has_default = models.iter().any(|entry| {
        entry
            .get("is_default")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    });
    for model_id in copilot_models {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        let selector = format!("github-copilot:{trimmed}");
        let is_default = !has_default;
        if is_default {
            has_default = true;
        }
        models.push(json!({
            "id": selector,
            "model": selector,
            "provider": "github-copilot",
            "display_name": format!("{trimmed} (Copilot)"),
            "is_default": is_default,
        }));
    }
}

pub(super) async fn discover_github_copilot_models(
    auth: &GitHubCopilotAuth,
) -> Result<Vec<String>, String> {
    let token = auth
        .exchange_copilot_token()
        .await
        .map_err(|err| format!("exchange github-copilot token: {err}"))?;
    let endpoint = format!("{}/models", token.base_url.trim().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|err| format!("build github-copilot http client: {err}"))?;
    let response = client
        .get(endpoint)
        .bearer_auth(token.token)
        .send()
        .await
        .map_err(|err| format!("request github-copilot models: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "github-copilot models request returned status {status}"
        ));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|err| format!("decode github-copilot models payload: {err}"))?;
    let mut discovered = Vec::new();
    if let Some(items) = payload.get("data").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(model_id) = item.get("id").and_then(|value| value.as_str()) {
                let normalized = model_id.trim();
                if !normalized.is_empty() {
                    discovered.push(normalized.to_string());
                }
            }
        }
    }
    if discovered.is_empty() {
        return Err("github-copilot models response did not include model ids".to_string());
    }
    let mut unique = HashSet::new();
    discovered.retain(|value| unique.insert(value.clone()));
    Ok(discovered)
}

pub(super) async fn discover_openai_compatible_models(
    provider_cfg: &OpenAiCompatibleProviderConfig,
) -> Result<Vec<String>, String> {
    let mut fallback = if let Ok(value) = std::env::var("OPENAI_COMPAT_MODELS") {
        parse_openai_compat_models_csv(&value)
    } else {
        provider_cfg
            .models
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
    };
    if !fallback.is_empty() {
        let mut unique = HashSet::new();
        fallback.retain(|value| unique.insert(value.clone()));
    }

    let config = RociConfig::from_env();
    let base_url = if let Some(url) = config.get_base_url("openai-compatible") {
        url
    } else {
        provider_cfg.base_url.clone()
    };
    if base_url.trim().is_empty() {
        return Ok(fallback);
    }

    let endpoint = format!("{}/models", base_url.trim().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|err| format!("build openai-compatible http client: {err}"))?;

    let mut request = client.get(endpoint.as_str());
    let api_key = config
        .get_api_key("openai-compatible")
        .unwrap_or_else(|| provider_cfg.api_key.clone());
    if !api_key.trim().is_empty() {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request openai-compatible models: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        if fallback.is_empty() {
            return Err(format!(
                "openai-compatible models request returned status {status}"
            ));
        }
        tracing::warn!(
            "openai-compatible model discovery returned status {status}; using OPENAI_COMPAT_MODELS fallback"
        );
        return Ok(fallback);
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|err| format!("decode openai-compatible models payload: {err}"))?;
    let mut discovered = Vec::new();
    if let Some(items) = payload.get("data").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(model_id) = item.get("id").and_then(|value| value.as_str()) {
                let normalized = model_id.trim();
                if !normalized.is_empty() {
                    discovered.push(normalized.to_string());
                }
            }
        }
    }

    if discovered.is_empty() {
        if fallback.is_empty() {
            return Err("openai-compatible models response did not include model ids".to_string());
        }
        return Ok(fallback);
    }

    let mut unique = HashSet::new();
    discovered.retain(|value| unique.insert(value.clone()));
    Ok(discovered)
}

pub(super) fn roci_model_catalog(providers: &ProvidersConfig) -> Vec<Value> {
    let mut models = Vec::new();
    let mut default_set = false;
    let has_openai_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    if has_openai_key {
        let openai_models = [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4.1",
            "gpt-4.1-mini",
            "gpt-4.1-nano",
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5.2",
            "o1",
            "o1-mini",
            "o1-pro",
            "o3",
            "o3-mini",
            "o4-mini",
        ];
        for model_id in openai_models {
            let model = format!("openai:{model_id}");
            let is_default = !default_set && model_id == "gpt-4o-mini";
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "openai",
                "display_name": model_id,
                "is_default": is_default,
            }));
        }
    }

    if providers.openai_codex.enabled {
        let codex_models = [
            "gpt-5.3-codex",
            "gpt-5.2",
            "gpt-5.2-codex",
            "gpt-5-codex",
            "gpt-5.1-codex",
            "gpt-5.1-codex-mini",
            "gpt-5.1-codex-max",
        ];
        for (idx, model_id) in codex_models.iter().enumerate() {
            let model = format!("openai-codex:{model_id}");
            let is_default = !default_set && idx == 0;
            if is_default {
                default_set = true;
            }
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "openai-codex",
                "display_name": format!("{model_id} (Codex)"),
                "is_default": is_default,
            }));
        }
    }

    if providers.github_copilot.enabled {
        for model_id in COPILOT_FALLBACK_MODELS {
            let model = format!("github-copilot:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "github-copilot",
                "display_name": format!("{model_id} (Copilot)"),
                "is_default": false,
            }));
        }
    }

    if providers.claude_code.enabled {
        let claude_models = [
            "claude-sonnet-4-5-20250514",
            "claude-opus-4-5-20251101",
            "claude-sonnet-4-20250514",
            "claude-haiku-3-5-20241022",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
        ];
        for model_id in claude_models {
            let model = format!("anthropic:{model_id}");
            models.push(json!({
                "id": model,
                "model": model,
                "provider": "anthropic",
                "display_name": model_id,
                "is_default": false,
            }));
        }
    }

    models
}

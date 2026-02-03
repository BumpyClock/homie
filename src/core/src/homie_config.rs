use std::path::PathBuf;

use serde::Deserialize;

use crate::paths::{
    homie_config_path, homie_credentials_dir, homie_execpolicy_path, homie_home_dir, user_home_dir,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HomieConfig {
    pub version: u32,
    pub debug: DebugConfig,
    pub models: ModelsConfig,
    pub chat: ChatConfig,
    pub providers: ProvidersConfig,
    pub paths: PathsConfig,
}

impl Default for HomieConfig {
    fn default() -> Self {
        Self {
            version: 1,
            debug: DebugConfig::default(),
            models: ModelsConfig::default(),
            chat: ChatConfig::default(),
            providers: ProvidersConfig::default(),
            paths: PathsConfig::default(),
        }
    }
}

impl HomieConfig {
    pub fn load() -> Result<Self, String> {
        let path = homie_config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            std::fs::read_to_string(&path).map_err(|e| format!("read config.toml: {e}"))?;
        toml::from_str(&raw).map_err(|e| format!("parse config.toml: {e}"))
    }

    pub fn config_path() -> Result<PathBuf, String> {
        homie_config_path()
    }

    pub fn credentials_dir(&self) -> Result<PathBuf, String> {
        if let Some(path) = self.paths.credentials_dir.as_ref() {
            let resolved = resolve_path(path)?;
            std::fs::create_dir_all(&resolved)
                .map_err(|e| format!("create credentials dir: {e}"))?;
            return Ok(resolved);
        }
        homie_credentials_dir()
    }

    pub fn execpolicy_path(&self) -> Result<PathBuf, String> {
        if let Some(path) = self.paths.execpolicy_path.as_ref() {
            return resolve_path(path);
        }
        homie_execpolicy_path()
    }

    pub fn raw_events_enabled(&self) -> bool {
        if self.debug.persist_raw_provider_events {
            return true;
        }
        let homie_debug = std::env::var(&self.debug.homie_debug_env).ok();
        let home_debug = std::env::var(&self.debug.home_debug_env).ok();
        matches!(homie_debug.as_deref(), Some("1"))
            || matches!(home_debug.as_deref(), Some("1"))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DebugConfig {
    pub homie_debug_env: String,
    pub home_debug_env: String,
    pub persist_raw_provider_events: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            homie_debug_env: "HOMIE_DEBUG".to_string(),
            home_debug_env: "HOME_DEBUG".to_string(),
            persist_raw_provider_events: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub catalog_ttl_secs: u64,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            catalog_ttl_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    pub system_prompt: String,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are Homie, a helpful assistant for remote machine access.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub openai_codex: OpenAiCodexProviderConfig,
    pub github_copilot: GithubCopilotProviderConfig,
    pub claude_code: ClaudeCodeProviderConfig,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            openai_codex: OpenAiCodexProviderConfig::default(),
            github_copilot: GithubCopilotProviderConfig::default(),
            claude_code: ClaudeCodeProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OpenAiCodexProviderConfig {
    pub enabled: bool,
    pub issuer: String,
    pub refresh_token_url_override: String,
}

impl Default for OpenAiCodexProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            issuer: "https://auth.openai.com".to_string(),
            refresh_token_url_override: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GithubCopilotProviderConfig {
    pub enabled: bool,
    pub github_host: String,
    pub device_code_url: String,
    pub token_url: String,
    pub copilot_token_url: String,
}

impl Default for GithubCopilotProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            github_host: "github.com".to_string(),
            device_code_url: "https://github.com/login/device/code".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            copilot_token_url: "https://api.github.com/copilot_internal/v2/token".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ClaudeCodeProviderConfig {
    pub enabled: bool,
    pub import_from_cli: bool,
}

impl Default for ClaudeCodeProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            import_from_cli: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub credentials_dir: Option<String>,
    pub execpolicy_path: Option<String>,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            credentials_dir: None,
            execpolicy_path: None,
        }
    }
}

fn resolve_path(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("path override is empty".to_string());
    }
    let home = user_home_dir();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home {
            return Ok(home.join(rest));
        }
    }
    if trimmed == "~" {
        if let Some(home) = home {
            return Ok(home);
        }
    }
    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        let base = homie_home_dir()?;
        return Ok(base.join(path));
    }
    Ok(path)
}

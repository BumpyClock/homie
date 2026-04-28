use std::path::PathBuf;

use serde::Deserialize;

use crate::paths::{
    homie_config_path, homie_credentials_dir, homie_execpolicy_path, homie_system_prompt_path,
};

use super::{
    paths::{resolve_path, PathsConfig},
    providers::ProvidersConfig,
    tools::ToolsConfig,
    DEFAULT_SYSTEM_PROMPT,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HomieConfig {
    pub version: u32,
    pub debug: DebugConfig,
    pub models: ModelsConfig,
    pub chat: ChatConfig,
    pub tools: ToolsConfig,
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
            tools: ToolsConfig::default(),
            providers: ProvidersConfig::default(),
            paths: PathsConfig::default(),
        }
    }
}

impl HomieConfig {
    pub fn load() -> Result<Self, String> {
        let path = homie_config_path()?;
        if !path.exists() {
            let mut config = Self::default();
            config.ensure_system_prompt()?;
            return Ok(config);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| format!("read config.toml: {e}"))?;
        let mut config: Self =
            toml::from_str(&raw).map_err(|e| format!("parse config.toml: {e}"))?;
        config.ensure_system_prompt()?;
        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf, String> {
        homie_config_path()
    }

    pub fn credentials_dir(&self) -> Result<PathBuf, String> {
        if let Some(path) = self.paths.credentials_dir.as_ref() {
            if path.trim().is_empty() {
                return homie_credentials_dir();
            }
            let resolved = resolve_path(path)?;
            std::fs::create_dir_all(&resolved)
                .map_err(|e| format!("create credentials dir: {e}"))?;
            return Ok(resolved);
        }
        homie_credentials_dir()
    }

    pub fn execpolicy_path(&self) -> Result<PathBuf, String> {
        if let Some(path) = self.paths.execpolicy_path.as_ref() {
            if path.trim().is_empty() {
                return homie_execpolicy_path();
            }
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
        matches!(homie_debug.as_deref(), Some("1")) || matches!(home_debug.as_deref(), Some("1"))
    }

    fn ensure_system_prompt(&mut self) -> Result<(), String> {
        let path = if let Some(path) = self.chat.system_prompt_path.as_ref() {
            if path.trim().is_empty() {
                homie_system_prompt_path()?
            } else {
                resolve_path(path)?
            }
        } else {
            homie_system_prompt_path()?
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create system prompt dir: {e}"))?;
        }
        if path.exists() {
            let raw =
                std::fs::read_to_string(&path).map_err(|e| format!("read system prompt: {e}"))?;
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                self.chat.system_prompt = trimmed.to_string();
                return Ok(());
            }
        } else {
            std::fs::write(&path, DEFAULT_SYSTEM_PROMPT)
                .map_err(|e| format!("write system prompt: {e}"))?;
        }
        self.chat.system_prompt = DEFAULT_SYSTEM_PROMPT.to_string();
        Ok(())
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
    pub system_prompt_path: Option<String>,
    pub stream_idle_timeout_ms: Option<u64>,
    #[serde(skip)]
    pub system_prompt: String,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            system_prompt_path: None,
            stream_idle_timeout_ms: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.trim().to_string(),
        }
    }
}

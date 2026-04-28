use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub openai_codex: OpenAiCodexProviderConfig,
    pub github_copilot: GithubCopilotProviderConfig,
    pub openai_compatible: OpenAiCompatibleProviderConfig,
    pub claude_code: ClaudeCodeProviderConfig,
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
pub struct OpenAiCompatibleProviderConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
}

impl Default for OpenAiCompatibleProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: String::new(),
            api_key: String::new(),
            models: Vec::new(),
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

use std::collections::HashMap;
use std::path::PathBuf;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};

use crate::paths::{
    homie_config_path, homie_credentials_dir, homie_execpolicy_path, homie_home_dir,
    homie_system_prompt_path, user_home_dir,
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

const DEFAULT_SYSTEM_PROMPT: &str = include_str!("../system_prompt.md");

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub web: WebToolsConfig,
    pub providers: HashMap<String, ToolProviderConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ToolProviderConfig {
    pub enabled: Option<bool>,
    pub channels: Vec<String>,
    pub allow_tools: Vec<String>,
    pub deny_tools: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WebToolsConfig {
    pub fetch: WebFetchConfig,
    pub search: WebSearchConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebFetchBackend {
    Native,
    Firecrawl,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WebFetchConfig {
    #[serde(
        default = "default_web_fetch_enabled",
        deserialize_with = "deserialize_web_fetch_enabled"
    )]
    pub enabled: bool,
    #[serde(
        default = "default_web_fetch_max_chars",
        deserialize_with = "deserialize_web_fetch_max_chars"
    )]
    pub max_chars: usize,
    #[serde(
        default = "default_web_fetch_timeout_seconds",
        deserialize_with = "deserialize_web_fetch_timeout_seconds"
    )]
    pub timeout_seconds: u64,
    #[serde(
        default = "default_web_fetch_cache_ttl_minutes",
        deserialize_with = "deserialize_web_fetch_cache_ttl_minutes"
    )]
    pub cache_ttl_minutes: u64,
    #[serde(
        default = "default_web_fetch_max_redirects",
        deserialize_with = "deserialize_web_fetch_max_redirects"
    )]
    pub max_redirects: usize,
    #[serde(
        default = "default_web_fetch_user_agent",
        deserialize_with = "deserialize_web_fetch_user_agent"
    )]
    pub user_agent: String,
    #[serde(
        default = "default_web_fetch_readability",
        deserialize_with = "deserialize_web_fetch_readability"
    )]
    pub readability: bool,
    pub firecrawl: FirecrawlConfig,
    #[serde(default)]
    pub backend: WebFetchBackend,
}

impl Default for WebFetchConfig {
    fn default() -> Self {
        Self {
            enabled: default_web_fetch_enabled(),
            max_chars: default_web_fetch_max_chars(),
            timeout_seconds: default_web_fetch_timeout_seconds(),
            cache_ttl_minutes: default_web_fetch_cache_ttl_minutes(),
            max_redirects: default_web_fetch_max_redirects(),
            user_agent: default_web_fetch_user_agent(),
            readability: default_web_fetch_readability(),
            firecrawl: FirecrawlConfig::default(),
            backend: WebFetchBackend::Auto,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FirecrawlConfig {
    #[serde(
        default = "default_firecrawl_enabled",
        deserialize_with = "deserialize_firecrawl_enabled"
    )]
    pub enabled: bool,
    pub api_key: String,
    #[serde(
        default = "default_firecrawl_base_url",
        deserialize_with = "deserialize_firecrawl_base_url"
    )]
    pub base_url: String,
    #[serde(
        default = "default_firecrawl_only_main_content",
        deserialize_with = "deserialize_firecrawl_only_main_content"
    )]
    pub only_main_content: bool,
    #[serde(
        default = "default_firecrawl_max_age_ms",
        deserialize_with = "deserialize_firecrawl_max_age_ms"
    )]
    pub max_age_ms: u64,
    #[serde(
        default = "default_firecrawl_timeout_seconds",
        deserialize_with = "deserialize_firecrawl_timeout_seconds"
    )]
    pub timeout_seconds: u64,
}

impl Default for FirecrawlConfig {
    fn default() -> Self {
        Self {
            enabled: default_firecrawl_enabled(),
            api_key: String::new(),
            base_url: default_firecrawl_base_url(),
            only_main_content: default_firecrawl_only_main_content(),
            max_age_ms: default_firecrawl_max_age_ms(),
            timeout_seconds: default_firecrawl_timeout_seconds(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WebSearchConfig {
    #[serde(
        default = "default_web_search_enabled",
        deserialize_with = "deserialize_web_search_enabled"
    )]
    pub enabled: bool,
    #[serde(
        default = "default_web_search_provider",
        deserialize_with = "deserialize_web_search_provider"
    )]
    pub provider: String,
    #[serde(
        default = "default_web_search_timeout_seconds",
        deserialize_with = "deserialize_web_search_timeout_seconds"
    )]
    pub timeout_seconds: u64,
    #[serde(
        default = "default_web_search_cache_ttl_minutes",
        deserialize_with = "deserialize_web_search_cache_ttl_minutes"
    )]
    pub cache_ttl_minutes: u64,
    #[serde(
        default = "default_web_search_max_results",
        deserialize_with = "deserialize_web_search_max_results"
    )]
    pub max_results: usize,
    pub brave: BraveSearchConfig,
    pub searxng: SearxngSearchConfig,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            enabled: default_web_search_enabled(),
            provider: default_web_search_provider(),
            timeout_seconds: default_web_search_timeout_seconds(),
            cache_ttl_minutes: default_web_search_cache_ttl_minutes(),
            max_results: default_web_search_max_results(),
            brave: BraveSearchConfig::default(),
            searxng: SearxngSearchConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BraveSearchConfig {
    pub api_key: String,
    #[serde(
        default = "default_brave_search_endpoint",
        deserialize_with = "deserialize_brave_search_endpoint"
    )]
    pub endpoint: String,
}

impl Default for BraveSearchConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            endpoint: default_brave_search_endpoint(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SearxngSearchConfig {
    pub base_url: String,
    pub api_key: String,
    #[serde(
        default = "default_searxng_api_key_header",
        deserialize_with = "deserialize_searxng_api_key_header"
    )]
    pub api_key_header: String,
    #[serde(
        default = "default_searxng_headers",
        deserialize_with = "deserialize_searxng_headers"
    )]
    pub headers: HashMap<String, String>,
}

impl Default for SearxngSearchConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            api_key: String::new(),
            api_key_header: default_searxng_api_key_header(),
            headers: default_searxng_headers(),
        }
    }
}

fn default_web_fetch_enabled() -> bool {
    true
}

fn default_web_fetch_max_chars() -> usize {
    50_000
}

fn default_web_fetch_timeout_seconds() -> u64 {
    30
}

fn default_web_fetch_cache_ttl_minutes() -> u64 {
    15
}

fn default_web_fetch_max_redirects() -> usize {
    3
}

fn default_web_fetch_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string()
}

fn default_web_fetch_readability() -> bool {
    true
}

fn default_firecrawl_enabled() -> bool {
    false
}

fn default_firecrawl_base_url() -> String {
    "https://api.firecrawl.dev".to_string()
}

fn default_firecrawl_only_main_content() -> bool {
    true
}

fn default_firecrawl_max_age_ms() -> u64 {
    172_800_000
}

fn default_firecrawl_timeout_seconds() -> u64 {
    30
}

fn default_web_search_enabled() -> bool {
    false
}

fn default_web_search_provider() -> String {
    "brave".to_string()
}

fn default_web_search_timeout_seconds() -> u64 {
    30
}

fn default_web_search_cache_ttl_minutes() -> u64 {
    15
}

fn default_web_search_max_results() -> usize {
    5
}

fn default_brave_search_endpoint() -> String {
    "https://api.search.brave.com/res/v1/web/search".to_string()
}

fn default_searxng_api_key_header() -> String {
    "X-API-Key".to_string()
}

fn default_searxng_headers() -> HashMap<String, String> {
    HashMap::new()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoolOrString {
    Bool(bool),
    Integer(i64),
    String(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum U64OrString {
    Unsigned(u64),
    Signed(i64),
    String(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UsizeOrString {
    Unsigned(usize),
    Signed(i64),
    String(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SearxngHeadersValue {
    Headers(HashMap<String, String>),
    String(String),
}

fn parse_bool_or_default(value: BoolOrString, default: bool) -> Result<bool, String> {
    match value {
        BoolOrString::Bool(value) => Ok(value),
        BoolOrString::Integer(value) => match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("expected boolean, 0, or 1".to_string()),
        },
        BoolOrString::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(default);
            }
            if trimmed.eq_ignore_ascii_case("true")
                || trimmed.eq_ignore_ascii_case("yes")
                || trimmed.eq_ignore_ascii_case("on")
                || trimmed == "1"
            {
                return Ok(true);
            }
            if trimmed.eq_ignore_ascii_case("false")
                || trimmed.eq_ignore_ascii_case("no")
                || trimmed.eq_ignore_ascii_case("off")
                || trimmed == "0"
            {
                return Ok(false);
            }
            Err("expected boolean string (true/false)".to_string())
        }
    }
}

fn parse_u64_or_default(value: U64OrString, default: u64) -> Result<u64, String> {
    match value {
        U64OrString::Unsigned(value) => Ok(value),
        U64OrString::Signed(value) => {
            u64::try_from(value).map_err(|_| "expected non-negative integer".to_string())
        }
        U64OrString::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(default);
            }
            trimmed
                .parse::<u64>()
                .map_err(|_| "expected non-negative integer string".to_string())
        }
    }
}

fn parse_usize_or_default(value: UsizeOrString, default: usize) -> Result<usize, String> {
    match value {
        UsizeOrString::Unsigned(value) => Ok(value),
        UsizeOrString::Signed(value) => {
            usize::try_from(value).map_err(|_| "expected non-negative integer".to_string())
        }
        UsizeOrString::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(default);
            }
            trimmed
                .parse::<usize>()
                .map_err(|_| "expected non-negative integer string".to_string())
        }
    }
}

fn parse_string_or_default(value: String, default: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed.to_string()
    }
}

fn deserialize_web_fetch_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_fetch_enabled()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_max_chars<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_fetch_max_chars()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_timeout_seconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_fetch_timeout_seconds()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_cache_ttl_minutes<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_fetch_cache_ttl_minutes()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_max_redirects<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_fetch_max_redirects()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_readability<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_fetch_readability()).map_err(D::Error::custom)
}

fn deserialize_web_fetch_user_agent<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_web_fetch_user_agent(),
    ))
}

fn deserialize_firecrawl_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_firecrawl_enabled()).map_err(D::Error::custom)
}

fn deserialize_firecrawl_only_main_content<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_firecrawl_only_main_content()).map_err(D::Error::custom)
}

fn deserialize_firecrawl_max_age_ms<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_firecrawl_max_age_ms()).map_err(D::Error::custom)
}

fn deserialize_firecrawl_timeout_seconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_firecrawl_timeout_seconds()).map_err(D::Error::custom)
}

fn deserialize_firecrawl_base_url<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(value, default_firecrawl_base_url()))
}

fn deserialize_web_search_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_search_enabled()).map_err(D::Error::custom)
}

fn deserialize_web_search_timeout_seconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_search_timeout_seconds()).map_err(D::Error::custom)
}

fn deserialize_web_search_cache_ttl_minutes<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_search_cache_ttl_minutes()).map_err(D::Error::custom)
}

fn deserialize_web_search_max_results<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_search_max_results()).map_err(D::Error::custom)
}

fn deserialize_web_search_provider<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_web_search_provider(),
    ))
}

fn deserialize_brave_search_endpoint<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_brave_search_endpoint(),
    ))
}

fn deserialize_searxng_api_key_header<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_searxng_api_key_header(),
    ))
}

fn deserialize_searxng_headers<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = SearxngHeadersValue::deserialize(deserializer)?;
    match value {
        SearxngHeadersValue::Headers(headers) => Ok(headers),
        SearxngHeadersValue::String(value) => {
            if value.trim().is_empty() {
                return Ok(HashMap::new());
            }
            Err(D::Error::custom(
                "expected headers table/object or empty string",
            ))
        }
    }
}

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub credentials_dir: Option<String>,
    pub execpolicy_path: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::{
        default_brave_search_endpoint, default_firecrawl_base_url, default_searxng_api_key_header,
        default_web_fetch_user_agent, default_web_search_provider, HomieConfig, ToolProviderConfig,
    };

    #[test]
    fn web_config_empty_override_strings_use_safe_defaults() {
        let raw = r#"
        [tools.web.fetch]
        user_agent = "   "

        [tools.web.fetch.firecrawl]
        base_url = ""

        [tools.web.search]
        provider = "  "

        [tools.web.search.brave]
        endpoint = ""

        [tools.web.search.searxng]
        api_key_header = "    "
        "#;
        let config: HomieConfig = toml::from_str(raw).expect("parse config");
        assert_eq!(
            config.tools.web.fetch.user_agent,
            default_web_fetch_user_agent()
        );
        assert_eq!(
            config.tools.web.fetch.firecrawl.base_url,
            default_firecrawl_base_url()
        );
        assert_eq!(
            config.tools.web.search.provider,
            default_web_search_provider()
        );
        assert_eq!(
            config.tools.web.search.brave.endpoint,
            default_brave_search_endpoint()
        );
        assert_eq!(
            config.tools.web.search.searxng.api_key_header,
            default_searxng_api_key_header()
        );
    }

    #[test]
    fn web_config_empty_numeric_bool_and_headers_strings_parse() {
        let raw = r#"
        [tools.web.fetch]
        enabled = ""
        max_chars = ""
        timeout_seconds = ""
        cache_ttl_minutes = ""
        max_redirects = ""
        readability = ""

        [tools.web.fetch.firecrawl]
        enabled = ""
        only_main_content = ""
        max_age_ms = ""
        timeout_seconds = ""

        [tools.web.search]
        enabled = ""
        timeout_seconds = ""
        cache_ttl_minutes = ""
        max_results = ""

        [tools.web.search.searxng]
        headers = ""
        "#;
        let parsed = toml::from_str::<HomieConfig>(raw);
        assert!(parsed.is_ok(), "empty overrides should parse: {parsed:?}");
    }

    #[test]
    fn tools_provider_overrides_parse() {
        let raw = r#"
        [tools.providers.core]
        enabled = true
        channels = ["web", "discord"]
        allow_tools = ["read", "ls"]
        deny_tools = ["exec"]

        [tools.providers.channel_discord]
        enabled = false
        "#;
        let config: HomieConfig = toml::from_str(raw).expect("parse config");
        let core = config
            .tools
            .providers
            .get("core")
            .expect("core provider override present");
        assert_eq!(core.enabled, Some(true));
        assert_eq!(
            core.channels,
            vec!["web".to_string(), "discord".to_string()]
        );
        assert_eq!(core.allow_tools, vec!["read".to_string(), "ls".to_string()]);
        assert_eq!(core.deny_tools, vec!["exec".to_string()]);
        let discord = config
            .tools
            .providers
            .get("channel_discord")
            .expect("channel_discord provider override present");
        assert_eq!(discord.enabled, Some(false));
    }

    #[test]
    fn tool_provider_default_is_empty() {
        let cfg = ToolProviderConfig::default();
        assert_eq!(cfg.enabled, None);
        assert!(cfg.channels.is_empty());
        assert!(cfg.allow_tools.is_empty());
        assert!(cfg.deny_tools.is_empty());
    }
}

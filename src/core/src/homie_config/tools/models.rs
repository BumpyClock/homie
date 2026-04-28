use serde::Deserialize;

use super::{
    default_brave_search_endpoint, default_firecrawl_base_url, default_firecrawl_enabled,
    default_firecrawl_max_age_ms, default_firecrawl_only_main_content,
    default_firecrawl_timeout_seconds, default_searxng_api_key_header, default_searxng_headers,
    default_web_fetch_cache_ttl_minutes, default_web_fetch_enabled, default_web_fetch_max_chars,
    default_web_fetch_max_redirects, default_web_fetch_readability,
    default_web_fetch_timeout_seconds, default_web_fetch_user_agent,
    default_web_search_cache_ttl_minutes, default_web_search_enabled,
    default_web_search_max_results, default_web_search_provider,
    default_web_search_timeout_seconds, deserialize_brave_search_endpoint,
    deserialize_firecrawl_base_url, deserialize_firecrawl_enabled,
    deserialize_firecrawl_max_age_ms, deserialize_firecrawl_only_main_content,
    deserialize_firecrawl_timeout_seconds, deserialize_searxng_api_key_header,
    deserialize_searxng_headers, deserialize_web_fetch_cache_ttl_minutes,
    deserialize_web_fetch_enabled, deserialize_web_fetch_max_chars,
    deserialize_web_fetch_max_redirects, deserialize_web_fetch_readability,
    deserialize_web_fetch_timeout_seconds, deserialize_web_fetch_user_agent,
    deserialize_web_search_cache_ttl_minutes, deserialize_web_search_enabled,
    deserialize_web_search_max_results, deserialize_web_search_provider,
    deserialize_web_search_timeout_seconds,
};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub web: WebToolsConfig,
    pub providers: std::collections::HashMap<String, ToolProviderConfig>,
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
    pub headers: std::collections::HashMap<String, String>,
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

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use roci::error::RociError;
use serde::Deserialize;
use url::Url;

use crate::homie_config::{FirecrawlConfig, WebFetchBackend};

use super::payload::{markdown_to_text, ExtractMode};

const DEFAULT_FIRECRAWL_ENDPOINT: &str = "https://api.firecrawl.dev/v2/scrape";
const FIRECRAWL_HEALTH_CACHE_TTL_SECS: u64 = 60;

static FIRECRAWL_AVAILABLE: OnceLock<Mutex<Option<(bool, Instant)>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolvedBackend {
    Native,
    Firecrawl,
}

#[derive(Debug)]
pub(super) struct FirecrawlResult {
    pub(super) text: String,
    pub(super) title: Option<String>,
    pub(super) final_url: Option<String>,
    pub(super) status: Option<u16>,
    pub(super) warning: Option<String>,
}

pub(super) async fn resolve_backend(
    config_backend: WebFetchBackend,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
) -> ResolvedBackend {
    match config_backend {
        WebFetchBackend::Native => ResolvedBackend::Native,
        WebFetchBackend::Firecrawl => {
            if base_url.trim().is_empty() {
                ResolvedBackend::Native
            } else {
                ResolvedBackend::Firecrawl
            }
        }
        WebFetchBackend::Auto => {
            if !base_url.trim().is_empty()
                && check_firecrawl_available(base_url, api_key, timeout).await
            {
                ResolvedBackend::Firecrawl
            } else {
                ResolvedBackend::Native
            }
        }
    }
}

async fn check_firecrawl_available(base_url: &str, api_key: Option<&str>, timeout: u64) -> bool {
    let cache = FIRECRAWL_AVAILABLE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((available, ts)) = *guard {
            if ts.elapsed().as_secs() < FIRECRAWL_HEALTH_CACHE_TTL_SECS {
                return available;
            }
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5.min(timeout.max(1))))
        .build()
    {
        Ok(c) => c,
        Err(_) => return cache_firecrawl_result(false),
    };

    let health_url = if let Ok(mut url) = Url::parse(base_url) {
        url.set_path("/health");
        url.to_string()
    } else {
        return cache_firecrawl_result(false);
    };

    if let Ok(resp) = client.get(&health_url).send().await {
        if resp.status().is_success() {
            return cache_firecrawl_result(true);
        }
    }

    let endpoint = resolve_firecrawl_endpoint(base_url);
    let body = serde_json::json!({
        "url": "https://example.com",
        "formats": ["markdown"],
        "timeout": 5000,
    });
    let mut req = client
        .post(&endpoint)
        .header("Content-Type", "application/json");
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let available = if let Ok(resp) = req.json(&body).send().await {
        resp.status().is_success()
    } else {
        false
    };

    cache_firecrawl_result(available)
}

fn cache_firecrawl_result(available: bool) -> bool {
    let cache = FIRECRAWL_AVAILABLE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((available, Instant::now()));
    }
    available
}

#[derive(Debug, Deserialize)]
struct FirecrawlResponse {
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    data: Option<FirecrawlData>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlData {
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    metadata: Option<FirecrawlMetadata>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlMetadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "sourceURL")]
    source_url: Option<String>,
    #[serde(default, rename = "statusCode")]
    status_code: Option<u16>,
}

pub(super) async fn fetch_firecrawl_content(
    url: &str,
    extract_mode: ExtractMode,
    cfg: &FirecrawlConfig,
    api_key: Option<&str>,
    base_url: &str,
) -> Result<FirecrawlResult, RociError> {
    let endpoint = resolve_firecrawl_endpoint(base_url);
    let timeout_ms = cfg.timeout_seconds.max(1) * 1000;
    let body = serde_json::json!({
        "url": url,
        "formats": ["markdown"],
        "onlyMainContent": cfg.only_main_content,
        "timeout": timeout_ms,
        "maxAge": cfg.max_age_ms,
        "proxy": "auto",
        "storeInCache": true
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("failed to build firecrawl client: {e}"),
        })?;
    let mut req = client
        .post(endpoint)
        .header("Content-Type", "application/json");
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let res = req
        .json(&body)
        .send()
        .await
        .map_err(|e| RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("firecrawl request failed: {e}"),
        })?;
    let status = res.status();
    let payload: FirecrawlResponse = res.json().await.map_err(|e| RociError::ToolExecution {
        tool_name: "web_fetch".into(),
        message: format!("firecrawl parse failed: {e}"),
    })?;
    if !status.is_success() || payload.success == Some(false) {
        let detail = payload.error.unwrap_or_else(|| status.to_string());
        return Err(RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("firecrawl fetch failed ({status}): {detail}"),
        });
    }
    let data = payload.data.unwrap_or(FirecrawlData {
        markdown: None,
        content: None,
        metadata: None,
    });
    let raw_text = data.markdown.or(data.content).unwrap_or_default();
    let text = match extract_mode {
        ExtractMode::Markdown => raw_text,
        ExtractMode::Text => markdown_to_text(&raw_text),
    };
    let meta = data.metadata.unwrap_or(FirecrawlMetadata {
        title: None,
        source_url: None,
        status_code: None,
    });
    Ok(FirecrawlResult {
        text,
        title: meta.title,
        final_url: meta.source_url,
        status: meta.status_code,
        warning: payload.warning,
    })
}

fn resolve_firecrawl_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return DEFAULT_FIRECRAWL_ENDPOINT.to_string();
    }
    if let Ok(mut url) = Url::parse(trimmed) {
        if url.path() != "/" && !url.path().is_empty() {
            return url.to_string();
        }
        url.set_path("/v2/scrape");
        return url.to_string();
    }
    DEFAULT_FIRECRAWL_ENDPOINT.to_string()
}

pub(super) fn resolve_firecrawl_enabled(cfg: &FirecrawlConfig) -> bool {
    if cfg.enabled {
        return true;
    }
    resolve_firecrawl_api_key(cfg).is_some()
}

pub(super) fn resolve_firecrawl_api_key(cfg: &FirecrawlConfig) -> Option<String> {
    if !cfg.api_key.trim().is_empty() {
        return Some(cfg.api_key.trim().to_string());
    }
    let from_env = std::env::var("FIRECRAWL_API_KEY").unwrap_or_default();
    let trimmed = from_env.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn resolve_firecrawl_base_url(cfg: &FirecrawlConfig) -> String {
    if !cfg.base_url.trim().is_empty() {
        return cfg.base_url.trim().to_string();
    }
    FirecrawlConfig::default().base_url
}

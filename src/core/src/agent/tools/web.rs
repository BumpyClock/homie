use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use serde::Deserialize;
use tokio::net::lookup_host;
use url::{Host, Url};

use crate::homie_config::{
    BraveSearchConfig, FirecrawlConfig, SearxngSearchConfig, WebFetchBackend, WebFetchConfig,
    WebSearchConfig,
};

use super::{debug_tools_enabled, ToolContext};

const MAX_SEARCH_COUNT: usize = 10;
const DEFAULT_ERROR_MAX_CHARS: usize = 4000;
const CACHE_MAX_ENTRIES: usize = 100;
const DEFAULT_FIRECRAWL_ENDPOINT: &str = "https://api.firecrawl.dev/v2/scrape";
const WEB_FETCH_TOOL_NAME: &str = "web_fetch";
const WEB_SEARCH_TOOL_NAME: &str = "web_search";

static FETCH_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
static SEARCH_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
static FIRECRAWL_AVAILABLE: OnceLock<Mutex<Option<(bool, Instant)>>> = OnceLock::new();
const FIRECRAWL_HEALTH_CACHE_TTL_SECS: u64 = 60;

#[derive(Clone)]
struct CacheEntry {
    value: serde_json::Value,
    expires_at: Instant,
}

#[derive(Debug, Deserialize)]
struct WebFetchArgs {
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "extractMode")]
    extract_mode: Option<String>,
    #[serde(default, rename = "maxChars")]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WebSearchArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    count: Option<usize>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default, rename = "search_lang")]
    search_lang: Option<String>,
    #[serde(default, rename = "ui_lang")]
    ui_lang: Option<String>,
    #[serde(default)]
    freshness: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum ExtractMode {
    Markdown,
    Text,
}

#[derive(Debug)]
struct FirecrawlResult {
    text: String,
    title: Option<String>,
    final_url: Option<String>,
    status: Option<u16>,
    warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedBackend {
    Native,
    Firecrawl,
}

async fn resolve_backend(
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

    // Try /health endpoint
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

    // Fallback: probe scrape of example.com
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
struct BraveSearchResponse {
    #[serde(default)]
    web: Option<BraveWebResult>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResult {
    #[serde(default)]
    results: Option<Vec<BraveSearchItem>>,
}

#[derive(Debug, Deserialize)]
struct BraveSearchItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    age: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Option<Vec<SearxngItem>>,
}

#[derive(Debug, Deserialize)]
struct SearxngItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default, rename = "publishedDate")]
    published_date: Option<String>,
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

pub fn web_fetch_tool(ctx: ToolContext) -> Option<Arc<dyn Tool>> {
    if !ctx.web.fetch.enabled {
        return None;
    }
    let params = AgentToolParameters::object()
        .string("url", "HTTP or HTTPS URL to fetch.", true)
        .string("extractMode", "Extraction mode (markdown or text).", false)
        .number("maxChars", "Maximum characters to return.", false)
        .build();

    Some(Arc::new(AgentTool::new(
        "web_fetch",
        "Fetch and extract readable content from a URL (HTML → markdown/text).",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { web_fetch_impl(&ctx, &args).await }
        },
    )))
}

pub fn web_search_tool(ctx: ToolContext) -> Option<Arc<dyn Tool>> {
    if !ctx.web.search.enabled {
        return None;
    }
    let params = AgentToolParameters::object()
        .string("query", "Search query string.", true)
        .number("count", "Number of results to return (1-10).", false)
        .string(
            "country",
            "2-letter country code for region-specific results.",
            false,
        )
        .string(
            "search_lang",
            "ISO language code for search results.",
            false,
        )
        .string("ui_lang", "ISO language code for UI elements.", false)
        .string(
            "freshness",
            "Brave only: pd|pw|pm|py|YYYY-MM-DDtoYYYY-MM-DD.",
            false,
        )
        .build();

    Some(Arc::new(AgentTool::new(
        "web_search",
        "Search the web using Brave API or SearXNG.",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { web_search_impl(&ctx, &args).await }
        },
    )))
}

async fn web_fetch_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    match web_fetch_inner(ctx, args).await {
        Ok(payload) => Ok(success_envelope(WEB_FETCH_TOOL_NAME, payload)),
        Err(err) => Ok(error_envelope_from_roci(WEB_FETCH_TOOL_NAME, err)),
    }
}

async fn web_fetch_inner(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed: WebFetchArgs = args.deserialize()?;
    let url = parsed
        .url
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| RociError::InvalidArgument("url required".into()))?;
    let extract_mode = match parsed.extract_mode.as_deref() {
        Some("text") => ExtractMode::Text,
        _ => ExtractMode::Markdown,
    };
    let cfg = &ctx.web.fetch;
    let max_chars = parsed.max_chars.unwrap_or(cfg.max_chars).max(100);
    let max_redirects = cfg.max_redirects;
    let timeout_seconds = cfg.timeout_seconds.max(1);
    let cache_ttl_ms = cfg.cache_ttl_minutes.saturating_mul(60_000);
    let user_agent = if cfg.user_agent.trim().is_empty() {
        WebFetchConfig::default().user_agent
    } else {
        cfg.user_agent.clone()
    };

    let cache_key = normalize_cache_key(&format!("fetch:{}:{:?}:{max_chars}", url, extract_mode));
    if let Some(cached) = read_cache(fetch_cache(), &cache_key) {
        let mut payload = cached.value;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("cached".into(), serde_json::Value::Bool(true));
        }
        return Ok(payload);
    }

    if debug_tools_enabled() {
        tracing::debug!(url, max_chars, "web_fetch invoked");
    }

    let start = Instant::now();

    let firecrawl_cfg = &cfg.firecrawl;
    let firecrawl_enabled = resolve_firecrawl_enabled(firecrawl_cfg);
    let firecrawl_api_key = resolve_firecrawl_api_key(firecrawl_cfg);
    let firecrawl_base_url = resolve_firecrawl_base_url(firecrawl_cfg);

    let resolved_backend = resolve_backend(
        cfg.backend,
        &firecrawl_base_url,
        firecrawl_api_key.as_deref(),
        firecrawl_cfg.timeout_seconds,
    )
    .await;

    // Firecrawl-first path: try Firecrawl before native fetch
    if resolved_backend == ResolvedBackend::Firecrawl {
        // SSRF check before sending URL to Firecrawl
        let parsed_url = Url::parse(url)
            .map_err(|_| RociError::InvalidArgument("invalid url".into()))?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(RociError::InvalidArgument("invalid url scheme".into()));
        }
        ensure_url_safe(&parsed_url).await?;

        match fetch_firecrawl_content(
            url,
            extract_mode,
            firecrawl_cfg,
            firecrawl_api_key.as_deref(),
            &firecrawl_base_url,
        )
        .await
        {
            Ok(firecrawl) => {
                let payload = build_fetch_payload(
                    url,
                    firecrawl.final_url.as_deref().unwrap_or(url),
                    firecrawl.status.unwrap_or(200),
                    "text/markdown",
                    firecrawl.title.as_deref(),
                    extract_mode,
                    "firecrawl",
                    "firecrawl",
                    &firecrawl.text,
                    max_chars,
                    start,
                    firecrawl.warning.as_deref(),
                );
                write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
                return Ok(payload);
            }
            Err(e) => {
                tracing::warn!("firecrawl primary fetch failed, falling back to native: {e}");
            }
        }
    }

    // Native fetch path (primary for Native, fallback for Firecrawl)
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("failed to build http client: {e}"),
        })?;

    let fetch_result = fetch_with_redirects(&client, url, max_redirects, &user_agent).await;

    // Only allow Firecrawl fallback when resolved to Native (haven't already tried Firecrawl)
    let allow_firecrawl_fallback =
        resolved_backend == ResolvedBackend::Native && firecrawl_enabled;

    let (response, final_url) = match fetch_result {
        Ok(res) => res,
        Err(err) => {
            if matches!(err, RociError::InvalidArgument(_)) {
                return Err(err);
            }
            if allow_firecrawl_fallback {
                let firecrawl = fetch_firecrawl_content(
                    url,
                    extract_mode,
                    firecrawl_cfg,
                    firecrawl_api_key.as_deref(),
                    &firecrawl_base_url,
                )
                .await?;
                let payload = build_fetch_payload(
                    url,
                    firecrawl.final_url.as_deref().unwrap_or(url),
                    firecrawl.status.unwrap_or(200),
                    "text/markdown",
                    firecrawl.title.as_deref(),
                    extract_mode,
                    "firecrawl",
                    "firecrawl",
                    &firecrawl.text,
                    max_chars,
                    start,
                    firecrawl.warning.as_deref(),
                );
                write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
                return Ok(payload);
            }
            return Err(err);
        }
    };

    if !response.status().is_success() {
        if allow_firecrawl_fallback {
            let firecrawl = fetch_firecrawl_content(
                url,
                extract_mode,
                firecrawl_cfg,
                firecrawl_api_key.as_deref(),
                &firecrawl_base_url,
            )
            .await?;
            let payload = build_fetch_payload(
                url,
                firecrawl.final_url.as_deref().unwrap_or(url),
                firecrawl.status.unwrap_or(response.status().as_u16()),
                "text/markdown",
                firecrawl.title.as_deref(),
                extract_mode,
                "firecrawl",
                "firecrawl",
                &firecrawl.text,
                max_chars,
                start,
                firecrawl.warning.as_deref(),
            );
            write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
            return Ok(payload);
        }
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let detail = response.text().await.unwrap_or_default();
        let rendered = format_fetch_error_detail(&detail, content_type.as_deref());
        return Err(RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("web fetch failed ({status}): {rendered}"),
        });
    }

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let body = response.text().await.unwrap_or_default();

    let mut extractor = "raw";
    let mut title: Option<String> = None;
    let mut text = body.clone();

    if content_type.to_lowercase().contains("text/html") {
        if cfg.readability {
            if let Some((content, extracted_title)) =
                extract_readable(&body, &final_url, extract_mode)
            {
                text = content;
                title = extracted_title;
                extractor = "readability";
            } else if allow_firecrawl_fallback {
                let firecrawl = fetch_firecrawl_content(
                    url,
                    extract_mode,
                    firecrawl_cfg,
                    firecrawl_api_key.as_deref(),
                    &firecrawl_base_url,
                )
                .await?;
                let payload = build_fetch_payload(
                    url,
                    firecrawl.final_url.as_deref().unwrap_or(url),
                    firecrawl.status.unwrap_or(200),
                    "text/markdown",
                    firecrawl.title.as_deref(),
                    extract_mode,
                    "firecrawl",
                    "firecrawl",
                    &firecrawl.text,
                    max_chars,
                    start,
                    firecrawl.warning.as_deref(),
                );
                write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
                return Ok(payload);
            } else {
                return Err(RociError::ToolExecution {
                    tool_name: "web_fetch".into(),
                    message: "web fetch extraction failed: readability disabled and firecrawl unavailable".into(),
                });
            }
        } else {
            return Err(RociError::ToolExecution {
                tool_name: "web_fetch".into(),
                message:
                    "web fetch extraction failed: readability disabled and firecrawl unavailable"
                        .into(),
            });
        }
    } else if content_type.contains("application/json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            text = serde_json::to_string_pretty(&json).unwrap_or(body);
            extractor = "json";
        }
    }

    let payload = build_fetch_payload(
        url,
        &final_url,
        status,
        &content_type,
        title.as_deref(),
        extract_mode,
        extractor,
        "native",
        &text,
        max_chars,
        start,
        None,
    );
    write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
    Ok(payload)
}

async fn web_search_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    match web_search_inner(ctx, args).await {
        Ok(payload) => Ok(wrap_tool_payload(WEB_SEARCH_TOOL_NAME, payload)),
        Err(err) => Ok(error_envelope_from_roci(WEB_SEARCH_TOOL_NAME, err)),
    }
}

async fn web_search_inner(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed: WebSearchArgs = args.deserialize()?;
    let query = parsed
        .query
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| RociError::InvalidArgument("query required".into()))?;
    let cfg = &ctx.web.search;
    let provider = normalize_provider(&cfg.provider);
    let count = parsed
        .count
        .unwrap_or(cfg.max_results)
        .clamp(1, MAX_SEARCH_COUNT);
    let timeout_seconds = cfg.timeout_seconds.max(1);
    let cache_ttl_ms = cfg.cache_ttl_minutes.saturating_mul(60_000);

    if let Some(freshness) = parsed.freshness.as_deref().filter(|s| !s.trim().is_empty()) {
        if provider != "brave" {
            return Ok(serde_json::json!({
                "error": "unsupported_freshness",
                "message": "freshness is only supported by the Brave web_search provider."
            }));
        }
        if normalize_freshness(freshness).is_none() {
            return Ok(serde_json::json!({
                "error": "invalid_freshness",
                "message": "freshness must be one of pd, pw, pm, py, or a range like YYYY-MM-DDtoYYYY-MM-DD."
            }));
        }
    }

    let cache_key = normalize_cache_key(&format!(
        "search:{}:{}:{:?}:{:?}:{:?}:{:?}:{:?}",
        provider,
        query,
        count,
        parsed.country,
        parsed.search_lang,
        parsed.ui_lang,
        parsed.freshness,
    ));
    if let Some(cached) = read_cache(search_cache(), &cache_key) {
        let mut payload = cached.value;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("cached".into(), serde_json::Value::Bool(true));
        }
        return Ok(payload);
    }

    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|e| RociError::ToolExecution {
            tool_name: "web_search".into(),
            message: format!("failed to build http client: {e}"),
        })?;

    let payload = if provider == "searxng" {
        run_searxng_search(&client, cfg, query, count, &parsed).await?
    } else {
        run_brave_search(&client, cfg, query, count, &parsed).await?
    };

    let payload = match payload {
        serde_json::Value::Object(mut map) => {
            map.insert(
                "tookMs".into(),
                serde_json::Value::Number((start.elapsed().as_millis() as u64).into()),
            );
            serde_json::Value::Object(map)
        }
        other => other,
    };
    let should_cache = payload
        .as_object()
        .and_then(|map| map.get("error"))
        .is_none();
    if should_cache {
        write_cache(search_cache(), &cache_key, payload.clone(), cache_ttl_ms);
    }
    Ok(payload)
}

fn wrap_tool_payload(tool_name: &str, payload: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = payload.as_object() {
        if let Some(code) = obj.get("error").and_then(|value| value.as_str()) {
            let message = obj
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("tool request failed");
            let mut details = obj.clone();
            details.remove("error");
            details.remove("message");
            let details = if details.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(details))
            };
            return error_envelope(tool_name, code, message.to_string(), false, details);
        }
    }
    success_envelope(tool_name, payload)
}

fn success_envelope(tool_name: &str, data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "tool": tool_name,
        "data": data
    })
}

fn error_envelope_from_roci(tool_name: &str, err: RociError) -> serde_json::Value {
    let retryable = err.is_retryable();
    match err {
        RociError::InvalidArgument(message) => {
            error_envelope(tool_name, "invalid_argument", message, retryable, None)
        }
        RociError::Timeout(timeout_ms) => error_envelope(
            tool_name,
            "timeout",
            format!("request timed out after {timeout_ms}ms"),
            retryable,
            None,
        ),
        RociError::Network(message) => error_envelope(
            tool_name,
            "network_error",
            message.to_string(),
            retryable,
            None,
        ),
        RociError::Serialization(message) => error_envelope(
            tool_name,
            "serialization_error",
            message.to_string(),
            retryable,
            None,
        ),
        RociError::ToolExecution { message, .. } => {
            error_envelope(tool_name, "tool_execution_failed", message, retryable, None)
        }
        other => error_envelope(tool_name, "tool_error", other.to_string(), retryable, None),
    }
}

fn error_envelope(
    tool_name: &str,
    code: &str,
    message: String,
    retryable: bool,
    details: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut error = serde_json::Map::new();
    error.insert(
        "code".to_string(),
        serde_json::Value::String(code.to_string()),
    );
    error.insert("message".to_string(), serde_json::Value::String(message));
    error.insert("retryable".to_string(), serde_json::Value::Bool(retryable));
    if let Some(details) = details {
        error.insert("details".to_string(), details);
    }
    serde_json::json!({
        "ok": false,
        "tool": tool_name,
        "error": error
    })
}

async fn run_brave_search(
    client: &reqwest::Client,
    cfg: &WebSearchConfig,
    query: &str,
    count: usize,
    parsed: &WebSearchArgs,
) -> Result<serde_json::Value, RociError> {
    let BraveSearchConfig { api_key, endpoint } = &cfg.brave;
    let key = resolve_brave_api_key(api_key);
    if key.is_empty() {
        return Ok(serde_json::json!({
            "error": "missing_brave_api_key",
            "message": "web_search needs a Brave Search API key. Set BRAVE_API_KEY or tools.web.search.brave.api_key in config."
        }));
    }

    let mut req = client
        .get(endpoint)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", key);

    let mut params: Vec<(&str, String)> =
        vec![("q", query.to_string()), ("count", count.to_string())];
    if let Some(country) = parsed.country.as_deref().filter(|s| !s.trim().is_empty()) {
        params.push(("country", country.to_string()));
    }
    if let Some(lang) = parsed
        .search_lang
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        params.push(("search_lang", lang.to_string()));
    }
    if let Some(lang) = parsed.ui_lang.as_deref().filter(|s| !s.trim().is_empty()) {
        params.push(("ui_lang", lang.to_string()));
    }
    if let Some(freshness) = parsed.freshness.as_deref().filter(|s| !s.trim().is_empty()) {
        if let Some(value) = normalize_freshness(freshness) {
            params.push(("freshness", value));
        }
    }

    req = req.query(&params);
    let res = req.send().await.map_err(|e| RociError::ToolExecution {
        tool_name: "web_search".into(),
        message: format!("brave request failed: {e}"),
    })?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(RociError::ToolExecution {
            tool_name: "web_search".into(),
            message: format!(
                "brave request failed ({status}): {}",
                truncate_str(&body, DEFAULT_ERROR_MAX_CHARS)
            ),
        });
    }
    let parsed_body: BraveSearchResponse =
        serde_json::from_str(&body).map_err(|e| RociError::ToolExecution {
            tool_name: "web_search".into(),
            message: format!("brave response parse failed: {e}"),
        })?;
    let results = parsed_body
        .web
        .and_then(|web| web.results)
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            let site = item.url.as_deref().and_then(resolve_site_name);
            serde_json::json!({
                "title": item.title.unwrap_or_default(),
                "url": item.url.unwrap_or_default(),
                "snippet": item.description.unwrap_or_default(),
                "published": item.age,
                "siteName": site,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "query": query,
        "provider": "brave",
        "count": results.len(),
        "results": results
    }))
}

async fn run_searxng_search(
    client: &reqwest::Client,
    cfg: &WebSearchConfig,
    query: &str,
    count: usize,
    parsed: &WebSearchArgs,
) -> Result<serde_json::Value, RociError> {
    let SearxngSearchConfig {
        base_url,
        api_key,
        api_key_header,
        headers,
    } = &cfg.searxng;
    let base_url = resolve_searxng_base_url(base_url);
    if base_url.is_empty() {
        return Ok(serde_json::json!({
            "error": "missing_searxng_base_url",
            "message": "web_search (searxng) needs tools.web.search.searxng.base_url in config."
        }));
    }
    if let Some(freshness) = parsed.freshness.as_deref().filter(|s| !s.trim().is_empty()) {
        return Ok(serde_json::json!({
            "error": "unsupported_freshness",
            "message": format!("freshness is not supported by searxng (received {freshness}).")
        }));
    }

    let endpoint = resolve_searxng_endpoint(&base_url);
    let mut req = client.get(endpoint).header("Accept", "application/json");
    let mut params: Vec<(&str, String)> = vec![
        ("q", query.to_string()),
        ("format", "json".to_string()),
        ("safesearch", "0".to_string()),
    ];
    if let Some(lang) = parsed
        .search_lang
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        params.push(("language", lang.to_string()));
    } else if let Some(lang) = parsed.ui_lang.as_deref().filter(|s| !s.trim().is_empty()) {
        params.push(("language", lang.to_string()));
    }
    req = req.query(&params);

    let mut header_map = reqwest::header::HeaderMap::new();
    for (key, value) in headers {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            header_map.insert(name, val);
        }
    }
    let api_key = resolve_searxng_api_key(api_key);
    if !api_key.is_empty() {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(api_key_header.as_bytes()),
            reqwest::header::HeaderValue::from_str(&api_key),
        ) {
            header_map.insert(name, val);
        }
    }
    req = req.headers(header_map);

    let res = req.send().await.map_err(|e| RociError::ToolExecution {
        tool_name: "web_search".into(),
        message: format!("searxng request failed: {e}"),
    })?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(RociError::ToolExecution {
            tool_name: "web_search".into(),
            message: format!(
                "searxng request failed ({status}): {}",
                truncate_str(&body, DEFAULT_ERROR_MAX_CHARS)
            ),
        });
    }
    let parsed_body: SearxngResponse =
        serde_json::from_str(&body).map_err(|e| RociError::ToolExecution {
            tool_name: "web_search".into(),
            message: format!("searxng response parse failed: {e}"),
        })?;
    let results = parsed_body
        .results
        .unwrap_or_default()
        .into_iter()
        .take(count)
        .map(|item| {
            let site = item.url.as_deref().and_then(resolve_site_name);
            serde_json::json!({
                "title": item.title.unwrap_or_default(),
                "url": item.url.unwrap_or_default(),
                "snippet": item.content.unwrap_or_default(),
                "published": item.published_date,
                "siteName": site,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "query": query,
        "provider": "searxng",
        "count": results.len(),
        "results": results
    }))
}

async fn fetch_with_redirects(
    client: &reqwest::Client,
    url: &str,
    max_redirects: usize,
    user_agent: &str,
) -> Result<(reqwest::Response, String), RociError> {
    let mut current =
        Url::parse(url).map_err(|_| RociError::InvalidArgument("invalid url".into()))?;
    if !matches!(current.scheme(), "http" | "https") {
        return Err(RociError::InvalidArgument("invalid url scheme".into()));
    }
    let mut visited = std::collections::HashSet::new();
    visited.insert(current.as_str().to_string());
    let mut redirects = 0usize;

    loop {
        ensure_url_safe(&current).await?;
        let req = client
            .get(current.clone())
            .header("Accept", "*/*")
            .header("User-Agent", user_agent);
        let res = req.send().await.map_err(|e| RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("request failed: {e}"),
        })?;
        if is_redirect(res.status()) {
            if redirects >= max_redirects {
                return Err(RociError::ToolExecution {
                    tool_name: "web_fetch".into(),
                    message: format!("too many redirects (limit: {max_redirects})"),
                });
            }
            let location = res
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let next = current
                .join(location)
                .map_err(|_| RociError::InvalidArgument("invalid redirect url".into()))?;
            if !visited.insert(next.as_str().to_string()) {
                return Err(RociError::ToolExecution {
                    tool_name: "web_fetch".into(),
                    message: "redirect loop detected".into(),
                });
            }
            current = next;
            redirects += 1;
            continue;
        }
        return Ok((res, current.to_string()));
    }
}

async fn ensure_url_safe(url: &Url) -> Result<(), RociError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(RociError::InvalidArgument("invalid url scheme".into()));
    }
    let host = url
        .host()
        .ok_or_else(|| RociError::InvalidArgument("invalid url host".into()))?;
    let host_str = url.host_str().unwrap_or_default().to_lowercase();
    if host_str == "localhost" || host_str.ends_with(".local") {
        return Err(RociError::InvalidArgument("blocked host".into()));
    }
    if let Host::Ipv4(ip) = host {
        if ip_is_private(IpAddr::V4(ip)) {
            return Err(RociError::InvalidArgument("blocked host".into()));
        }
        return Ok(());
    }
    if let Host::Ipv6(ip) = host {
        if ip_is_private(IpAddr::V6(ip)) {
            return Err(RociError::InvalidArgument("blocked host".into()));
        }
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs =
        lookup_host((host_str.as_str(), port))
            .await
            .map_err(|e| RociError::ToolExecution {
                tool_name: "web_fetch".into(),
                message: format!("dns lookup failed: {e}"),
            })?;
    for addr in addrs {
        if ip_is_private(addr.ip()) {
            return Err(RociError::InvalidArgument("blocked host".into()));
        }
    }
    Ok(())
}

fn ip_is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6.is_unspecified()
        }
    }
}

fn is_redirect(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::MOVED_PERMANENTLY
            | reqwest::StatusCode::FOUND
            | reqwest::StatusCode::SEE_OTHER
            | reqwest::StatusCode::TEMPORARY_REDIRECT
            | reqwest::StatusCode::PERMANENT_REDIRECT
    )
}

fn extract_readable(html: &str, url: &str, mode: ExtractMode) -> Option<(String, Option<String>)> {
    let readability = readabilityrs::Readability::new(html, Some(url), None).ok()?;
    let article = readability.parse()?;
    let title = article.title.clone();
    let content = article.content.unwrap_or_default();
    if content.trim().is_empty() {
        return None;
    }
    let text = match mode {
        ExtractMode::Markdown => htmd::convert(&content).unwrap_or_else(|_| content.clone()),
        ExtractMode::Text => {
            if let Some(text_content) = article.text_content {
                text_content
            } else {
                html2text::from_read(content.as_bytes(), 100).unwrap_or_default()
            }
        }
    };
    Some((text, title))
}

async fn fetch_firecrawl_content(
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

fn build_fetch_payload(
    url: &str,
    final_url: &str,
    status: u16,
    content_type: &str,
    title: Option<&str>,
    extract_mode: ExtractMode,
    extractor: &str,
    backend: &str,
    text: &str,
    max_chars: usize,
    start: Instant,
    warning: Option<&str>,
) -> serde_json::Value {
    let truncated = truncate_text(text, max_chars);
    let mode = match extract_mode {
        ExtractMode::Markdown => "markdown",
        ExtractMode::Text => "text",
    };
    let mut obj = serde_json::json!({
        "url": url,
        "finalUrl": final_url,
        "status": status,
        "contentType": content_type,
        "extractMode": mode,
        "extractor": extractor,
        "backend": backend,
        "truncated": truncated.1,
        "length": truncated.0.chars().count(),
        "fetchedAt": chrono::Utc::now().to_rfc3339(),
        "tookMs": start.elapsed().as_millis() as u64,
        "text": truncated.0,
    });
    if let Some(title) = title {
        if let Some(map) = obj.as_object_mut() {
            map.insert("title".into(), serde_json::Value::String(title.to_string()));
        }
    }
    if let Some(warning) = warning {
        if let Some(map) = obj.as_object_mut() {
            map.insert(
                "warning".into(),
                serde_json::Value::String(warning.to_string()),
            );
        }
    }
    obj
}

fn truncate_text(text: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), text.is_empty());
    }
    let mut out = String::new();
    let mut count = 0usize;
    for ch in text.chars() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
        count += 1;
    }
    let truncated = text.chars().count() > max_chars;
    (out, truncated)
}

fn format_fetch_error_detail(detail: &str, content_type: Option<&str>) -> String {
    let trimmed = detail.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut text = trimmed.to_string();
    if content_type
        .map(|v| v.to_lowercase().contains("text/html"))
        .unwrap_or(false)
        || looks_like_html(trimmed)
    {
        let markdown = htmd::convert(trimmed).unwrap_or_else(|_| trimmed.to_string());
        text = markdown_to_text(&markdown);
    }
    truncate_str(&text, DEFAULT_ERROR_MAX_CHARS)
}

fn looks_like_html(value: &str) -> bool {
    let trimmed = value.trim_start().to_lowercase();
    trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html")
}

fn markdown_to_text(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser};
    let mut out = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            _ => {}
        }
    }
    out
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

fn resolve_firecrawl_enabled(cfg: &FirecrawlConfig) -> bool {
    if cfg.enabled {
        return true;
    }
    resolve_firecrawl_api_key(cfg).is_some()
}

fn resolve_firecrawl_api_key(cfg: &FirecrawlConfig) -> Option<String> {
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

fn resolve_firecrawl_base_url(cfg: &FirecrawlConfig) -> String {
    if !cfg.base_url.trim().is_empty() {
        return cfg.base_url.trim().to_string();
    }
    FirecrawlConfig::default().base_url
}

fn normalize_provider(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value == "searxng" {
        "searxng".to_string()
    } else {
        "brave".to_string()
    }
}

fn resolve_brave_api_key(config_value: &str) -> String {
    if !config_value.trim().is_empty() {
        return config_value.trim().to_string();
    }
    std::env::var("BRAVE_API_KEY")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn resolve_searxng_base_url(config_value: &str) -> String {
    if !config_value.trim().is_empty() {
        return config_value.trim().to_string();
    }
    std::env::var("SEARXNG_BASE_URL")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn resolve_searxng_api_key(config_value: &str) -> String {
    if !config_value.trim().is_empty() {
        return config_value.trim().to_string();
    }
    std::env::var("SEARXNG_API_KEY")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn resolve_searxng_endpoint(base_url: &str) -> String {
    if let Ok(mut url) = Url::parse(base_url) {
        if url.path().is_empty() || url.path() == "/" {
            url.set_path("/search");
        }
        return url.to_string();
    }
    base_url.to_string()
}

fn normalize_freshness(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if matches!(trimmed, "pd" | "pw" | "pm" | "py") {
        return Some(trimmed.to_string());
    }
    let parts: Vec<&str> = trimmed.split("to").collect();
    if parts.len() != 2 {
        return None;
    }
    let start = chrono::NaiveDate::parse_from_str(parts[0], "%Y-%m-%d").ok()?;
    let end = chrono::NaiveDate::parse_from_str(parts[1], "%Y-%m-%d").ok()?;
    if start > end {
        return None;
    }
    Some(format!("{}to{}", parts[0], parts[1]))
}

fn truncate_str(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    for ch in value.chars() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
        count += 1;
    }
    out
}

fn resolve_site_name(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|s| s.to_string()))
}

fn normalize_cache_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn read_cache(cache: &Mutex<HashMap<String, CacheEntry>>, key: &str) -> Option<CacheEntry> {
    let mut guard = cache.lock().ok()?;
    if let Some(entry) = guard.get(key) {
        if entry.expires_at > Instant::now() {
            return Some(entry.clone());
        }
    }
    guard.remove(key);
    None
}

fn write_cache(
    cache: &Mutex<HashMap<String, CacheEntry>>,
    key: &str,
    value: serde_json::Value,
    ttl_ms: u64,
) {
    if ttl_ms == 0 {
        return;
    }
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if guard.len() >= CACHE_MAX_ENTRIES {
        if let Some(oldest) = guard.keys().next().cloned() {
            guard.remove(&oldest);
        }
    }
    guard.insert(
        key.to_string(),
        CacheEntry {
            value,
            expires_at: Instant::now() + Duration::from_millis(ttl_ms),
        },
    );
}

fn fetch_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    FETCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn search_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    SEARCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, OnceLock};

    use roci::tools::ToolArguments;
    use serde_json::json;

    use crate::homie_config::HomieConfig;

    use std::time::Instant;

    use crate::homie_config::WebFetchBackend;

    use super::{
        build_fetch_payload, fetch_cache, normalize_cache_key, resolve_backend, search_cache,
        web_fetch_impl, web_search_impl, write_cache, ExtractMode, ResolvedBackend, ToolContext,
        WEB_FETCH_TOOL_NAME, WEB_SEARCH_TOOL_NAME,
    };

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> &'static Mutex<()> {
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_caches() {
        if let Ok(mut guard) = fetch_cache().lock() {
            guard.clear();
        }
        if let Ok(mut guard) = search_cache().lock() {
            guard.clear();
        }
    }

    fn test_ctx(config: HomieConfig) -> ToolContext {
        ToolContext::new(Arc::new(config))
    }

    #[tokio::test]
    async fn web_fetch_returns_structured_error_envelope_for_missing_url() {
        let _guard = test_lock().lock().expect("test lock");
        clear_caches();
        let mut config = HomieConfig::default();
        config.tools.web.fetch.enabled = true;
        let ctx = test_ctx(config);
        let payload = web_fetch_impl(&ctx, &ToolArguments::new(json!({})))
            .await
            .expect("web_fetch response");
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["tool"], json!(WEB_FETCH_TOOL_NAME));
        assert_eq!(payload["error"]["code"], json!("invalid_argument"));
    }

    #[tokio::test]
    async fn web_search_wraps_provider_validation_error_envelope() {
        let _guard = test_lock().lock().expect("test lock");
        clear_caches();
        let mut config = HomieConfig::default();
        config.tools.web.search.enabled = true;
        config.tools.web.search.provider = "searxng".to_string();
        let ctx = test_ctx(config);
        let payload = web_search_impl(
            &ctx,
            &ToolArguments::new(json!({
                "query": "rust toolchains",
                "freshness": "pd"
            })),
        )
        .await
        .expect("web_search response");
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["tool"], json!(WEB_SEARCH_TOOL_NAME));
        assert_eq!(payload["error"]["code"], json!("unsupported_freshness"));
    }

    #[tokio::test]
    async fn web_fetch_cached_payload_returns_success_envelope() {
        let _guard = test_lock().lock().expect("test lock");
        clear_caches();
        let mut config = HomieConfig::default();
        config.tools.web.fetch.enabled = true;
        let cache_key = normalize_cache_key(&format!(
            "fetch:{}:{:?}:{}",
            "https://example.com",
            ExtractMode::Markdown,
            config.tools.web.fetch.max_chars
        ));
        write_cache(
            fetch_cache(),
            &cache_key,
            json!({
                "url": "https://example.com",
                "text": "cached body"
            }),
            60_000,
        );
        let ctx = test_ctx(config);
        let payload = web_fetch_impl(
            &ctx,
            &ToolArguments::new(json!({
                "url": "https://example.com"
            })),
        )
        .await
        .expect("web_fetch response");
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["tool"], json!(WEB_FETCH_TOOL_NAME));
        assert_eq!(payload["data"]["cached"], json!(true));
    }

    #[tokio::test]
    async fn web_search_cached_payload_returns_success_envelope() {
        let _guard = test_lock().lock().expect("test lock");
        clear_caches();
        let mut config = HomieConfig::default();
        config.tools.web.search.enabled = true;
        let count = config.tools.web.search.max_results;
        let cache_key = normalize_cache_key(&format!(
            "search:{}:{}:{:?}:{:?}:{:?}:{:?}:{:?}",
            "brave",
            "rust",
            count,
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
        ));
        write_cache(
            search_cache(),
            &cache_key,
            json!({
                "query": "rust",
                "provider": "brave",
                "count": 1,
                "results": [
                    {
                        "title": "Rust",
                        "url": "https://www.rust-lang.org",
                        "snippet": "Rust language"
                    }
                ]
            }),
            60_000,
        );
        let ctx = test_ctx(config);
        let payload = web_search_impl(
            &ctx,
            &ToolArguments::new(json!({
                "query": "rust"
            })),
        )
        .await
        .expect("web_search response");
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["tool"], json!(WEB_SEARCH_TOOL_NAME));
        assert_eq!(payload["data"]["cached"], json!(true));
    }

    #[test]
    fn web_fetch_backend_default_is_auto() {
        assert_eq!(WebFetchBackend::default(), WebFetchBackend::Auto);
    }

    #[test]
    fn web_fetch_backend_deserialize_variants() {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            backend: WebFetchBackend,
        }
        let native: Wrapper = toml::from_str(r#"backend = "native""#).expect("native");
        assert_eq!(native.backend, WebFetchBackend::Native);

        let firecrawl: Wrapper = toml::from_str(r#"backend = "firecrawl""#).expect("firecrawl");
        assert_eq!(firecrawl.backend, WebFetchBackend::Firecrawl);

        let auto: Wrapper = toml::from_str(r#"backend = "auto""#).expect("auto");
        assert_eq!(auto.backend, WebFetchBackend::Auto);
    }

    #[test]
    fn web_fetch_backend_unknown_falls_back_to_auto() {
        // Unknown variant should fail to deserialize; config uses #[serde(default)]
        // so a missing field falls back to Auto.
        #[derive(serde::Deserialize)]
        struct Wrapper {
            #[serde(default)]
            backend: WebFetchBackend,
        }
        let missing: Wrapper = toml::from_str("").expect("missing field uses default");
        assert_eq!(missing.backend, WebFetchBackend::Auto);

        // Explicit unknown string should be a parse error
        let unknown = toml::from_str::<Wrapper>(r#"backend = "banana""#);
        assert!(unknown.is_err(), "unknown variant should fail to parse");
    }

    #[test]
    fn web_fetch_payload_includes_backend_field() {
        let start = Instant::now();
        let payload = build_fetch_payload(
            "https://example.com",
            "https://example.com",
            200,
            "text/html",
            Some("Example"),
            ExtractMode::Markdown,
            "readability",
            "native",
            "hello world",
            50_000,
            start,
            None,
        );
        assert_eq!(payload["backend"], json!("native"));

        let payload_fc = build_fetch_payload(
            "https://example.com",
            "https://example.com",
            200,
            "text/markdown",
            None,
            ExtractMode::Markdown,
            "firecrawl",
            "firecrawl",
            "hello world",
            50_000,
            start,
            None,
        );
        assert_eq!(payload_fc["backend"], json!("firecrawl"));
    }

    #[tokio::test]
    async fn web_fetch_backend_resolve_logic() {
        // Native always returns Native regardless of base_url
        assert_eq!(
            resolve_backend(WebFetchBackend::Native, "https://fc.local", None, 30).await,
            ResolvedBackend::Native
        );

        // Firecrawl with empty base_url falls back to Native
        assert_eq!(
            resolve_backend(WebFetchBackend::Firecrawl, "", None, 30).await,
            ResolvedBackend::Native
        );
        assert_eq!(
            resolve_backend(WebFetchBackend::Firecrawl, "   ", None, 30).await,
            ResolvedBackend::Native
        );

        // Firecrawl with non-empty base_url returns Firecrawl
        assert_eq!(
            resolve_backend(
                WebFetchBackend::Firecrawl,
                "https://fc.local",
                None,
                30
            )
            .await,
            ResolvedBackend::Firecrawl
        );

        // Auto with no base_url returns Native (health check skipped)
        assert_eq!(
            resolve_backend(WebFetchBackend::Auto, "", None, 30).await,
            ResolvedBackend::Native
        );
    }
}

use std::time::{Duration, Instant};

use roci::error::RociError;
use roci::tools::ToolArguments;
use serde::Deserialize;

use crate::homie_config::{BraveSearchConfig, SearxngSearchConfig, WebSearchConfig};

use super::super::ToolContext;
use super::cache::{normalize_cache_key, read_cache, search_cache, write_cache};
use super::shared::{resolve_site_name, truncate_str};
use super::{error_envelope_from_roci, wrap_tool_payload, WEB_SEARCH_TOOL_NAME};

const MAX_SEARCH_COUNT: usize = 10;
const DEFAULT_ERROR_MAX_CHARS: usize = 4000;

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

pub(super) async fn web_search_impl(
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
    if let Ok(mut url) = url::Url::parse(base_url) {
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

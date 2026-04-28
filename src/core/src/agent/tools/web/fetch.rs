use std::net::IpAddr;
use std::time::{Duration, Instant};

use roci::error::RociError;
use roci::tools::ToolArguments;
use serde::Deserialize;
use tokio::net::lookup_host;
use url::{Host, Url};

use crate::homie_config::WebFetchConfig;

use super::super::{debug_tools_enabled, ToolContext};
use super::cache::{fetch_cache, normalize_cache_key, read_cache, write_cache};
use super::firecrawl::{
    fetch_firecrawl_content, resolve_firecrawl_api_key, resolve_firecrawl_base_url,
    resolve_firecrawl_enabled, ResolvedBackend,
};
use super::payload::{format_fetch_error_detail, ExtractMode};
use super::{error_envelope_from_roci, success_envelope, WEB_FETCH_TOOL_NAME};

pub(super) use super::firecrawl::resolve_backend;
pub(super) use super::payload::{build_fetch_payload, FetchPayloadArgs};

#[derive(Debug, Deserialize)]
struct WebFetchArgs {
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "extractMode")]
    extract_mode: Option<String>,
    #[serde(default, rename = "maxChars")]
    max_chars: Option<usize>,
}

pub(super) async fn web_fetch_impl(
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

    if resolved_backend == ResolvedBackend::Firecrawl {
        let parsed_url =
            Url::parse(url).map_err(|_| RociError::InvalidArgument("invalid url".into()))?;
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
                let payload = build_fetch_payload(FetchPayloadArgs {
                    url,
                    final_url: firecrawl.final_url.as_deref().unwrap_or(url),
                    status: firecrawl.status.unwrap_or(200),
                    content_type: "text/markdown",
                    title: firecrawl.title.as_deref(),
                    extract_mode,
                    extractor: "firecrawl",
                    backend: "firecrawl",
                    text: &firecrawl.text,
                    max_chars,
                    start,
                    warning: firecrawl.warning.as_deref(),
                });
                write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
                return Ok(payload);
            }
            Err(e) => {
                tracing::warn!("firecrawl primary fetch failed, falling back to native: {e}");
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| RociError::ToolExecution {
            tool_name: "web_fetch".into(),
            message: format!("failed to build http client: {e}"),
        })?;

    let fetch_result = fetch_with_redirects(&client, url, max_redirects, &user_agent).await;
    let allow_firecrawl_fallback = resolved_backend == ResolvedBackend::Native && firecrawl_enabled;

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
                let payload = build_fetch_payload(FetchPayloadArgs {
                    url,
                    final_url: firecrawl.final_url.as_deref().unwrap_or(url),
                    status: firecrawl.status.unwrap_or(200),
                    content_type: "text/markdown",
                    title: firecrawl.title.as_deref(),
                    extract_mode,
                    extractor: "firecrawl",
                    backend: "firecrawl",
                    text: &firecrawl.text,
                    max_chars,
                    start,
                    warning: firecrawl.warning.as_deref(),
                });
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
            let payload = build_fetch_payload(FetchPayloadArgs {
                url,
                final_url: firecrawl.final_url.as_deref().unwrap_or(url),
                status: firecrawl.status.unwrap_or(response.status().as_u16()),
                content_type: "text/markdown",
                title: firecrawl.title.as_deref(),
                extract_mode,
                extractor: "firecrawl",
                backend: "firecrawl",
                text: &firecrawl.text,
                max_chars,
                start,
                warning: firecrawl.warning.as_deref(),
            });
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
                let payload = build_fetch_payload(FetchPayloadArgs {
                    url,
                    final_url: firecrawl.final_url.as_deref().unwrap_or(url),
                    status: firecrawl.status.unwrap_or(200),
                    content_type: "text/markdown",
                    title: firecrawl.title.as_deref(),
                    extract_mode,
                    extractor: "firecrawl",
                    backend: "firecrawl",
                    text: &firecrawl.text,
                    max_chars,
                    start,
                    warning: firecrawl.warning.as_deref(),
                });
                write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
                return Ok(payload);
            } else {
                return Err(RociError::ToolExecution {
                    tool_name: "web_fetch".into(),
                    message:
                        "web fetch extraction failed: readability disabled and firecrawl unavailable"
                            .into(),
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

    let payload = build_fetch_payload(FetchPayloadArgs {
        url,
        final_url: &final_url,
        status,
        content_type: &content_type,
        title: title.as_deref(),
        extract_mode,
        extractor,
        backend: "native",
        text: &text,
        max_chars,
        start,
        warning: None,
    });
    write_cache(fetch_cache(), &cache_key, payload.clone(), cache_ttl_ms);
    Ok(payload)
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

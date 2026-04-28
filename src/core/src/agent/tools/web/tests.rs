use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use roci::tools::ToolArguments;
use serde_json::json;

use crate::homie_config::{HomieConfig, WebFetchBackend};

use super::super::ToolContext;
use super::cache::{fetch_cache, normalize_cache_key, search_cache, write_cache};
use super::fetch::{build_fetch_payload, resolve_backend, web_fetch_impl, FetchPayloadArgs};
use super::firecrawl::ResolvedBackend;
use super::payload::ExtractMode;
use super::search::web_search_impl;
use super::{WEB_FETCH_TOOL_NAME, WEB_SEARCH_TOOL_NAME};

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
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        backend: WebFetchBackend,
    }
    let missing: Wrapper = toml::from_str("").expect("missing field uses default");
    assert_eq!(missing.backend, WebFetchBackend::Auto);

    let unknown = toml::from_str::<Wrapper>(r#"backend = "banana""#);
    assert!(unknown.is_err(), "unknown variant should fail to parse");
}

#[test]
fn web_fetch_payload_includes_backend_field() {
    let start = Instant::now();
    let payload = build_fetch_payload(FetchPayloadArgs {
        url: "https://example.com",
        final_url: "https://example.com",
        status: 200,
        content_type: "text/html",
        title: Some("Example"),
        extract_mode: ExtractMode::Markdown,
        extractor: "readability",
        backend: "native",
        text: "hello world",
        max_chars: 50_000,
        start,
        warning: None,
    });
    assert_eq!(payload["backend"], json!("native"));

    let payload_fc = build_fetch_payload(FetchPayloadArgs {
        url: "https://example.com",
        final_url: "https://example.com",
        status: 200,
        content_type: "text/markdown",
        title: None,
        extract_mode: ExtractMode::Markdown,
        extractor: "firecrawl",
        backend: "firecrawl",
        text: "hello world",
        max_chars: 50_000,
        start,
        warning: None,
    });
    assert_eq!(payload_fc["backend"], json!("firecrawl"));
}

#[tokio::test]
async fn web_fetch_backend_resolve_logic() {
    assert_eq!(
        resolve_backend(WebFetchBackend::Native, "https://fc.local", None, 30).await,
        ResolvedBackend::Native
    );

    assert_eq!(
        resolve_backend(WebFetchBackend::Firecrawl, "", None, 30).await,
        ResolvedBackend::Native
    );
    assert_eq!(
        resolve_backend(WebFetchBackend::Firecrawl, "   ", None, 30).await,
        ResolvedBackend::Native
    );

    assert_eq!(
        resolve_backend(WebFetchBackend::Firecrawl, "https://fc.local", None, 30).await,
        ResolvedBackend::Firecrawl
    );

    assert_eq!(
        resolve_backend(WebFetchBackend::Auto, "", None, 30).await,
        ResolvedBackend::Native
    );
}

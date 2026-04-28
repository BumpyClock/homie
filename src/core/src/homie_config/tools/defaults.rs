use std::collections::HashMap;

pub(crate) fn default_web_fetch_enabled() -> bool {
    true
}

pub(crate) fn default_web_fetch_max_chars() -> usize {
    50_000
}

pub(crate) fn default_web_fetch_timeout_seconds() -> u64 {
    30
}

pub(crate) fn default_web_fetch_cache_ttl_minutes() -> u64 {
    15
}

pub(crate) fn default_web_fetch_max_redirects() -> usize {
    3
}

pub(crate) fn default_web_fetch_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".to_string()
}

pub(crate) fn default_web_fetch_readability() -> bool {
    true
}

pub(crate) fn default_firecrawl_enabled() -> bool {
    false
}

pub(crate) fn default_firecrawl_base_url() -> String {
    "https://api.firecrawl.dev".to_string()
}

pub(crate) fn default_firecrawl_only_main_content() -> bool {
    true
}

pub(crate) fn default_firecrawl_max_age_ms() -> u64 {
    172_800_000
}

pub(crate) fn default_firecrawl_timeout_seconds() -> u64 {
    30
}

pub(crate) fn default_web_search_enabled() -> bool {
    false
}

pub(crate) fn default_web_search_provider() -> String {
    "brave".to_string()
}

pub(crate) fn default_web_search_timeout_seconds() -> u64 {
    30
}

pub(crate) fn default_web_search_cache_ttl_minutes() -> u64 {
    15
}

pub(crate) fn default_web_search_max_results() -> usize {
    5
}

pub(crate) fn default_brave_search_endpoint() -> String {
    "https://api.search.brave.com/res/v1/web/search".to_string()
}

pub(crate) fn default_searxng_api_key_header() -> String {
    "X-API-Key".to_string()
}

pub(crate) fn default_searxng_headers() -> HashMap<String, String> {
    HashMap::new()
}

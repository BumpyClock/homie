use std::collections::HashMap;

use serde::{de::Error as _, Deserialize, Deserializer};

use super::{
    default_brave_search_endpoint, default_firecrawl_base_url, default_firecrawl_enabled,
    default_firecrawl_max_age_ms, default_firecrawl_only_main_content,
    default_firecrawl_timeout_seconds, default_searxng_api_key_header,
    default_web_fetch_cache_ttl_minutes, default_web_fetch_enabled, default_web_fetch_max_chars,
    default_web_fetch_max_redirects, default_web_fetch_readability,
    default_web_fetch_timeout_seconds, default_web_fetch_user_agent,
    default_web_search_cache_ttl_minutes, default_web_search_enabled,
    default_web_search_max_results, default_web_search_provider,
    default_web_search_timeout_seconds,
};

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

pub(crate) fn deserialize_web_fetch_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_fetch_enabled()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_max_chars<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_fetch_max_chars()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_timeout_seconds<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_fetch_timeout_seconds()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_cache_ttl_minutes<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_fetch_cache_ttl_minutes()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_max_redirects<'de, D>(
    deserializer: D,
) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_fetch_max_redirects()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_readability<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_fetch_readability()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_fetch_user_agent<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_web_fetch_user_agent(),
    ))
}

pub(crate) fn deserialize_firecrawl_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_firecrawl_enabled()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_firecrawl_only_main_content<'de, D>(
    deserializer: D,
) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_firecrawl_only_main_content()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_firecrawl_max_age_ms<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_firecrawl_max_age_ms()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_firecrawl_timeout_seconds<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_firecrawl_timeout_seconds()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_firecrawl_base_url<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(value, default_firecrawl_base_url()))
}

pub(crate) fn deserialize_web_search_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = BoolOrString::deserialize(deserializer)?;
    parse_bool_or_default(value, default_web_search_enabled()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_search_timeout_seconds<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_search_timeout_seconds()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_search_cache_ttl_minutes<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = U64OrString::deserialize(deserializer)?;
    parse_u64_or_default(value, default_web_search_cache_ttl_minutes()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_search_max_results<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = UsizeOrString::deserialize(deserializer)?;
    parse_usize_or_default(value, default_web_search_max_results()).map_err(D::Error::custom)
}

pub(crate) fn deserialize_web_search_provider<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_web_search_provider(),
    ))
}

pub(crate) fn deserialize_brave_search_endpoint<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_brave_search_endpoint(),
    ))
}

pub(crate) fn deserialize_searxng_api_key_header<'de, D>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_string_or_default(
        value,
        default_searxng_api_key_header(),
    ))
}

pub(crate) fn deserialize_searxng_headers<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, String>, D::Error>
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

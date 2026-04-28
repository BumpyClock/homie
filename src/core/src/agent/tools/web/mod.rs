use std::sync::Arc;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};

use super::ToolContext;

mod cache;
mod fetch;
mod firecrawl;
mod payload;
mod search;
mod shared;
#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests;

pub(super) const WEB_FETCH_TOOL_NAME: &str = "web_fetch";
pub(super) const WEB_SEARCH_TOOL_NAME: &str = "web_search";

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
            async move { fetch::web_fetch_impl(&ctx, &args).await }
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
            async move { search::web_search_impl(&ctx, &args).await }
        },
    )))
}

pub(super) fn wrap_tool_payload(tool_name: &str, payload: serde_json::Value) -> serde_json::Value {
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

pub(super) fn success_envelope(tool_name: &str, data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "tool": tool_name,
        "data": data
    })
}

pub(super) fn error_envelope_from_roci(tool_name: &str, err: RociError) -> serde_json::Value {
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

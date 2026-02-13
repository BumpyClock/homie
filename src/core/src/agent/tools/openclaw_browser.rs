use std::sync::Arc;
use std::time::Duration;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use serde_json::Value;

use super::registry::ToolProvider;
use super::ToolContext;

pub const PROVIDER_ID: &str = "openclaw_browser";
const TOOL_NAME: &str = "browser";
const DEFAULT_TIMEOUT_MS: u64 = 20_000;

const ACTIONS: [&str; 16] = [
    "status",
    "start",
    "stop",
    "profiles",
    "tabs",
    "open",
    "focus",
    "close",
    "snapshot",
    "screenshot",
    "navigate",
    "console",
    "pdf",
    "upload",
    "dialog",
    "act",
];

const TARGETS: [&str; 3] = ["sandbox", "host", "node"];
const SNAPSHOT_FORMATS: [&str; 2] = ["aria", "ai"];
const SNAPSHOT_MODES: [&str; 1] = ["efficient"];
const SNAPSHOT_REFS: [&str; 2] = ["role", "aria"];
const IMAGE_TYPES: [&str; 2] = ["png", "jpeg"];

pub struct OpenClawBrowserProvider;

impl ToolProvider for OpenClawBrowserProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn is_dynamic(&self) -> bool {
        true
    }

    fn tools(&self, ctx: ToolContext) -> Vec<Arc<dyn Tool>> {
        vec![openclaw_browser_tool(ctx)]
    }
}

pub fn openclaw_browser_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = openclaw_browser_schema();
    Arc::new(AgentTool::new(
        TOOL_NAME,
        "OpenClaw browser control via configured endpoint.",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { Ok(openclaw_browser_execute(&ctx, &args).await) }
        },
    ))
}

fn openclaw_browser_schema() -> AgentToolParameters {
    AgentToolParameters::from_schema(serde_json::json!({
        "type": "object",
        "additionalProperties": true,
        "properties": {
            "action": {
                "type": "string",
                "description": "Browser action (status/start/stop/profiles/tabs/open/focus/close/snapshot/screenshot/navigate/console/pdf/upload/dialog/act).",
                "enum": ACTIONS,
            },
            "target": {
                "type": "string",
                "description": "Browser target (sandbox/host/node).",
                "enum": TARGETS,
            },
            "node": { "type": "string", "description": "Node identifier for browser proxy." },
            "profile": { "type": "string", "description": "Browser profile name." },
            "targetUrl": { "type": "string", "description": "Target URL for open/navigate." },
            "targetId": { "type": "string", "description": "Target tab id." },
            "limit": { "type": "number", "description": "Snapshot node limit." },
            "maxChars": { "type": "number", "description": "Snapshot max chars." },
            "mode": { "type": "string", "enum": SNAPSHOT_MODES },
            "snapshotFormat": { "type": "string", "enum": SNAPSHOT_FORMATS },
            "refs": { "type": "string", "enum": SNAPSHOT_REFS },
            "interactive": { "type": "boolean" },
            "compact": { "type": "boolean" },
            "depth": { "type": "number" },
            "selector": { "type": "string" },
            "frame": { "type": "string" },
            "labels": { "type": "boolean" },
            "fullPage": { "type": "boolean" },
            "ref": { "type": "string" },
            "element": { "type": "string" },
            "type": { "type": "string", "enum": IMAGE_TYPES },
            "level": { "type": "string" },
            "paths": { "type": "array", "items": { "type": "string" } },
            "inputRef": { "type": "string" },
            "timeoutMs": { "type": "number" },
            "accept": { "type": "boolean" },
            "promptText": { "type": "string" },
            "request": { "type": "object", "additionalProperties": true },
        },
        "required": ["action"],
    }))
}

async fn openclaw_browser_execute(ctx: &ToolContext, args: &ToolArguments) -> Value {
    let params = match args.deserialize::<Value>() {
        Ok(value) => value,
        Err(err) => return error_envelope_from_roci(err),
    };
    let map = match params.as_object() {
        Some(map) => map,
        None => return error_envelope("invalid_argument", "arguments must be an object", false, None),
    };
    let endpoint = ctx.openclaw_browser.endpoint.trim();
    if endpoint.is_empty() {
        return error_envelope(
            "not_configured",
            "OpenClaw browser provider is not configured",
            false,
            None,
        );
    }
    let action = match read_string(map, "action") {
        Some(value) => value.to_ascii_lowercase(),
        None => return error_envelope("invalid_argument", "action is required", false, None),
    };
    if !ACTIONS.contains(&action.as_str()) {
        return error_envelope(
            "invalid_argument",
            format!("unknown action: {action}"),
            false,
            None,
        );
    }
    let timeout_ms = read_number_alias(map, &["timeoutMs", "timeout_ms"])
        .and_then(|value| to_timeout_ms(value))
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    let base_url = normalize_base_url(endpoint);
    let api_key = ctx.openclaw_browser.api_key.trim();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return error_envelope(
                "tool_error",
                format!("failed to build http client: {err}"),
                false,
                None,
            )
        }
    };
    let profile = read_string(map, "profile");
    let requests = match build_requests(&action, map, profile.as_deref()) {
        Ok(requests) => requests,
        Err(err) => return err,
    };
    let mut last = None;
    for request in requests {
        match request_json(
            &client,
            api_key,
            &base_url,
            request.method,
            &request.path,
            request.query,
            request.body,
        )
        .await
        {
            Ok(payload) => last = Some(payload),
            Err(err) => {
                return error_envelope(err.code, err.message, err.retryable, err.details);
            }
        }
    }
    match last {
        Some(payload) => wrap_tool_payload(payload),
        None => error_envelope("tool_error", "no request executed", false, None),
    }
}

fn wrap_tool_payload(payload: Value) -> Value {
    let code = payload
        .as_object()
        .and_then(|obj| obj.get("error"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    if let Some(code) = code {
        let message = payload
            .as_object()
            .and_then(|obj| obj.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or("tool request failed")
            .to_string();
        return error_envelope(code.as_str(), message, false, Some(payload));
    }
    success_envelope(payload)
}

fn success_envelope(data: Value) -> Value {
    serde_json::json!({
        "ok": true,
        "tool": TOOL_NAME,
        "data": data
    })
}

fn error_envelope_from_roci(err: RociError) -> Value {
    let retryable = err.is_retryable();
    match err {
        RociError::InvalidArgument(message) => {
            error_envelope("invalid_argument", message, retryable, None)
        }
        RociError::Timeout(timeout_ms) => error_envelope(
            "timeout",
            format!("request timed out after {timeout_ms}ms"),
            retryable,
            None,
        ),
        RociError::Network(message) => {
            error_envelope("network_error", message.to_string(), retryable, None)
        }
        RociError::Serialization(message) => {
            error_envelope("serialization_error", message.to_string(), retryable, None)
        }
        RociError::ToolExecution { message, .. } => {
            error_envelope("tool_execution_failed", message, retryable, None)
        }
        other => error_envelope("tool_error", other.to_string(), retryable, None),
    }
}

fn error_envelope(
    code: &str,
    message: impl Into<String>,
    retryable: bool,
    details: Option<Value>,
) -> Value {
    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), Value::String(code.to_string()));
    error.insert("message".to_string(), Value::String(message.into()));
    error.insert("retryable".to_string(), Value::Bool(retryable));
    if let Some(details) = details {
        error.insert("details".to_string(), details);
    }
    serde_json::json!({
        "ok": false,
        "tool": TOOL_NAME,
        "error": error
    })
}

struct RequestSpec {
    method: reqwest::Method,
    path: String,
    query: Vec<(String, String)>,
    body: Option<Value>,
}

struct RequestError {
    code: &'static str,
    message: String,
    retryable: bool,
    details: Option<Value>,
}

fn build_requests(
    action: &str,
    map: &serde_json::Map<String, Value>,
    profile: Option<&str>,
) -> Result<Vec<RequestSpec>, Value> {
    match action {
        "status" => Ok(vec![RequestSpec {
            method: reqwest::Method::GET,
            path: "/".to_string(),
            query: query_with_profile(profile, None),
            body: None,
        }]),
        "start" => Ok(vec![
            RequestSpec {
                method: reqwest::Method::POST,
                path: "/start".to_string(),
                query: query_with_profile(profile, None),
                body: None,
            },
            RequestSpec {
                method: reqwest::Method::GET,
                path: "/".to_string(),
                query: query_with_profile(profile, None),
                body: None,
            },
        ]),
        "stop" => Ok(vec![
            RequestSpec {
                method: reqwest::Method::POST,
                path: "/stop".to_string(),
                query: query_with_profile(profile, None),
                body: None,
            },
            RequestSpec {
                method: reqwest::Method::GET,
                path: "/".to_string(),
                query: query_with_profile(profile, None),
                body: None,
            },
        ]),
        "profiles" => Ok(vec![RequestSpec {
            method: reqwest::Method::GET,
            path: "/profiles".to_string(),
            query: Vec::new(),
            body: None,
        }]),
        "tabs" => Ok(vec![RequestSpec {
            method: reqwest::Method::GET,
            path: "/tabs".to_string(),
            query: query_with_profile(profile, None),
            body: None,
        }]),
        "open" => {
            let target_url = read_string_alias(map, &["targetUrl", "url"]);
            let target_url = match target_url {
                Some(value) => value,
                None => {
                    return Err(error_envelope(
                        "invalid_argument",
                        "targetUrl is required",
                        false,
                        None,
                    ))
                }
            };
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/tabs/open".to_string(),
                query: query_with_profile(profile, None),
                body: Some(serde_json::json!({ "url": target_url })),
            }])
        }
        "focus" => {
            let target_id = match read_string(map, "targetId") {
                Some(value) => value,
                None => {
                    return Err(error_envelope(
                        "invalid_argument",
                        "targetId is required",
                        false,
                        None,
                    ))
                }
            };
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/tabs/focus".to_string(),
                query: query_with_profile(profile, None),
                body: Some(serde_json::json!({ "targetId": target_id })),
            }])
        }
        "close" => {
            let target_id = read_string(map, "targetId");
            if let Some(target_id) = target_id {
                Ok(vec![RequestSpec {
                    method: reqwest::Method::DELETE,
                    path: format!("/tabs/{}", encode_path_segment(&target_id)),
                    query: query_with_profile(profile, None),
                    body: None,
                }])
            } else {
                Ok(vec![RequestSpec {
                    method: reqwest::Method::POST,
                    path: "/act".to_string(),
                    query: query_with_profile(profile, None),
                    body: Some(serde_json::json!({ "kind": "close" })),
                }])
            }
        }
        "snapshot" => {
            let format =
                read_string_alias(map, &["snapshotFormat", "format"]).unwrap_or_else(|| "ai".to_string());
            let format = if format == "aria" { "aria" } else { "ai" };
            let mode = read_string(map, "mode");
            let target_id = read_string(map, "targetId");
            let limit = read_number(map, "limit");
            let max_chars = read_number(map, "maxChars");
            let refs = read_string(map, "refs");
            let interactive = read_bool(map, "interactive");
            let compact = read_bool(map, "compact");
            let depth = read_number(map, "depth");
            let selector = read_string(map, "selector");
            let frame = read_string(map, "frame");
            let labels = read_bool(map, "labels");
            let mut query = Vec::new();
            query.push(("format".to_string(), format.to_string()));
            if let Some(target_id) = target_id {
                query.push(("targetId".to_string(), target_id));
            }
            if let Some(limit) = limit {
                query.push(("limit".to_string(), limit.to_string()));
            }
            if let Some(max_chars) = max_chars {
                query.push(("maxChars".to_string(), max_chars.to_string()));
            }
            if let Some(refs) = refs {
                if refs == "role" || refs == "aria" {
                    query.push(("refs".to_string(), refs));
                }
            }
            if let Some(interactive) = interactive {
                query.push(("interactive".to_string(), interactive.to_string()));
            }
            if let Some(compact) = compact {
                query.push(("compact".to_string(), compact.to_string()));
            }
            if let Some(depth) = depth {
                query.push(("depth".to_string(), depth.to_string()));
            }
            if let Some(selector) = selector {
                if !selector.is_empty() {
                    query.push(("selector".to_string(), selector));
                }
            }
            if let Some(frame) = frame {
                if !frame.is_empty() {
                    query.push(("frame".to_string(), frame));
                }
            }
            if let Some(labels) = labels {
                if labels {
                    query.push(("labels".to_string(), "1".to_string()));
                }
            }
            if let Some(mode) = mode {
                if mode == "efficient" {
                    query.push(("mode".to_string(), mode));
                }
            }
            Ok(vec![RequestSpec {
                method: reqwest::Method::GET,
                path: "/snapshot".to_string(),
                query: query_with_profile(profile, Some(query)),
                body: None,
            }])
        }
        "screenshot" => {
            let target_id = read_string(map, "targetId");
            let full_page = read_bool(map, "fullPage").unwrap_or(false);
            let ref_value = read_string(map, "ref");
            let element = read_string(map, "element");
            let image_type = read_string(map, "type");
            let body = serde_json::json!({
                "targetId": target_id,
                "fullPage": full_page,
                "ref": ref_value,
                "element": element,
                "type": if matches!(image_type.as_deref(), Some("jpeg")) { "jpeg" } else { "png" },
            });
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/screenshot".to_string(),
                query: query_with_profile(profile, None),
                body: Some(body),
            }])
        }
        "navigate" => {
            let target_url = read_string_alias(map, &["targetUrl", "url"]);
            let target_url = match target_url {
                Some(value) => value,
                None => {
                    return Err(error_envelope(
                        "invalid_argument",
                        "targetUrl is required",
                        false,
                        None,
                    ))
                }
            };
            let target_id = read_string(map, "targetId");
            let body = serde_json::json!({
                "url": target_url,
                "targetId": target_id,
            });
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/navigate".to_string(),
                query: query_with_profile(profile, None),
                body: Some(body),
            }])
        }
        "console" => {
            let level = read_string(map, "level");
            let target_id = read_string(map, "targetId");
            let mut query = Vec::new();
            if let Some(level) = level {
                query.push(("level".to_string(), level));
            }
            if let Some(target_id) = target_id {
                query.push(("targetId".to_string(), target_id));
            }
            Ok(vec![RequestSpec {
                method: reqwest::Method::GET,
                path: "/console".to_string(),
                query: query_with_profile(profile, Some(query)),
                body: None,
            }])
        }
        "pdf" => {
            let target_id = read_string(map, "targetId");
            let body = serde_json::json!({ "targetId": target_id });
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/pdf".to_string(),
                query: query_with_profile(profile, None),
                body: Some(body),
            }])
        }
        "upload" => {
            let paths = read_string_array(map, "paths");
            let paths = match paths {
                Some(values) if !values.is_empty() => values,
                _ => {
                    return Err(error_envelope(
                        "invalid_argument",
                        "paths are required",
                        false,
                        None,
                    ))
                }
            };
            let body = serde_json::json!({
                "paths": paths,
                "ref": read_string(map, "ref"),
                "inputRef": read_string(map, "inputRef"),
                "element": read_string(map, "element"),
                "targetId": read_string(map, "targetId"),
                "timeoutMs": read_number_alias(map, &["timeoutMs", "timeout_ms"]),
            });
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/hooks/file-chooser".to_string(),
                query: query_with_profile(profile, None),
                body: Some(body),
            }])
        }
        "dialog" => {
            let accept = read_bool(map, "accept").unwrap_or(false);
            let body = serde_json::json!({
                "accept": accept,
                "promptText": read_string(map, "promptText"),
                "targetId": read_string(map, "targetId"),
                "timeoutMs": read_number_alias(map, &["timeoutMs", "timeout_ms"]),
            });
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/hooks/dialog".to_string(),
                query: query_with_profile(profile, None),
                body: Some(body),
            }])
        }
        "act" => {
            let request = map.get("request").cloned();
            let request = match request {
                Some(Value::Object(_)) => request,
                _ => {
                    return Err(error_envelope(
                        "invalid_argument",
                        "request is required",
                        false,
                        None,
                    ))
                }
            };
            Ok(vec![RequestSpec {
                method: reqwest::Method::POST,
                path: "/act".to_string(),
                query: query_with_profile(profile, None),
                body: request,
            }])
        }
        _ => Err(error_envelope(
            "invalid_argument",
            "action not supported",
            false,
            None,
        )),
    }
}

async fn request_json(
    client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    method: reqwest::Method,
    path: &str,
    query: Vec<(String, String)>,
    body: Option<Value>,
) -> Result<Value, RequestError> {
    let url = join_url(base_url, path)?;
    let mut req = client.request(method, url).header("Accept", "application/json");
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }
    if !query.is_empty() {
        req = req.query(&query);
    }
    if let Some(body) = body {
        req = req.json(&body);
    }
    let response = req.send().await.map_err(map_reqwest_error)?;
    let status = response.status();
    let text = response.text().await.map_err(map_reqwest_error)?;
    if !status.is_success() {
        let details = if text.trim().is_empty() {
            None
        } else if let Ok(json) = serde_json::from_str::<Value>(&text) {
            Some(json)
        } else {
            Some(Value::String(text.clone()))
        };
        let message = if let Some(Value::String(msg)) = details
            .as_ref()
            .and_then(|value| value.as_object())
            .and_then(|obj| obj.get("error"))
        {
            msg.clone()
        } else if text.trim().is_empty() {
            format!("browser request failed with status {}", status.as_u16())
        } else {
            format!(
                "browser request failed with status {}: {}",
                status.as_u16(),
                text.trim()
            )
        };
        return Err(RequestError {
            code: "http_error",
            message,
            retryable: status.is_server_error(),
            details,
        });
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str::<Value>(&text).map_err(|err| RequestError {
        code: "serialization_error",
        message: format!("failed to parse response: {err}"),
        retryable: false,
        details: Some(Value::String(text)),
    })
}

fn join_url(base_url: &str, path: &str) -> Result<String, RequestError> {
    let base = base_url.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if base.is_empty() {
        return Err(RequestError {
            code: "not_configured",
            message: "OpenClaw browser provider is not configured".to_string(),
            retryable: false,
            details: None,
        });
    }
    Ok(format!("{base}{path}"))
}

fn normalize_base_url(endpoint: &str) -> String {
    endpoint.trim_end_matches('/').to_string()
}

fn encode_path_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn query_with_profile(profile: Option<&str>, extra: Option<Vec<(String, String)>>) -> Vec<(String, String)> {
    let mut query = extra.unwrap_or_default();
    if let Some(profile) = profile {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            query.push(("profile".to_string(), trimmed.to_string()));
        }
    }
    query
}

fn map_reqwest_error(err: reqwest::Error) -> RequestError {
    if err.is_timeout() {
        return RequestError {
            code: "timeout",
            message: "request timed out".to_string(),
            retryable: true,
            details: None,
        };
    }
    RequestError {
        code: "network_error",
        message: err.to_string(),
        retryable: true,
        details: None,
    }
}

fn read_string(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_string_alias(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = read_string(map, key) {
            return Some(value);
        }
    }
    None
}

fn read_number(map: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    map.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    })
}

fn read_number_alias(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(value) = read_number(map, key) {
            return Some(value);
        }
    }
    None
}

fn read_bool(map: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    map.get(key).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::String(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn read_string_array(map: &serde_json::Map<String, Value>, key: &str) -> Option<Vec<String>> {
    let value = map.get(key)?;
    match value {
        Value::Array(values) => {
            let entries = values
                .iter()
                .filter_map(|entry| entry.as_str())
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>();
            if entries.is_empty() {
                None
            } else {
                Some(entries)
            }
        }
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(vec![trimmed.to_string()])
            }
        }
        _ => None,
    }
}

fn to_timeout_ms(value: f64) -> Option<u64> {
    if !value.is_finite() {
        return None;
    }
    let ms = value.round();
    if ms < 1.0 {
        return None;
    }
    if ms > u64::MAX as f64 {
        return None;
    }
    Some(ms as u64)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::homie_config::OpenClawBrowserToolsConfig;

    use super::{build_requests, openclaw_browser_tool, ToolContext};

    #[test]
    fn reports_not_configured_when_endpoint_missing() {
        let mut config = crate::HomieConfig::default();
        config.tools.openclaw_browser = OpenClawBrowserToolsConfig::default();
        let ctx = ToolContext::new(Arc::new(config));
        let tool = openclaw_browser_tool(ctx);
        let payload = futures::executor::block_on(tool.execute(
            &roci::tools::ToolArguments::new(json!({ "action": "status" })),
            &roci::tools::tool::ToolExecutionContext::default(),
        ))
        .expect("tool response");
        assert_eq!(payload["error"]["code"], "not_configured");
    }

    #[test]
    fn build_requests_for_status_includes_profile() {
        let params = json!({ "action": "status", "profile": "chrome" });
        let map = params.as_object().expect("params object");
        let requests = build_requests("status", map, Some("chrome")).expect("requests");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, reqwest::Method::GET);
        assert_eq!(request.path, "/");
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "profile" && value == "chrome"));
    }

    #[test]
    fn build_requests_for_open_includes_body() {
        let params = json!({ "action": "open", "targetUrl": "https://example.com" });
        let map = params.as_object().expect("params object");
        let requests = build_requests("open", map, None).expect("requests");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, reqwest::Method::POST);
        assert_eq!(request.path, "/tabs/open");
        let body = request.body.as_ref().expect("body");
        assert_eq!(body["url"], "https://example.com");
    }
}

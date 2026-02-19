// Parser-heavy tool module returns rich `RociError` variants; keep existing signatures stable.
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};

use super::args::ParsedToolArgs;
use super::{ProcessRegistry, ToolContext};
use crate::agent::tools::process_registry::ProcessStatus;

const DEFAULT_TAIL_BYTES: usize = 4000;

#[derive(Debug, PartialEq, Eq)]
struct ProcessRequest {
    action: String,
    id: Option<String>,
    tail: usize,
}

pub fn process_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("action", "list|status|output|kill", true)
        .string("id", "Process id (required for status/output/kill)", false)
        .number("tail", "Output tail bytes", false)
        .build();

    Arc::new(AgentTool::new(
        "process",
        "Manage background exec sessions",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { process_impl(&ctx.processes, &args).await }
        },
    ))
}

async fn process_impl(
    registry: &Arc<ProcessRegistry>,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_process_request(args)?;
    let action = parsed.action;
    let id = parsed.id;
    let tail = parsed.tail;

    if super::debug_tools_enabled() {
        tracing::debug!(
            action = %action,
            id = id.as_deref().unwrap_or(""),
            tail,
            "process tool invoked"
        );
    }

    match action.as_str() {
        "list" => {
            let list = registry.list(tail);
            let entries: Vec<serde_json::Value> = list.into_iter().map(process_info_json).collect();
            Ok(serde_json::json!({ "processes": entries }))
        }
        "status" => {
            let id = require_process_id("status", id.as_deref())?;
            registry.try_wait(&id);
            let info = registry
                .info(&id, tail)
                .ok_or_else(|| RociError::ToolExecution {
                    tool_name: "process".into(),
                    message: "process not found".into(),
                })?;
            Ok(process_info_json(info))
        }
        "output" => {
            let id = require_process_id("output", id.as_deref())?;
            registry.try_wait(&id);
            let info = registry
                .info(&id, tail)
                .ok_or_else(|| RociError::ToolExecution {
                    tool_name: "process".into(),
                    message: "process not found".into(),
                })?;
            Ok(serde_json::json!({
                "id": info.id,
                "status": match info.status {
                    ProcessStatus::Running => "running",
                    ProcessStatus::Exited => "exited",
                },
                "exit_code": info.exit_code,
                "output": info.output_tail,
            }))
        }
        "kill" => {
            let id = require_process_id("kill", id.as_deref())?;
            registry
                .kill(&id)
                .await
                .map_err(|e| RociError::ToolExecution {
                    tool_name: "process".into(),
                    message: e,
                })?;
            Ok(serde_json::json!({ "status": "killed", "id": id }))
        }
        _ => Err(RociError::InvalidArgument(
            "action must be one of list, status, output, kill".into(),
        )),
    }
}

fn parse_process_request(args: &ToolArguments) -> Result<ProcessRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let action = clean_string(parsed.get_string("action")?)
        .or_else(|| clean_literal(parsed.literal()))
        .unwrap_or_else(|| "list".to_string());
    let action = normalize_action(action.as_str())?;
    let id = clean_string(parsed.get_string_any(&["id", "process_id", "processId"])?);
    let tail = parsed
        .get_usize_any(&["tail", "tail_bytes", "tailBytes"])?
        .unwrap_or(DEFAULT_TAIL_BYTES);
    Ok(ProcessRequest { action, id, tail })
}

fn normalize_action(action: &str) -> Result<String, RociError> {
    let normalized = action.trim().to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "list" | "ls" => "list",
        "status" | "stat" | "info" => "status",
        "output" | "out" | "logs" | "log" | "tail" => "output",
        "kill" | "stop" | "terminate" => "kill",
        _ => {
            return Err(RociError::InvalidArgument(
                "action must be one of list, status, output, kill".into(),
            ));
        }
    };
    Ok(canonical.to_string())
}

fn require_process_id(action: &str, id: Option<&str>) -> Result<String, RociError> {
    let Some(process_id) = clean_literal(id) else {
        return Err(RociError::InvalidArgument(format!(
            "id is required for action={action}"
        )));
    };
    Ok(process_id)
}

fn clean_string(value: Option<String>) -> Option<String> {
    value
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
}

fn clean_literal(value: Option<&str>) -> Option<String> {
    value
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
}

fn process_info_json(info: super::ProcessInfo) -> serde_json::Value {
    serde_json::json!({
        "id": info.id,
        "command": info.command,
        "cwd": info.cwd,
        "started_at": info.started_at.to_rfc3339(),
        "pid": info.pid,
        "status": match info.status {
            ProcessStatus::Running => "running",
            ProcessStatus::Exited => "exited",
        },
        "exit_code": info.exit_code,
        "output_tail": info.output_tail,
    })
}

#[cfg(test)]
mod tests {
    use roci::tools::ToolArguments;
    use serde_json::json;

    use super::{parse_process_request, require_process_id, DEFAULT_TAIL_BYTES};

    #[test]
    fn process_request_defaults_missing_action_to_list() {
        let args = ToolArguments::new(json!({}));
        let parsed = parse_process_request(&args).expect("parse process request");
        assert_eq!(parsed.action, "list");
        assert_eq!(parsed.id, None);
        assert_eq!(parsed.tail, DEFAULT_TAIL_BYTES);
    }

    #[test]
    fn process_request_accepts_aliases_and_numeric_tail_string() {
        let args = ToolArguments::new(json!({
            "action": "logs",
            "process_id": "abc123",
            "tailBytes": "1024"
        }));
        let parsed = parse_process_request(&args).expect("parse process request");
        assert_eq!(parsed.action, "output");
        assert_eq!(parsed.id.as_deref(), Some("abc123"));
        assert_eq!(parsed.tail, 1024);
    }

    #[test]
    fn process_request_rejects_unknown_action_with_clear_error() {
        let args = ToolArguments::new(json!({ "action": "restart" }));
        let err = parse_process_request(&args).expect_err("unknown action should fail");
        assert_eq!(
            err.to_string(),
            "Invalid argument: action must be one of list, status, output, kill"
        );
    }

    #[test]
    fn process_id_error_is_clear_for_status_action() {
        let err = require_process_id("status", None).expect_err("missing id should fail");
        assert_eq!(
            err.to_string(),
            "Invalid argument: id is required for action=status"
        );
    }
}

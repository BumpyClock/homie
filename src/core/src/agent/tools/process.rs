use std::sync::Arc;

use roci::error::RociError;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use roci::tools::tool::ToolExecutionContext;
use serde::Deserialize;

use super::{ProcessRegistry, ToolContext};
use crate::agent::tools::process_registry::ProcessStatus;

const DEFAULT_TAIL_BYTES: usize = 4000;

#[derive(Debug, Deserialize)]
struct ProcessArgs {
    action: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    tail: Option<usize>,
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
    let parsed: ProcessArgs = args.deserialize()?;
    let action = parsed.action.trim().to_lowercase();
    let tail = parsed.tail.unwrap_or(DEFAULT_TAIL_BYTES);

    if super::debug_tools_enabled() {
        tracing::debug!(
            action = %action,
            id = parsed.id.as_deref().unwrap_or(""),
            tail,
            "process tool invoked"
        );
    }

    match action.as_str() {
        "list" => {
            let list = registry.list(tail);
            let entries: Vec<serde_json::Value> = list
                .into_iter()
                .map(|info| process_info_json(info))
                .collect();
            Ok(serde_json::json!({ "processes": entries }))
        }
        "status" => {
            let id = parsed.id.ok_or_else(|| {
                RociError::InvalidArgument("process id required".into())
            })?;
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
            let id = parsed.id.ok_or_else(|| {
                RociError::InvalidArgument("process id required".into())
            })?;
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
            let id = parsed.id.ok_or_else(|| {
                RociError::InvalidArgument("process id required".into())
            })?;
            registry.kill(&id).await.map_err(|e| RociError::ToolExecution {
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

use std::sync::Arc;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use uuid::Uuid;

use crate::storage::{CronRecord, CronStatus, Store};

use super::args::ParsedToolArgs;
use super::ToolContext;

#[derive(Debug)]
struct CronToolParams {
    action: String,
    cron_id: Option<String>,
    name: Option<String>,
    schedule: Option<String>,
    command: Option<String>,
    skip_overlap: Option<bool>,
    status: Option<String>,
    limit: Option<usize>,
}

pub fn cron_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("action", "create|list|status|cancel|runs", false)
        .string("cron_id", "Cron identifier for status/cancel/runs", false)
        .string("name", "Cron name", false)
        .string("schedule", "Cron schedule expression", false)
        .string("command", "Cron command", false)
        .boolean("skip_overlap", "Skip overlapping runs", false)
        .number("limit", "Run list limit", false)
        .string("status", "Cron status for create", false)
        .build();

    Arc::new(AgentTool::new(
        "cron",
        "Manage cron jobs and run records",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { cron_impl(&ctx, &args).await }
        },
    ))
}

async fn cron_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let params = parse_cron_tool_params(args)?;
    let action = normalize_action(&params.action)?;
    let store = ctx
        .store
        .as_deref()
        .ok_or_else(|| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: "cron tool requires store injection".into(),
        })?;

    match action.as_str() {
        "create" => create_cron(store, params),
        "list" => list_crons(store),
        "status" => cron_status(store, params.cron_id),
        "cancel" => cancel_cron(store, params.cron_id),
        "runs" => cron_runs(store, params.cron_id, params.limit),
        _ => Err(RociError::InvalidArgument(format!(
            "unsupported action: {action}"
        ))),
    }
}

fn parse_cron_tool_params(args: &ToolArguments) -> Result<CronToolParams, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let action = clean_string(parsed.get_string_any(&["action", "method"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .unwrap_or_else(|| "list".to_string());

    Ok(CronToolParams {
        action,
        cron_id: clean_string(parsed.get_string_any(&["cron_id", "id", "cronId"])?),
        name: clean_string(parsed.get_string_any(&["name"])?),
        schedule: clean_string(parsed.get_string_any(&["schedule", "cron_schedule"])?),
        command: clean_string(parsed.get_string_any(&["command", "cmd", "command_line"])?),
        skip_overlap: parsed.get_bool_any(&["skip_overlap", "skipOverlap"])?,
        status: clean_string(parsed.get_string_any(&["status"])?),
        limit: parsed.get_usize_any(&["limit", "max", "count"])?,
    })
}

fn normalize_action(action: &str) -> Result<String, RociError> {
    match action.trim().to_ascii_lowercase().as_str() {
        "create" | "add" | "start" => Ok("create".to_string()),
        "list" => Ok("list".to_string()),
        "status" | "read" => Ok("status".to_string()),
        "cancel" | "pause" | "stop" => Ok("cancel".to_string()),
        "runs" | "log" | "logs" | "tail" => Ok("runs".to_string()),
        _ => Err(RociError::InvalidArgument(
            "action must be one of create|list|status|cancel|runs".into(),
        )),
    }
}

fn parse_cron_status(raw: Option<&str>) -> CronStatus {
    match raw.unwrap_or("active").trim().to_ascii_lowercase().as_str() {
        "paused" => CronStatus::Paused,
        _ => CronStatus::Active,
    }
}

fn create_cron(store: &dyn Store, params: CronToolParams) -> Result<serde_json::Value, RociError> {
    let name = params
        .name
        .ok_or_else(|| RociError::InvalidArgument("name is required".into()))?;
    let schedule = params
        .schedule
        .ok_or_else(|| RociError::InvalidArgument("schedule is required".into()))?;
    let command = params
        .command
        .ok_or_else(|| RociError::InvalidArgument("command is required".into()))?;

    let now = now_unix();
    let record = CronRecord {
        cron_id: Uuid::new_v4().to_string(),
        name,
        schedule,
        command,
        status: parse_cron_status(params.status.as_deref()),
        skip_overlap: params.skip_overlap.unwrap_or(false),
        created_at: now,
        updated_at: now,
        next_run_at: None,
        last_run_at: None,
    };

    store
        .upsert_cron(&record)
        .map_err(|error| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        })?;

    Ok(serde_json::json!({ "cron": record }))
}

fn list_crons(store: &dyn Store) -> Result<serde_json::Value, RociError> {
    store
        .list_crons()
        .map(|crons| serde_json::json!({ "crons": crons }))
        .map_err(|error| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        })
}

fn cron_status(store: &dyn Store, id: Option<String>) -> Result<serde_json::Value, RociError> {
    let cron_id = id.ok_or_else(|| RociError::InvalidArgument("cron_id is required".into()))?;
    match store.get_cron(&cron_id) {
        Ok(Some(cron)) => Ok(serde_json::json!({ "cron": cron })),
        Ok(None) => Err(RociError::InvalidArgument("cron not found".into())),
        Err(error) => Err(RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        }),
    }
}

fn cancel_cron(store: &dyn Store, id: Option<String>) -> Result<serde_json::Value, RociError> {
    let cron_id = id.ok_or_else(|| RociError::InvalidArgument("cron_id is required".into()))?;
    let cron = store
        .get_cron(&cron_id)
        .map_err(|error| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        })?
        .ok_or_else(|| RociError::InvalidArgument("cron not found".into()))?;

    let updated = CronRecord {
        status: CronStatus::Paused,
        updated_at: now_unix(),
        ..cron
    };

    store
        .upsert_cron(&updated)
        .map_err(|error| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        })?;

    Ok(serde_json::json!({ "cron": updated }))
}

fn cron_runs(
    store: &dyn Store,
    id: Option<String>,
    limit: Option<usize>,
) -> Result<serde_json::Value, RociError> {
    let cron_id = id.ok_or_else(|| RociError::InvalidArgument("cron_id is required".into()))?;
    let runs = store
        .list_cron_runs(&cron_id, limit.unwrap_or(20).clamp(1, 100))
        .map_err(|error| RociError::ToolExecution {
            tool_name: "cron".into(),
            message: error,
        })?;
    Ok(serde_json::json!({ "cron_id": cron_id, "runs": runs }))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

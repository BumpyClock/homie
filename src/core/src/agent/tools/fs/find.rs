use roci::error::RociError;
use roci::tools::ToolArguments;
use tokio::process::Command;
use tokio::time::timeout;

use super::super::ToolContext;
use super::constants::COMMAND_TIMEOUT;
use super::parse::parse_find_request;
use super::resolve_path;

pub(super) async fn find_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_find_request(args)?;
    let pattern = parsed.pattern.as_str();
    let limit = parsed.limit;
    let base = parsed
        .path
        .as_deref()
        .and_then(|p| resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());

    if super::super::debug_tools_enabled() {
        tracing::debug!(
            pattern,
            path = %base.to_string_lossy(),
            limit,
            "find tool invoked"
        );
    }

    let mut cmd = Command::new("rg");
    cmd.arg("--files")
        .arg("--no-messages")
        .arg("--glob")
        .arg(pattern);
    cmd.arg("--").arg(&base);
    let output = timeout(COMMAND_TIMEOUT, cmd.output())
        .await
        .map_err(|_| RociError::ToolExecution {
            tool_name: "find".into(),
            message: "rg timed out".into(),
        })?
        .map_err(|e| RociError::ToolExecution {
            tool_name: "find".into(),
            message: format!("failed to run rg: {e}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RociError::ToolExecution {
            tool_name: "find".into(),
            message: format!("rg failed: {stderr}"),
        });
    }
    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        entries.push(line.to_string());
        if entries.len() >= limit {
            break;
        }
    }

    Ok(serde_json::json!({
        "path": base.to_string_lossy(),
        "matches": entries,
    }))
}

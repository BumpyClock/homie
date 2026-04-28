use roci::error::RociError;
use roci::tools::ToolArguments;
use tokio::process::Command;
use tokio::time::timeout;

use super::super::ToolContext;
use super::constants::COMMAND_TIMEOUT;
use super::parse::parse_grep_request;
use super::resolve_path;

pub(super) async fn grep_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_grep_request(args)?;
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
            "grep tool invoked"
        );
    }

    let mut cmd = Command::new("rg");
    cmd.arg("--line-number")
        .arg("--column")
        .arg("--no-heading")
        .arg("--color")
        .arg("never")
        .arg("--no-messages")
        .arg("--regexp")
        .arg(pattern);
    if let Some(include) = parsed.include.as_deref() {
        cmd.arg("--glob").arg(include);
    }
    cmd.arg("--").arg(&base);
    let output = timeout(COMMAND_TIMEOUT, cmd.output())
        .await
        .map_err(|_| RociError::ToolExecution {
            tool_name: "grep".into(),
            message: "rg timed out".into(),
        })?
        .map_err(|e| RociError::ToolExecution {
            tool_name: "grep".into(),
            message: format!("failed to run rg: {e}"),
        })?;

    let status = output.status.code().unwrap_or(1);
    if status != 0 && status != 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RociError::ToolExecution {
            tool_name: "grep".into(),
            message: format!("rg failed: {stderr}"),
        });
    }

    let mut matches = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        matches.push(line.to_string());
        if matches.len() >= limit {
            break;
        }
    }

    Ok(serde_json::json!({
        "path": base.to_string_lossy(),
        "matches": matches,
    }))
}

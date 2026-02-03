use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use roci::error::RociError;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use roci::tools::tool::ToolExecutionContext;
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::ToolContext;

const DEFAULT_READ_LIMIT: usize = 2000;
const DEFAULT_LS_LIMIT: usize = 200;
const DEFAULT_LS_DEPTH: usize = 2;
const DEFAULT_GREP_LIMIT: usize = 100;
const MAX_GREP_LIMIT: usize = 2000;
const DEFAULT_FIND_LIMIT: usize = 200;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ReadArgs {
    path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LsArgs {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FindArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub fn read_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("path", "File path to read", true)
        .number("offset", "1-indexed line offset", false)
        .number("limit", "Max lines to return", false)
        .build();

    Arc::new(AgentTool::new(
        "read",
        "Read a file from disk",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { read_impl(&ctx, &args).await }
        },
    ))
}

pub fn ls_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("path", "Directory path", false)
        .number("depth", "Depth to traverse", false)
        .number("limit", "Max entries", false)
        .build();

    Arc::new(AgentTool::new(
        "ls",
        "List directory contents",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { ls_impl(&ctx, &args).await }
        },
    ))
}

pub fn find_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("pattern", "Glob pattern (rg-style)", true)
        .string("path", "Search root", false)
        .number("limit", "Max results", false)
        .build();

    Arc::new(AgentTool::new(
        "find",
        "Find files by glob",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { find_impl(&ctx, &args).await }
        },
    ))
}

pub fn grep_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("pattern", "Regex pattern", true)
        .string("path", "Search root", false)
        .string("include", "Glob filter", false)
        .number("limit", "Max matches", false)
        .build();

    Arc::new(AgentTool::new(
        "grep",
        "Search file contents",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { grep_impl(&ctx, &args).await }
        },
    ))
}

pub fn resolve_path(path: &str, cwd: &Path) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = crate::paths::user_home_dir() {
            return Some(home.join(rest));
        }
    }
    if trimmed == "~" {
        return crate::paths::user_home_dir();
    }
    let path_buf = PathBuf::from(trimmed);
    if path_buf.is_relative() {
        return Some(cwd.join(path_buf));
    }
    Some(path_buf)
}

async fn read_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<serde_json::Value, RociError> {
    let parsed: ReadArgs = args.deserialize()?;
    let path = resolve_path(&parsed.path, &ctx.cwd)
        .ok_or_else(|| RociError::InvalidArgument("path required".into()))?;
    let offset = parsed.offset.unwrap_or(1).max(1);
    let limit = parsed.limit.unwrap_or(DEFAULT_READ_LIMIT).max(1);

    if super::debug_tools_enabled() {
        tracing::debug!(
            path = %path.to_string_lossy(),
            offset,
            limit,
            "read tool invoked"
        );
    }

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| RociError::ToolExecution {
            tool_name: "read".into(),
            message: format!("failed to read file: {e}"),
        })?;

    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut output = Vec::new();
    let mut line_no = 0usize;

    while let Some(line) = lines.next_line().await.map_err(|e| RociError::ToolExecution {
        tool_name: "read".into(),
        message: format!("failed to read file: {e}"),
    })? {
        line_no += 1;
        if line_no < offset {
            continue;
        }
        if output.len() >= limit {
            break;
        }
        output.push(format!("L{}: {}", line_no, line));
    }

    if line_no < offset {
        return Err(RociError::ToolExecution {
            tool_name: "read".into(),
            message: "offset exceeds file length".into(),
        });
    }

    Ok(serde_json::json!({
        "path": path.to_string_lossy(),
        "offset": offset,
        "limit": limit,
        "content": output.join("\n"),
    }))
}

async fn ls_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<serde_json::Value, RociError> {
    let parsed: LsArgs = args.deserialize()?;
    let base = parsed
        .path
        .as_deref()
        .and_then(|p| resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());
    let depth = parsed.depth.unwrap_or(DEFAULT_LS_DEPTH).max(1);
    let limit = parsed.limit.unwrap_or(DEFAULT_LS_LIMIT).max(1);

    if super::debug_tools_enabled() {
        tracing::debug!(
            path = %base.to_string_lossy(),
            depth,
            limit,
            "ls tool invoked"
        );
    }

    let entries = list_dir(&base, depth, limit).await?;
    Ok(serde_json::json!({
        "path": base.to_string_lossy(),
        "entries": entries,
    }))
}

async fn list_dir(
    base: &Path,
    depth: usize,
    limit: usize,
) -> Result<Vec<serde_json::Value>, RociError> {
    let mut results = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back((base.to_path_buf(), 0usize));

    while let Some((dir, level)) = queue.pop_front() {
        if results.len() >= limit {
            break;
        }
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(e) => {
                if super::debug_tools_enabled() {
                    tracing::debug!(
                        path = %dir.to_string_lossy(),
                        error = %e,
                        "ls skip unreadable directory"
                    );
                }
                continue;
            }
        };
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| RociError::ToolExecution {
            tool_name: "ls".into(),
            message: format!("failed to read directory: {e}"),
        })? {
            if results.len() >= limit {
                break;
            }
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => {
                    if super::debug_tools_enabled() {
                        tracing::debug!(error = %e, "ls skip unreadable entry");
                    }
                    continue;
                }
            };
            let path = entry.path();
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().to_string();
            let kind = if file_type.is_dir() { "dir" } else { "file" };
            results.push(serde_json::json!({
                "path": path.to_string_lossy(),
                "relative_path": rel_str,
                "type": kind,
            }));
            if file_type.is_dir() && level + 1 < depth {
                queue.push_back((path, level + 1));
            }
        }
    }

    Ok(results)
}

async fn find_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<serde_json::Value, RociError> {
    let parsed: FindArgs = args.deserialize()?;
    let pattern = parsed.pattern.trim();
    if pattern.is_empty() {
        return Err(RociError::InvalidArgument("pattern must not be empty".into()));
    }
    let limit = parsed.limit.unwrap_or(DEFAULT_FIND_LIMIT).max(1);
    let base = parsed
        .path
        .as_deref()
        .and_then(|p| resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());

    if super::debug_tools_enabled() {
        tracing::debug!(
            pattern,
            path = %base.to_string_lossy(),
            limit,
            "find tool invoked"
        );
    }

    let mut cmd = Command::new("rg");
    cmd.arg("--files").arg("--no-messages").arg("--glob").arg(pattern);
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

async fn grep_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<serde_json::Value, RociError> {
    let parsed: GrepArgs = args.deserialize()?;
    let pattern = parsed.pattern.trim();
    if pattern.is_empty() {
        return Err(RociError::InvalidArgument("pattern must not be empty".into()));
    }
    let limit = parsed.limit.unwrap_or(DEFAULT_GREP_LIMIT).clamp(1, MAX_GREP_LIMIT);
    let base = parsed
        .path
        .as_deref()
        .and_then(|p| resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());

    if super::debug_tools_enabled() {
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
    if let Some(include) = parsed.include.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
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

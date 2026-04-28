use std::collections::VecDeque;
use std::path::Path;

use roci::error::RociError;
use roci::tools::ToolArguments;

use super::super::ToolContext;
use super::parse::parse_ls_request;
use super::resolve_path;

pub(super) async fn ls_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_ls_request(args)?;
    let base = parsed
        .path
        .as_deref()
        .and_then(|p| resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());
    let depth = parsed.depth;
    let limit = parsed.limit;

    if super::super::debug_tools_enabled() {
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
                if super::super::debug_tools_enabled() {
                    tracing::debug!(
                        path = %dir.to_string_lossy(),
                        error = %e,
                        "ls skip unreadable directory"
                    );
                }
                continue;
            }
        };
        while let Some(entry) =
            read_dir
                .next_entry()
                .await
                .map_err(|e| RociError::ToolExecution {
                    tool_name: "ls".into(),
                    message: format!("failed to read directory: {e}"),
                })?
        {
            if results.len() >= limit {
                break;
            }
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => {
                    if super::super::debug_tools_enabled() {
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

use roci::error::RociError;
use roci::tools::ToolArguments;
use tokio::io::AsyncBufReadExt;

use super::super::ToolContext;
use super::parse::parse_read_request;
use super::resolve_path;

pub(super) async fn read_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_read_request(args)?;
    let path = resolve_path(&parsed.path, &ctx.cwd)
        .ok_or_else(|| RociError::InvalidArgument("path must not be empty".into()))?;
    let offset = parsed.offset;
    let limit = parsed.limit;

    if super::super::debug_tools_enabled() {
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

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| RociError::ToolExecution {
            tool_name: "read".into(),
            message: format!("failed to read file: {e}"),
        })?
    {
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

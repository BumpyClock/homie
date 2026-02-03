use std::path::{Path, PathBuf};
use std::sync::Arc;

use roci::error::RociError;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use roci::tools::tool::ToolExecutionContext;
use serde::Deserialize;

use super::ToolContext;

const BEGIN_PATCH: &str = "*** Begin Patch";
const END_PATCH: &str = "*** End Patch";
const ADD_FILE: &str = "*** Add File: ";
const DELETE_FILE: &str = "*** Delete File: ";
const UPDATE_FILE: &str = "*** Update File: ";
const MOVE_TO: &str = "*** Move to: ";
const END_OF_FILE: &str = "*** End of File";

#[derive(Debug, Deserialize)]
struct ApplyPatchArgs {
    patch: String,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug)]
enum PatchHunk {
    Add { path: String, contents: Vec<String> },
    Delete { path: String },
    Update { path: String, move_path: Option<String>, chunks: Vec<UpdateChunk> },
}

#[derive(Debug)]
struct UpdateChunk {
    context: Option<String>,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    eof: bool,
}

pub fn apply_patch_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("patch", "Patch text", true)
        .string("cwd", "Working directory for relative paths", false)
        .build();

    Arc::new(AgentTool::new(
        "apply_patch",
        "Apply a patch to files",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { apply_patch_impl(&ctx, &args).await }
        },
    ))
}

async fn apply_patch_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed: ApplyPatchArgs = args.deserialize()?;
    if parsed.patch.trim().is_empty() {
        return Err(RociError::InvalidArgument("patch must not be empty".into()));
    }
    let cwd = parsed
        .cwd
        .as_deref()
        .and_then(|p| super::fs::resolve_path(p, &ctx.cwd))
        .unwrap_or_else(|| ctx.cwd.clone());
    if super::debug_tools_enabled() {
        tracing::debug!(
            cwd = %cwd.to_string_lossy(),
            patch_len = parsed.patch.len(),
            "apply_patch tool invoked"
        );
    }
    let hunks = parse_patch(&parsed.patch).map_err(|e| RociError::ToolExecution {
        tool_name: "apply_patch".into(),
        message: e,
    })?;
    let changes = apply_hunks(&cwd, hunks).map_err(|e| RociError::ToolExecution {
        tool_name: "apply_patch".into(),
        message: e,
    })?;

    Ok(serde_json::json!({
        "status": "ok",
        "changes": changes,
        "diff": parsed.patch,
    }))
}

fn parse_patch(patch: &str) -> Result<Vec<PatchHunk>, String> {
    let lines: Vec<&str> = patch.lines().collect();
    if lines.is_empty() || lines[0].trim() != BEGIN_PATCH {
        return Err("patch missing '*** Begin Patch'".into());
    }
    let mut hunks = Vec::new();
    let mut i = 1usize;
    while i < lines.len() {
        let line = lines[i].trim_end();
        if line.trim() == END_PATCH {
            break;
        }
        if let Some(path) = line.trim_start().strip_prefix(ADD_FILE) {
            let (contents, next) = parse_add(lines.as_slice(), i + 1)?;
            hunks.push(PatchHunk::Add {
                path: path.trim().to_string(),
                contents,
            });
            i = next;
            continue;
        }
        if let Some(path) = line.trim_start().strip_prefix(DELETE_FILE) {
            hunks.push(PatchHunk::Delete {
                path: path.trim().to_string(),
            });
            i += 1;
            continue;
        }
        if let Some(path) = line.trim_start().strip_prefix(UPDATE_FILE) {
            let (move_path, chunks, next) = parse_update(lines.as_slice(), i + 1)?;
            hunks.push(PatchHunk::Update {
                path: path.trim().to_string(),
                move_path,
                chunks,
            });
            i = next;
            continue;
        }
        return Err(format!("unexpected patch line: {line}"));
    }
    Ok(hunks)
}

fn parse_add(lines: &[&str], mut idx: usize) -> Result<(Vec<String>, usize), String> {
    let mut contents = Vec::new();
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim_start();
        if trimmed.starts_with("*** ") || trimmed == END_PATCH {
            break;
        }
        if let Some(rest) = line.strip_prefix('+') {
            contents.push(rest.to_string());
        } else {
            contents.push(line.to_string());
        }
        idx += 1;
    }
    Ok((contents, idx))
}

fn parse_update(
    lines: &[&str],
    mut idx: usize,
) -> Result<(Option<String>, Vec<UpdateChunk>, usize), String> {
    let mut move_path = None;
    if idx < lines.len() {
        if let Some(rest) = lines[idx].trim_start().strip_prefix(MOVE_TO) {
            move_path = Some(rest.trim().to_string());
            idx += 1;
        }
    }

    let mut chunks = Vec::new();
    let mut current: Option<UpdateChunk> = None;

    while idx < lines.len() {
        let raw = lines[idx];
        let trimmed = raw.trim_start();
        if trimmed.starts_with("*** ") || trimmed == END_PATCH {
            break;
        }
        if trimmed == END_OF_FILE {
            if let Some(chunk) = current.as_mut() {
                chunk.eof = true;
            }
            idx += 1;
            continue;
        }
        if trimmed.starts_with("@@") {
            if let Some(chunk) = current.take() {
                chunks.push(chunk);
            }
            let context = trimmed.strip_prefix("@@").unwrap_or("").trim();
            current = Some(UpdateChunk {
                context: if context.is_empty() { None } else { Some(context.to_string()) },
                old_lines: Vec::new(),
                new_lines: Vec::new(),
                eof: false,
            });
            idx += 1;
            continue;
        }

        let Some(chunk) = current.as_mut() else {
            return Err("update chunk missing @@ context".into());
        };
        if let Some(rest) = raw.strip_prefix(' ') {
            chunk.old_lines.push(rest.to_string());
            chunk.new_lines.push(rest.to_string());
        } else if let Some(rest) = raw.strip_prefix('-') {
            chunk.old_lines.push(rest.to_string());
        } else if let Some(rest) = raw.strip_prefix('+') {
            chunk.new_lines.push(rest.to_string());
        } else {
            return Err(format!("invalid patch line: {raw}"));
        }
        idx += 1;
    }

    if let Some(chunk) = current.take() {
        chunks.push(chunk);
    }

    if chunks.is_empty() {
        return Err("update file missing chunks".into());
    }

    Ok((move_path, chunks, idx))
}

fn apply_hunks(cwd: &Path, hunks: Vec<PatchHunk>) -> Result<Vec<serde_json::Value>, String> {
    let mut changes = Vec::new();
    for hunk in hunks {
        match hunk {
            PatchHunk::Add { path, contents } => {
                let file_path = resolve_hunk_path(cwd, &path)?;
                if file_path.exists() {
                    return Err(format!("file already exists: {}", file_path.display()));
                }
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("failed to create dirs: {e}"))?;
                }
                let text = contents.join("\n");
                std::fs::write(&file_path, text)
                    .map_err(|e| format!("failed to write file: {e}"))?;
                changes.push(json_change("add", &file_path));
            }
            PatchHunk::Delete { path } => {
                let file_path = resolve_hunk_path(cwd, &path)?;
                if !file_path.exists() {
                    return Err(format!("file not found: {}", file_path.display()));
                }
                move_to_trash(&file_path)?;
                changes.push(json_change("delete", &file_path));
            }
            PatchHunk::Update {
                path,
                move_path,
                chunks,
            } => {
                let file_path = resolve_hunk_path(cwd, &path)?;
                let mut content = std::fs::read_to_string(&file_path)
                    .map_err(|e| format!("failed to read file: {e}"))?;
                let had_trailing_newline = content.ends_with('\n');
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

                let mut cursor = 0usize;
                for chunk in chunks {
                    let anchor = if let Some(context) = chunk.context.as_ref() {
                        find_anchor(&lines, context, cursor)
                            .ok_or_else(|| format!("context not found: {context}"))?
                    } else {
                        cursor
                    };
                    let start = if chunk.old_lines.is_empty() {
                        anchor
                    } else if chunk.eof {
                        if lines.len() < chunk.old_lines.len() {
                            return Err("patch expects EOF but file is shorter".into());
                        }
                        let tail_start = lines.len() - chunk.old_lines.len();
                        if lines[tail_start..] != chunk.old_lines[..] {
                            return Err("patch EOF context not found".into());
                        }
                        tail_start
                    } else {
                        find_subsequence(&lines, &chunk.old_lines, anchor)
                            .ok_or_else(|| "patch target not found".to_string())?
                    };

                    let end = start + chunk.old_lines.len();
                    lines.splice(start..end, chunk.new_lines.clone());
                    cursor = start + chunk.new_lines.len();
                }

                content = lines.join("\n");
                if had_trailing_newline {
                    content.push('\n');
                }
                std::fs::write(&file_path, content)
                    .map_err(|e| format!("failed to write file: {e}"))?;

                if let Some(target) = move_path {
                    let target_path = resolve_hunk_path(cwd, &target)?;
                    if let Some(parent) = target_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("failed to create dirs: {e}"))?;
                    }
                    std::fs::rename(&file_path, &target_path)
                        .map_err(|e| format!("failed to move file: {e}"))?;
                    changes.push(json_change("move", &target_path));
                } else {
                    changes.push(json_change("update", &file_path));
                }
            }
        }
    }
    Ok(changes)
}

fn resolve_hunk_path(cwd: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty patch path".into());
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(cwd.join(path))
}

fn find_anchor(lines: &[String], context: &str, start: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, line)| line.contains(context))
        .map(|(idx, _)| idx + 1)
}

fn find_subsequence(lines: &[String], target: &[String], start: usize) -> Option<usize> {
    if target.is_empty() {
        return Some(start);
    }
    for idx in start..=lines.len().saturating_sub(target.len()) {
        if lines[idx..idx + target.len()] == target[..] {
            return Some(idx);
        }
    }
    None
}

fn move_to_trash(path: &Path) -> Result<(), String> {
    let trash_dir = crate::paths::homie_home_dir()
        .map_err(|e| format!("failed to resolve homie home: {e}"))?
        .join("trash");
    std::fs::create_dir_all(&trash_dir)
        .map_err(|e| format!("failed to create trash dir: {e}"))?;
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "deleted".into());
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let target = trash_dir.join(format!("{stamp}-{file_name}"));
    std::fs::rename(path, &target)
        .or_else(|_| std::fs::remove_file(path))
        .map_err(|e| format!("failed to delete file: {e}"))?;
    Ok(())
}

fn json_change(kind: &str, path: &Path) -> serde_json::Value {
    serde_json::json!({
        "action": kind,
        "path": path.to_string_lossy(),
    })
}

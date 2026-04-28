// Parser-heavy tool module returns rich `RociError` variants; keep existing signatures stable.
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};

use super::ToolContext;

mod constants;
mod find;
mod grep;
mod ls;
mod parse;
mod path;
mod read;
mod requests;
#[cfg(test)]
mod tests;

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
            async move { read::read_impl(&ctx, &args).await }
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
            async move { ls::ls_impl(&ctx, &args).await }
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
            async move { find::find_impl(&ctx, &args).await }
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
            async move { grep::grep_impl(&ctx, &args).await }
        },
    ))
}

pub fn resolve_path(path: &str, cwd: &std::path::Path) -> Option<std::path::PathBuf> {
    path::resolve_path(path, cwd)
}

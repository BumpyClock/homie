use roci::error::RociError;
use roci::tools::ToolArguments;

use super::super::args::ParsedToolArgs;
use super::constants::{
    DEFAULT_FIND_LIMIT, DEFAULT_GREP_LIMIT, DEFAULT_LS_DEPTH, DEFAULT_LS_LIMIT, DEFAULT_READ_LIMIT,
    MAX_GREP_LIMIT,
};
use super::requests::{FindRequest, GrepRequest, LsRequest, ReadRequest};

pub(super) fn parse_read_request(args: &ToolArguments) -> Result<ReadRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let path = clean_string(parsed.get_string_any(&["path", "file", "file_path", "filepath"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .ok_or_else(|| RociError::InvalidArgument("path must not be empty".into()))?;
    let offset = parsed
        .get_usize_any(&["offset", "start", "line"])?
        .unwrap_or(1)
        .max(1);
    let limit = parsed
        .get_usize_any(&["limit", "max_lines", "maxLines"])?
        .unwrap_or(DEFAULT_READ_LIMIT)
        .max(1);
    Ok(ReadRequest {
        path,
        offset,
        limit,
    })
}

pub(super) fn parse_ls_request(args: &ToolArguments) -> Result<LsRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let path = clean_string(parsed.get_string_any(&["path", "dir", "directory"])?)
        .or_else(|| clean_literal(parsed.literal()));
    let depth = parsed
        .get_usize_any(&["depth", "max_depth", "maxDepth"])?
        .unwrap_or(DEFAULT_LS_DEPTH)
        .max(1);
    let limit = parsed
        .get_usize_any(&["limit", "max_entries", "maxEntries"])?
        .unwrap_or(DEFAULT_LS_LIMIT)
        .max(1);
    Ok(LsRequest { path, depth, limit })
}

pub(super) fn parse_find_request(args: &ToolArguments) -> Result<FindRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let pattern = clean_string(parsed.get_string_any(&["pattern", "glob", "query"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .ok_or_else(|| RociError::InvalidArgument("pattern must not be empty".into()))?;
    let path = clean_string(parsed.get_string_any(&["path", "dir", "directory"])?);
    let limit = parsed
        .get_usize_any(&["limit", "max_results", "maxResults"])?
        .unwrap_or(DEFAULT_FIND_LIMIT)
        .max(1);
    Ok(FindRequest {
        pattern,
        path,
        limit,
    })
}

pub(super) fn parse_grep_request(args: &ToolArguments) -> Result<GrepRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let pattern = clean_string(parsed.get_string_any(&["pattern", "regex", "query"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .ok_or_else(|| RociError::InvalidArgument("pattern must not be empty".into()))?;
    let path = clean_string(parsed.get_string_any(&["path", "dir", "directory"])?);
    let include = clean_string(parsed.get_string_any(&["include", "glob"])?);
    let limit = parsed
        .get_usize_any(&["limit", "max_results", "maxResults"])?
        .unwrap_or(DEFAULT_GREP_LIMIT)
        .clamp(1, MAX_GREP_LIMIT);
    Ok(GrepRequest {
        pattern,
        path,
        include,
        limit,
    })
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

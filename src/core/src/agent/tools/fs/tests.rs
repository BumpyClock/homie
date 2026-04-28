use roci::tools::ToolArguments;
use serde_json::json;

use super::constants::{
    DEFAULT_FIND_LIMIT, DEFAULT_LS_DEPTH, DEFAULT_LS_LIMIT, DEFAULT_READ_LIMIT,
};
use super::parse::{parse_find_request, parse_grep_request, parse_ls_request, parse_read_request};

#[test]
fn read_request_accepts_string_payload_and_numeric_strings() {
    let args = ToolArguments::new(json!({
        "path": "Cargo.toml",
        "offset": "2",
        "limit": "10"
    }));
    let parsed = parse_read_request(&args).expect("parse read request");
    assert_eq!(parsed.path, "Cargo.toml");
    assert_eq!(parsed.offset, 2);
    assert_eq!(parsed.limit, 10);

    let args = ToolArguments::new(json!("README.md"));
    let parsed = parse_read_request(&args).expect("parse read request");
    assert_eq!(parsed.path, "README.md");
    assert_eq!(parsed.offset, 1);
    assert_eq!(parsed.limit, DEFAULT_READ_LIMIT);
}

#[test]
fn read_request_accepts_path_aliases() {
    let args = ToolArguments::new(json!({
        "file_path": "Cargo.toml",
        "maxLines": "12"
    }));
    let parsed = parse_read_request(&args).expect("parse read request");
    assert_eq!(parsed.path, "Cargo.toml");
    assert_eq!(parsed.limit, 12);
}

#[test]
fn read_request_rejects_missing_path_with_clear_error() {
    let args = ToolArguments::new(json!({ "offset": 1 }));
    let err = parse_read_request(&args).expect_err("missing path should fail");
    assert_eq!(err.to_string(), "Invalid argument: path must not be empty");
}

#[test]
fn ls_request_defaults_to_cwd_and_supports_literal_path() {
    let args = ToolArguments::new(json!({}));
    let parsed = parse_ls_request(&args).expect("parse ls request");
    assert_eq!(parsed.path, None);
    assert_eq!(parsed.depth, DEFAULT_LS_DEPTH);
    assert_eq!(parsed.limit, DEFAULT_LS_LIMIT);

    let args = ToolArguments::new(json!("src"));
    let parsed = parse_ls_request(&args).expect("parse ls request");
    assert_eq!(parsed.path.as_deref(), Some("src"));
}

#[test]
fn find_request_supports_literal_pattern_and_default_limit() {
    let args = ToolArguments::new(json!("*.rs"));
    let parsed = parse_find_request(&args).expect("parse find request");
    assert_eq!(parsed.pattern, "*.rs");
    assert_eq!(parsed.limit, DEFAULT_FIND_LIMIT);
}

#[test]
fn find_request_accepts_query_aliases() {
    let args = ToolArguments::new(json!({
        "query": "*.md",
        "directory": "docs",
        "maxResults": "5"
    }));
    let parsed = parse_find_request(&args).expect("parse find request");
    assert_eq!(parsed.pattern, "*.md");
    assert_eq!(parsed.path.as_deref(), Some("docs"));
    assert_eq!(parsed.limit, 5);
}

#[test]
fn grep_request_clamps_and_parses_limit_strings() {
    let args = ToolArguments::new(json!({
        "pattern": "foo",
        "limit": "5000",
        "include": "*.rs"
    }));
    let parsed = parse_grep_request(&args).expect("parse grep request");
    assert_eq!(parsed.limit, 2000);
    assert_eq!(parsed.include.as_deref(), Some("*.rs"));
}

#[test]
fn grep_request_accepts_regex_and_glob_aliases() {
    let args = ToolArguments::new(json!({
        "regex": "main",
        "glob": "*.rs",
        "maxResults": "20"
    }));
    let parsed = parse_grep_request(&args).expect("parse grep request");
    assert_eq!(parsed.pattern, "main");
    assert_eq!(parsed.include.as_deref(), Some("*.rs"));
    assert_eq!(parsed.limit, 20);
}

#[test]
fn grep_request_rejects_empty_pattern() {
    let args = ToolArguments::new(json!({ "pattern": "   " }));
    let err = parse_grep_request(&args).expect_err("empty pattern should fail");
    assert_eq!(
        err.to_string(),
        "Invalid argument: pattern must not be empty"
    );
}

// Parser-heavy tool module returns rich `RociError` variants; keep existing signatures stable.
#![allow(clippy::result_large_err)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;

use super::args::ParsedToolArgs;
use super::ToolContext;

const BROWSER_TOOL_NAME: &str = "browser";
const DEFAULT_TIMEOUT_SECS: u64 = 90;
const MAX_ERROR_CHARS: usize = 4_000;

#[derive(Debug, PartialEq, Eq)]
struct BrowserRequest {
    command: String,
    cwd: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    provider: Option<String>,
    headed: bool,
    json: bool,
    timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserRunner {
    Direct,
    Npx,
}

#[derive(Debug)]
struct BrowserCommandResult {
    runner: BrowserRunner,
    status: i32,
    stdout: String,
    stderr: String,
}

pub fn browser_tool(ctx: ToolContext) -> std::sync::Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string(
            "command",
            "agent-browser command (example: 'open https://example.com' or 'snapshot -i').",
            true,
        )
        .string("cwd", "Working directory for relative file paths.", false)
        .string("session", "Optional browser session name.", false)
        .string("profile", "Optional browser profile path.", false)
        .string(
            "provider",
            "Optional provider (browserbase|browseruse|kernel|ios).",
            false,
        )
        .boolean("headed", "Run with a visible browser window.", false)
        .boolean(
            "json",
            "Use JSON output from agent-browser (default true).",
            false,
        )
        .number("timeout", "Timeout in seconds (default 90).", false)
        .build();

    std::sync::Arc::new(AgentTool::new(
        BROWSER_TOOL_NAME,
        "Control a headless browser using the agent-browser CLI.",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { browser_impl(&ctx, &args).await }
        },
    ))
}

async fn browser_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<Value, RociError> {
    match browser_inner(ctx, args).await {
        Ok(payload) => Ok(success_envelope(payload)),
        Err(err) => Ok(error_envelope_from_roci(err)),
    }
}

async fn browser_inner(ctx: &ToolContext, args: &ToolArguments) -> Result<Value, RociError> {
    let request = parse_browser_request(args)?;
    let mut command_tokens = parse_command_tokens(&request.command)?;
    let action = command_tokens
        .first()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty());
    if request.json && !command_tokens.iter().any(|token| token == "--json") {
        command_tokens.insert(0, "--json".to_string());
    }
    if let Some(session) = request.session.as_deref() {
        command_tokens.insert(0, session.to_string());
        command_tokens.insert(0, "--session".to_string());
    }
    if let Some(profile) = request.profile.as_deref() {
        command_tokens.insert(0, profile.to_string());
        command_tokens.insert(0, "--profile".to_string());
    }
    if let Some(provider) = request.provider.as_deref() {
        command_tokens.insert(0, provider.to_string());
        command_tokens.insert(0, "--provider".to_string());
    }
    if request.headed {
        command_tokens.insert(0, "--headed".to_string());
    }
    let cwd = resolve_cwd(ctx, request.cwd.as_deref());

    if super::debug_tools_enabled() {
        tracing::debug!(
            command = %request.command,
            cwd = %cwd.display(),
            timeout_secs = request.timeout_secs,
            "browser tool invoked"
        );
    }

    let result = run_agent_browser(&command_tokens, &cwd, request.timeout_secs).await?;
    if result.status != 0 {
        let detail = format_error_detail(&result.stdout, &result.stderr);
        return Err(RociError::ToolExecution {
            tool_name: BROWSER_TOOL_NAME.to_string(),
            message: format!(
                "agent-browser command failed (runner={}, exit_code={}): {detail}",
                runner_label(result.runner),
                result.status
            ),
        });
    }

    let parsed = parse_json_output(result.stdout.as_str());
    let mut payload = match parsed {
        Some(Value::Object(obj)) => {
            if let Some(false) = obj.get("success").and_then(Value::as_bool) {
                let msg = obj
                    .get("error")
                    .and_then(Value::as_str)
                    .or_else(|| obj.get("message").and_then(Value::as_str))
                    .unwrap_or("agent-browser command failed");
                return Err(RociError::ToolExecution {
                    tool_name: BROWSER_TOOL_NAME.to_string(),
                    message: msg.to_string(),
                });
            }
            obj.get("data").cloned().unwrap_or(Value::Object(obj))
        }
        Some(value) => value,
        None => Value::String(result.stdout.trim().to_string()),
    };

    if let Value::Object(map) = &mut payload {
        map.entry("action".to_string())
            .or_insert_with(|| Value::String(action.unwrap_or_else(|| "command".to_string())));
        map.insert(
            "runner".to_string(),
            Value::String(runner_label(result.runner).to_string()),
        );
        map.insert(
            "command".to_string(),
            Value::String(request.command.clone()),
        );
    }

    Ok(payload)
}

fn parse_browser_request(args: &ToolArguments) -> Result<BrowserRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let command = clean_string(parsed.get_string_any(&["command", "cmd", "action"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .ok_or_else(|| RociError::InvalidArgument("command must not be empty".into()))?;
    let cwd = clean_string(parsed.get_string_any(&["cwd", "workdir", "working_dir"])?);
    let session = clean_string(parsed.get_string_any(&["session", "session_name"])?);
    let profile = clean_string(parsed.get_string("profile")?);
    let provider = clean_string(parsed.get_string("provider")?);
    let headed = parsed.get_bool("headed")?.unwrap_or(false);
    let json = parsed.get_bool("json")?.unwrap_or(true);
    let timeout_secs = parsed
        .get_u64_any(&["timeout", "timeout_secs", "timeoutSeconds"])?
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .max(1);

    Ok(BrowserRequest {
        command,
        cwd,
        session,
        profile,
        provider,
        headed,
        json,
        timeout_secs,
    })
}

fn parse_command_tokens(command: &str) -> Result<Vec<String>, RociError> {
    let mut tokens = shell_words::split(command)
        .map_err(|e| RociError::InvalidArgument(format!("invalid browser command: {e}")))?;
    if tokens.is_empty() {
        return Err(RociError::InvalidArgument(
            "command must not be empty".into(),
        ));
    }
    if tokens
        .first()
        .map(|token| token == "agent-browser")
        .unwrap_or(false)
    {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return Err(RociError::InvalidArgument(
            "command must include an action".into(),
        ));
    }
    Ok(tokens)
}

fn resolve_cwd(ctx: &ToolContext, override_path: Option<&str>) -> PathBuf {
    if let Some(path) = override_path {
        if let Some(resolved) = super::fs::resolve_path(path, &ctx.cwd) {
            return resolved;
        }
    }
    ctx.cwd.clone()
}

async fn run_agent_browser(
    args: &[String],
    cwd: &Path,
    timeout_secs: u64,
) -> Result<BrowserCommandResult, RociError> {
    match run_command(BrowserRunner::Direct, args, cwd, timeout_secs).await {
        Ok(result) => Ok(result),
        Err(err) if is_not_found_error(&err) => {
            run_command(BrowserRunner::Npx, args, cwd, timeout_secs)
                .await
                .map_err(|fallback| map_spawn_error(BrowserRunner::Npx, fallback))
        }
        Err(err) => Err(map_spawn_error(BrowserRunner::Direct, err)),
    }
}

async fn run_command(
    runner: BrowserRunner,
    args: &[String],
    cwd: &Path,
    timeout_secs: u64,
) -> Result<BrowserCommandResult, std::io::Error> {
    let mut command = build_command(runner, args, cwd);
    let child = command.spawn()?;
    let output = timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "command timed out"))??;

    Ok(BrowserCommandResult {
        runner,
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn build_command(runner: BrowserRunner, args: &[String], cwd: &Path) -> Command {
    let mut command = match runner {
        BrowserRunner::Direct => {
            let binary = std::env::var("HOMIE_AGENT_BROWSER_BIN")
                .unwrap_or_else(|_| "agent-browser".to_string());
            Command::new(binary)
        }
        BrowserRunner::Npx => {
            let mut cmd = Command::new("npx");
            cmd.args(["--yes", "agent-browser"]);
            cmd
        }
    };
    command.current_dir(cwd).args(args);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command
}

fn is_not_found_error(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::NotFound
}

fn map_spawn_error(runner: BrowserRunner, err: std::io::Error) -> RociError {
    if err.kind() == std::io::ErrorKind::TimedOut {
        return RociError::ToolExecution {
            tool_name: BROWSER_TOOL_NAME.to_string(),
            message: "browser command timed out".to_string(),
        };
    }
    if err.kind() == std::io::ErrorKind::NotFound {
        return RociError::ToolExecution {
            tool_name: BROWSER_TOOL_NAME.to_string(),
            message: "agent-browser not found. Install with `npm i -g agent-browser && agent-browser install` or `brew install agent-browser && agent-browser install`.".to_string(),
        };
    }
    RociError::ToolExecution {
        tool_name: BROWSER_TOOL_NAME.to_string(),
        message: format!(
            "failed to launch browser command via {}: {err}",
            runner_label(runner)
        ),
    }
}

fn parse_json_output(stdout: &str) -> Option<Value> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(trimmed).ok()
}

fn format_error_detail(stdout: &str, stderr: &str) -> String {
    let std_err = stderr.trim();
    if !std_err.is_empty() {
        return truncate_str(std_err, MAX_ERROR_CHARS);
    }
    let std_out = stdout.trim();
    if !std_out.is_empty() {
        return truncate_str(std_out, MAX_ERROR_CHARS);
    }
    "no output".to_string()
}

fn truncate_str(value: &str, max_chars: usize) -> String {
    let mut iter = value.chars();
    let head: String = iter.by_ref().take(max_chars).collect();
    if iter.next().is_none() {
        return head;
    }
    format!("{head}...")
}

fn runner_label(runner: BrowserRunner) -> &'static str {
    match runner {
        BrowserRunner::Direct => "agent-browser",
        BrowserRunner::Npx => "npx agent-browser",
    }
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

fn success_envelope(data: Value) -> Value {
    serde_json::json!({
        "ok": true,
        "tool": BROWSER_TOOL_NAME,
        "data": data
    })
}

fn error_envelope_from_roci(err: RociError) -> Value {
    let retryable = err.is_retryable();
    match err {
        RociError::InvalidArgument(message) => {
            error_envelope("invalid_argument", message, retryable)
        }
        RociError::Timeout(timeout_ms) => error_envelope(
            "timeout",
            format!("request timed out after {timeout_ms}ms"),
            retryable,
        ),
        RociError::Network(message) => {
            error_envelope("network_error", message.to_string(), retryable)
        }
        RociError::Serialization(message) => {
            error_envelope("serialization_error", message.to_string(), retryable)
        }
        RociError::ToolExecution { message, .. } => {
            error_envelope("tool_execution_failed", message, retryable)
        }
        other => error_envelope("tool_error", other.to_string(), retryable),
    }
}

fn error_envelope(code: &str, message: String, retryable: bool) -> Value {
    serde_json::json!({
        "ok": false,
        "tool": BROWSER_TOOL_NAME,
        "error": {
            "code": code,
            "message": message,
            "retryable": retryable
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use roci::tools::ToolArguments;
    use serde_json::json;

    use crate::homie_config::HomieConfig;

    use super::{browser_impl, parse_browser_request, parse_command_tokens, ToolContext};

    #[test]
    fn parse_browser_request_defaults_json_and_timeout() {
        let args = ToolArguments::new(json!({
            "command": "snapshot -i"
        }));
        let parsed = parse_browser_request(&args).expect("parse browser request");
        assert_eq!(parsed.command, "snapshot -i");
        assert!(parsed.json);
        assert_eq!(parsed.timeout_secs, 90);
    }

    #[test]
    fn parse_browser_request_accepts_literal_payload() {
        let args = ToolArguments::new(json!("open https://example.com"));
        let parsed = parse_browser_request(&args).expect("parse browser request");
        assert_eq!(parsed.command, "open https://example.com");
    }

    #[test]
    fn parse_command_tokens_strips_agent_browser_prefix() {
        let tokens =
            parse_command_tokens("agent-browser open https://example.com").expect("parse command");
        assert_eq!(
            tokens,
            vec!["open".to_string(), "https://example.com".to_string()]
        );
    }

    #[tokio::test]
    async fn browser_impl_returns_error_envelope_for_missing_command() {
        let config = Arc::new(HomieConfig::default());
        let ctx = ToolContext::new(config);
        let payload = browser_impl(&ctx, &ToolArguments::new(json!({})))
            .await
            .expect("browser response");
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["tool"], json!("browser"));
        assert_eq!(payload["error"]["code"], json!("invalid_argument"));
    }
}

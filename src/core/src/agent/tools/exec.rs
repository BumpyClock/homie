// Parser-heavy tool module returns rich `RociError` variants; keep existing signatures stable.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use roci::error::RociError;
use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::args::ParsedToolArgs;
use super::{ProcessRegistry, ToolContext};

const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, PartialEq, Eq)]
struct ExecRequest {
    command: String,
    cwd: Option<String>,
    env: HashMap<String, String>,
    background: bool,
    yield_ms: Option<u64>,
    timeout_secs: u64,
}

pub fn exec_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("command", "Shell command to execute", true)
        .string("cwd", "Working directory (optional)", false)
        .string(
            "yieldMs",
            "Milliseconds before returning and backgrounding",
            false,
        )
        .boolean("background", "Run in background immediately", false)
        .number("timeout", "Timeout in seconds", false)
        .build();

    Arc::new(AgentTool::new(
        "exec",
        "Run a shell command",
        params,
        move |args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { exec_impl(&ctx, &args).await }
        },
    ))
}

fn parse_exec_request(args: &ToolArguments) -> Result<ExecRequest, RociError> {
    let parsed = ParsedToolArgs::new(args)?;
    let command = clean_string(parsed.get_string_any(&["command", "cmd", "shell_command"])?)
        .or_else(|| clean_literal(parsed.literal()))
        .ok_or_else(|| RociError::InvalidArgument("command must not be empty".into()))?;
    let cwd = clean_string(parsed.get_string_any(&["cwd", "workdir", "working_dir"])?);
    let env = parsed
        .get_env_map_any(&["env", "environment"])?
        .unwrap_or_default();
    let yield_ms = parsed.get_u64_any(&["yieldMs", "yield_ms", "yield"])?;
    let background = parsed
        .get_bool_any(&["background", "detached"])?
        .unwrap_or(false)
        || yield_ms.is_some();
    let timeout_secs = parsed
        .get_u64_any(&["timeout", "timeout_secs", "timeoutSeconds"])?
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    Ok(ExecRequest {
        command,
        cwd,
        env,
        background,
        yield_ms,
        timeout_secs,
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

async fn exec_impl(
    ctx: &ToolContext,
    args: &ToolArguments,
) -> Result<serde_json::Value, RociError> {
    let parsed = parse_exec_request(args)?;
    let command = parsed.command;
    let cwd = resolve_cwd(ctx, parsed.cwd.as_deref());
    let env = parsed.env;
    let background = parsed.background;
    let yield_ms = parsed.yield_ms;
    let timeout_secs = parsed.timeout_secs;

    if super::debug_tools_enabled() {
        tracing::debug!(
            command = %command,
            cwd = %cwd.to_string_lossy(),
            background,
            yield_ms,
            timeout_secs,
            "exec tool invoked"
        );
    }

    if background {
        let id = spawn_background(ctx.processes.clone(), command.as_str(), &cwd, &env).await?;
        let info = ctx
            .processes
            .info(&id, 0)
            .ok_or_else(|| RociError::ToolExecution {
                tool_name: "exec".into(),
                message: "failed to load background process metadata".into(),
            })?;
        return Ok(serde_json::json!({
            "status": "running",
            "process_id": info.id,
            "pid": info.pid,
            "cwd": info.cwd,
            "command": info.command,
        }));
    }

    let start = Instant::now();
    let mut cmd = build_shell_command(command.as_str());
    cmd.current_dir(&cwd);
    if !env.is_empty() {
        cmd.envs(&env);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd.spawn().map_err(|e| RociError::ToolExecution {
        tool_name: "exec".into(),
        message: format!("spawn failed: {e}"),
    })?;

    let output = if timeout_secs > 0 {
        timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
            .await
            .map_err(|_| RociError::ToolExecution {
                tool_name: "exec".into(),
                message: format!("command timed out after {timeout_secs}s"),
            })??
    } else {
        child
            .wait_with_output()
            .await
            .map_err(|e| RociError::ToolExecution {
                tool_name: "exec".into(),
                message: format!("command failed: {e}"),
            })?
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout_truncated = output_truncated_flag(&output.stdout);
    let stderr_truncated = output_truncated_flag(&output.stderr);
    let stdout = truncate_output(&output.stdout);
    let stderr = truncate_output(&output.stderr);
    let process_id = if stdout_truncated || stderr_truncated {
        let combined = combine_output(&output.stdout, &output.stderr);
        Some(ctx.processes.insert_completed(
            command.clone(),
            cwd.to_string_lossy().to_string(),
            output.status.code(),
            combined,
        ))
    } else {
        None
    };
    Ok(serde_json::json!({
        "status": "completed",
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
        "duration_ms": duration_ms,
        "cwd": cwd.to_string_lossy(),
        "process_id": process_id,
        "stdout_truncated": stdout_truncated,
        "stderr_truncated": stderr_truncated,
    }))
}

async fn spawn_background(
    registry: Arc<ProcessRegistry>,
    command: &str,
    cwd: &PathBuf,
    env: &HashMap<String, String>,
) -> Result<String, RociError> {
    let mut cmd = build_shell_command(command);
    cmd.current_dir(cwd);
    if !env.is_empty() {
        cmd.envs(env);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| RociError::ToolExecution {
        tool_name: "exec".into(),
        message: format!("spawn failed: {e}"),
    })?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let id = registry.insert(
        command.to_string(),
        cwd.to_string_lossy().to_string(),
        child,
    );

    if let Some(mut out) = stdout {
        let registry = registry.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match out.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => registry.append_output(&id_clone, &buf[..n]),
                }
            }
        });
    }

    if let Some(mut err) = stderr {
        let registry = registry.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match err.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => registry.append_output(&id_clone, &buf[..n]),
                }
            }
        });
    }

    let registry = registry.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        loop {
            let exit = registry.try_wait(&id_clone);
            match exit {
                Some(Some(code)) => {
                    registry.mark_exited(&id_clone, Some(code));
                    break;
                }
                Some(None) => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                None => break,
            }
        }
    });

    Ok(id)
}

fn resolve_cwd(ctx: &ToolContext, override_path: Option<&str>) -> PathBuf {
    if let Some(path) = override_path {
        if let Some(resolved) = super::fs::resolve_path(path, &ctx.cwd) {
            return resolved;
        }
    }
    ctx.cwd.clone()
}

fn build_shell_command(command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let shell = detect_windows_shell();
        let mut cmd = Command::new(shell.program);
        cmd.args(shell.args(command));
        return cmd;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = Command::new(shell);
        cmd.args(["-lc", command]);
        cmd
    }
}

#[cfg(target_os = "windows")]
struct WindowsShell {
    program: String,
    kind: WindowsShellKind,
}

#[cfg(target_os = "windows")]
enum WindowsShellKind {
    Pwsh,
    Powershell,
    Cmd,
}

#[cfg(target_os = "windows")]
impl WindowsShell {
    fn args(&self, command: &str) -> Vec<String> {
        match self.kind {
            WindowsShellKind::Pwsh | WindowsShellKind::Powershell => vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                command.to_string(),
            ],
            WindowsShellKind::Cmd => vec!["/C".into(), command.to_string()],
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_shell() -> WindowsShell {
    if let Some(pwsh) = which_exe("pwsh.exe") {
        return WindowsShell {
            program: pwsh,
            kind: WindowsShellKind::Pwsh,
        };
    }
    if let Some(powershell) = which_exe("powershell.exe") {
        return WindowsShell {
            program: powershell,
            kind: WindowsShellKind::Powershell,
        };
    }
    WindowsShell {
        program: std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()),
        kind: WindowsShellKind::Cmd,
    }
}

#[cfg(target_os = "windows")]
fn which_exe(name: &str) -> Option<String> {
    let output = std::process::Command::new("where")
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .find(|line| !line.is_empty())
}

fn truncate_output(bytes: &[u8]) -> String {
    const MAX: usize = 200_000;
    if bytes.len() <= MAX {
        return String::from_utf8_lossy(bytes).to_string();
    }
    let slice = &bytes[bytes.len() - MAX..];
    String::from_utf8_lossy(slice).to_string()
}

fn output_truncated_flag(bytes: &[u8]) -> bool {
    bytes.len() > 200_000
}

fn combine_output(stdout: &[u8], stderr: &[u8]) -> Vec<u8> {
    let mut combined = Vec::new();
    if !stdout.is_empty() {
        combined.extend_from_slice(b"STDOUT:\n");
        combined.extend_from_slice(stdout);
        if !stdout.ends_with(b"\n") {
            combined.push(b'\n');
        }
    }
    if !stderr.is_empty() {
        combined.extend_from_slice(b"STDERR:\n");
        combined.extend_from_slice(stderr);
        if !stderr.ends_with(b"\n") {
            combined.push(b'\n');
        }
    }
    combined
}

#[cfg(test)]
mod tests {
    use roci::tools::ToolArguments;
    use serde_json::json;

    use super::{parse_exec_request, DEFAULT_TIMEOUT_SECS};

    #[test]
    fn exec_request_accepts_literal_command_with_defaults() {
        let args = ToolArguments::new(json!("echo hi"));
        let parsed = parse_exec_request(&args).expect("parse exec request");
        assert_eq!(parsed.command, "echo hi");
        assert_eq!(parsed.cwd, None);
        assert!(parsed.env.is_empty());
        assert!(!parsed.background);
        assert_eq!(parsed.yield_ms, None);
        assert_eq!(parsed.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn exec_request_accepts_aliases_and_numeric_drift() {
        let args = ToolArguments::new(json!({
            "cmd": "npm test",
            "workdir": "src",
            "environment": {
                "RETRIES": 2,
                "CI": true
            },
            "yield_ms": "125",
            "timeoutSeconds": "90"
        }));
        let parsed = parse_exec_request(&args).expect("parse exec request");
        assert_eq!(parsed.command, "npm test");
        assert_eq!(parsed.cwd.as_deref(), Some("src"));
        assert_eq!(parsed.env.get("RETRIES"), Some(&"2".to_string()));
        assert_eq!(parsed.env.get("CI"), Some(&"true".to_string()));
        assert_eq!(parsed.yield_ms, Some(125));
        assert!(parsed.background);
        assert_eq!(parsed.timeout_secs, 90);
    }

    #[test]
    fn exec_request_rejects_missing_command_with_clear_error() {
        let args = ToolArguments::new(json!({ "cwd": "." }));
        let err = parse_exec_request(&args).expect_err("missing command should fail");
        assert_eq!(
            err.to_string(),
            "Invalid argument: command must not be empty"
        );
    }
}

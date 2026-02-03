use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use roci::error::RociError;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};
use roci::tools::tool::ToolExecutionContext;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::{ProcessRegistry, ToolContext};

const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Deserialize)]
struct ExecArgs {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    background: Option<bool>,
    #[serde(default, rename = "yieldMs")]
    yield_ms: Option<u64>,
    #[serde(default)]
    timeout: Option<u64>,
}

pub fn exec_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string("command", "Shell command to execute", true)
        .string("cwd", "Working directory (optional)", false)
        .string("yieldMs", "Milliseconds before returning and backgrounding", false)
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

async fn exec_impl(ctx: &ToolContext, args: &ToolArguments) -> Result<serde_json::Value, RociError> {
    let parsed: ExecArgs = args.deserialize()?;
    let command = parsed.command.trim();
    if command.is_empty() {
        return Err(RociError::InvalidArgument("command must not be empty".into()));
    }
    let cwd = resolve_cwd(ctx, parsed.cwd.as_deref());
    let env = parsed.env.unwrap_or_default();
    let background = parsed.background.unwrap_or(false) || parsed.yield_ms.is_some();
    let timeout_secs = parsed.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);

    if super::debug_tools_enabled() {
        tracing::debug!(
            command = %command,
            cwd = %cwd.to_string_lossy(),
            background,
            timeout_secs,
            "exec tool invoked"
        );
    }

    if background {
        let id = spawn_background(ctx.processes.clone(), command, &cwd, &env).await?;
        let info = ctx.processes.info(&id, 0).unwrap();
        return Ok(serde_json::json!({
            "status": "running",
            "process_id": info.id,
            "pid": info.pid,
            "cwd": info.cwd,
            "command": info.command,
        }));
    }

    let start = Instant::now();
    let mut cmd = build_shell_command(command);
    cmd.current_dir(&cwd);
    if !env.is_empty() {
        cmd.envs(env);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        RociError::ToolExecution {
            tool_name: "exec".into(),
            message: format!("spawn failed: {e}"),
        }
    })?;

    let output = if timeout_secs > 0 {
        timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
            .await
            .map_err(|_| {
                RociError::ToolExecution {
                    tool_name: "exec".into(),
                    message: format!("command timed out after {timeout_secs}s"),
                }
            })??
    } else {
        child.wait_with_output().await.map_err(|e| RociError::ToolExecution {
            tool_name: "exec".into(),
            message: format!("command failed: {e}"),
        })?
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout = truncate_output(&output.stdout);
    let stderr = truncate_output(&output.stderr);
    Ok(serde_json::json!({
        "status": "completed",
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
        "duration_ms": duration_ms,
        "cwd": cwd.to_string_lossy(),
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
    let id = registry.insert(command.to_string(), cwd.to_string_lossy().to_string(), child);

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
    let output = std::process::Command::new("where").arg(name).output().ok()?;
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

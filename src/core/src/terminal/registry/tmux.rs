use std::process::Command;

use portable_pty::CommandBuilder;

use super::types::TerminalError;
use super::types::TmuxSessionInfo;
use super::{SessionInfo, TerminalRegistry};

pub(super) fn tmux_supported() -> bool {
    if cfg!(target_os = "windows") {
        return false;
    }
    Command::new("tmux").arg("-V").output().is_ok()
}

pub(super) fn is_tmux_shell(shell: &str) -> bool {
    shell.starts_with("tmux:")
}

pub(super) fn tmux_has_session(session_name: &str) -> Result<bool, TerminalError> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map_err(|e| TerminalError::Internal(format!("tmux has-session failed: {e}")))?;
    if output.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lowered = stderr.to_lowercase();
    if lowered.contains("no server running") || lowered.contains("no sessions") {
        return Ok(false);
    }
    Ok(false)
}

impl TerminalRegistry {
    pub fn list_tmux_sessions(&self) -> Result<(bool, Vec<TmuxSessionInfo>), TerminalError> {
        if !tmux_supported() {
            return Ok((false, Vec::new()));
        }
        let output = Command::new("tmux")
            .args([
                "list-sessions",
                "-F",
                "#{session_name}|#{session_windows}|#{session_attached}",
            ])
            .output()
            .map_err(|e| TerminalError::Internal(format!("tmux list failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let lowered = stderr.to_lowercase();
            if lowered.contains("no server running") || lowered.contains("no sessions") {
                return Ok((true, Vec::new()));
            }
            return Err(TerminalError::Internal(format!(
                "tmux list failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();
        for line in stdout.lines() {
            let mut parts = line.split('|');
            let name = match parts.next() {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => continue,
            };
            let windows = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let attached = parts.next().map(|v| v == "1").unwrap_or(false);
            sessions.push(TmuxSessionInfo {
                name,
                windows,
                attached,
            });
        }
        Ok((true, sessions))
    }

    pub fn attach_tmux_session(
        &mut self,
        session_name: String,
        cols: u16,
        rows: u16,
    ) -> Result<SessionInfo, TerminalError> {
        if !tmux_supported() {
            return Err(TerminalError::Internal("tmux not supported".into()));
        }
        if !tmux_has_session(&session_name)? {
            return Err(TerminalError::Missing(format!(
                "tmux session not found: {session_name}"
            )));
        }
        let mut cmd = CommandBuilder::new("tmux");
        cmd.arg("attach");
        cmd.arg("-t");
        cmd.arg(&session_name);
        let display = format!("tmux:{session_name}");
        self.start_session_with_command(display, cmd, cols, rows, Some(session_name))
    }

    pub fn kill_tmux_session(&self, session_name: String) -> Result<(), TerminalError> {
        if !tmux_supported() {
            return Err(TerminalError::Internal("tmux not supported".into()));
        }
        let output = Command::new("tmux")
            .args(["kill-session", "-t", &session_name])
            .output()
            .map_err(|e| TerminalError::Internal(format!("tmux kill failed: {e}")))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lowered = stderr.to_lowercase();
        if lowered.contains("no server running") || lowered.contains("no sessions") {
            return Err(TerminalError::Missing(format!(
                "tmux session not found: {session_name}"
            )));
        }
        Err(TerminalError::Internal(format!(
            "tmux kill failed: {stderr}"
        )))
    }
}

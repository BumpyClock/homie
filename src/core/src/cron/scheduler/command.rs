use tokio::process::Command;

use crate::storage::CronRunStatus;

use super::common::MAX_OUTPUT_BYTES;

pub(super) async fn execute_command(
    command: &str,
) -> Result<(CronRunStatus, Option<i64>, Option<String>, Option<String>), String> {
    #[cfg(unix)]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.arg("-lc").arg(command);
        c
    };

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    };

    let command_output = cmd
        .output()
        .await
        .map_err(|err| format!("failed to run cron command: {err}"))?;

    let mut combined = String::new();
    if !command_output.stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&command_output.stdout));
    }
    if !command_output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&command_output.stderr));
    }
    if combined.len() > MAX_OUTPUT_BYTES {
        combined.truncate(MAX_OUTPUT_BYTES);
    }

    if command_output.status.success() {
        let output = if combined.is_empty() {
            None
        } else {
            Some(combined)
        };
        Ok((
            CronRunStatus::Succeeded,
            output_status_code(&command_output.status),
            output,
            None,
        ))
    } else {
        let err = if combined.is_empty() {
            Some("command exited with failure".to_string())
        } else {
            Some(combined.clone())
        };
        let output = if combined.is_empty() {
            None
        } else {
            Some(combined)
        };
        Ok((
            CronRunStatus::Failed,
            output_status_code(&command_output.status),
            output,
            err,
        ))
    }
}

fn output_status_code(status: &std::process::ExitStatus) -> Option<i64> {
    status.code().map(|v| v as i64)
}

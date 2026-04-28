use portable_pty::CommandBuilder;

pub(super) fn build_shell_command(shell: &str) -> (String, CommandBuilder) {
    #[cfg(target_os = "windows")]
    {
        let raw = shell.trim();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(raw);

        let lower = unquoted.to_ascii_lowercase();
        let marker = "cmd.exe";
        if let Some(pos) = lower.find(marker) {
            let exe = unquoted[..pos + marker.len()].trim().to_string();
            let rest = unquoted[pos + marker.len()..].trim();

            // Special-case: allow "cmd.exe /d" to be passed as a single string (common mistake).
            // Also default to "/d" for cmd to avoid AutoRun side effects.
            if rest.is_empty() || rest.eq_ignore_ascii_case("/d") {
                let mut cmd = CommandBuilder::new(&exe);
                cmd.arg("/d");
                return (format!("{exe} /d"), cmd);
            }
        }

        (shell.to_string(), CommandBuilder::new(shell))
    }

    #[cfg(not(target_os = "windows"))]
    {
        (shell.to_string(), CommandBuilder::new(shell))
    }
}

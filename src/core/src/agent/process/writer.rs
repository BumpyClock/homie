use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Background task: serializes outbound lines to the Codex stdin pipe.
pub(super) async fn writer_loop(
    mut stdin: tokio::process::ChildStdin,
    mut rx: mpsc::Receiver<String>,
) {
    while let Some(line) = rx.recv().await {
        let mut data = line.into_bytes();
        data.push(b'\n');
        if let Err(e) = stdin.write_all(&data).await {
            tracing::warn!("codex stdin write error: {e}");
            break;
        }
        if let Err(e) = stdin.flush().await {
            tracing::warn!("codex stdin flush error: {e}");
            break;
        }
    }

    tracing::debug!("codex writer loop exited");
}

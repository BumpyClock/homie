use std::io::{Read, Write};
use std::thread::JoinHandle;

use portable_pty::{Child, MasterPty, PtySize};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Holds the PTY master, writer, child process, and reader thread for one
/// terminal session. Dropping the runtime triggers graceful shutdown.
pub struct SessionRuntime {
    pub id: Uuid,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Box<dyn Child + Send + Sync>,
    reader_handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl SessionRuntime {
    pub fn new(
        id: Uuid,
        master: Box<dyn MasterPty + Send>,
        writer: Box<dyn Write + Send>,
        child: Box<dyn Child + Send + Sync>,
        reader_handle: JoinHandle<()>,
        shutdown_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            id,
            master: Some(master),
            writer: Some(writer),
            child,
            reader_handle: Some(reader_handle),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Write raw bytes to the PTY (terminal input).
    pub fn write_input(&mut self, data: &[u8]) -> std::io::Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "writer closed"))?;
        writer.write_all(data)?;
        writer.flush()
    }

    /// Resize the PTY.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        let master = self
            .master
            .as_ref()
            .ok_or_else(|| "master closed".to_string())?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    /// Graceful shutdown: signal reader, kill child, join reader thread.
    pub fn shutdown(&mut self) {
        // Signal reader thread to stop.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Drop writer + master to unblock any blocking reads.
        self.writer.take();
        self.master.take();

        // Kill child process.
        let _ = self.child.kill();

        // Wait for child exit (best-effort).
        for _ in 0..10 {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                _ => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }

        // Join reader thread.
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the child process has exited. Returns exit code if so.
    pub fn try_wait(&mut self) -> Option<u32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.exit_code())
    }

    /// Spawn the reader thread that reads PTY output and sends it via an mpsc
    /// channel. The thread exits on EOF, read error, or shutdown signal.
    pub fn spawn_reader(
        master: &dyn MasterPty,
        session_id: Uuid,
        output_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Result<JoinHandle<()>, String> {
        let mut reader = master
            .try_clone_reader()
            .map_err(|e| format!("clone reader: {e}"))?;

        let handle = std::thread::Builder::new()
            .name(format!("pty-reader-{}", &session_id.to_string()[..8]))
            .spawn(move || {
                // Convert oneshot into a pollable flag.
                let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let flag = shutdown.clone();
                std::thread::spawn(move || {
                    let _ = shutdown_rx.blocking_recv();
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                });

                let mut buf = [0u8; 32768];
                loop {
                    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            if output_tx.blocking_send(buf[..n].to_vec()).is_err() {
                                break; // receiver dropped
                            }
                        }
                        Err(e) => {
                            // On shutdown, broken-pipe / interrupted are expected.
                            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            tracing::warn!(session = %session_id, "pty read error: {e}");
                            break;
                        }
                    }
                }
                tracing::debug!(session = %session_id, "reader thread exited");
            })
            .map_err(|e| format!("spawn reader thread: {e}"))?;

        Ok(handle)
    }
}

impl Drop for SessionRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

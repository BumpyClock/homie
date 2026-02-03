use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use uuid::Uuid;

const MAX_OUTPUT_BYTES: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Exited,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub id: String,
    pub command: String,
    pub cwd: String,
    pub started_at: DateTime<Utc>,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub exit_code: Option<i32>,
    pub output_tail: String,
}

struct ProcessEntry {
    id: String,
    command: String,
    cwd: String,
    started_at: DateTime<Utc>,
    pid: Option<u32>,
    status: ProcessStatus,
    exit_code: Option<i32>,
    output: Vec<u8>,
    child: Option<tokio::process::Child>,
}

#[derive(Default)]
pub struct ProcessRegistry {
    inner: Mutex<HashMap<String, ProcessEntry>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &self,
        command: String,
        cwd: String,
        child: tokio::process::Child,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let pid = child.id();
        let entry = ProcessEntry {
            id: id.clone(),
            command,
            cwd,
            started_at: Utc::now(),
            pid,
            status: ProcessStatus::Running,
            exit_code: None,
            output: Vec::new(),
            child: Some(child),
        };
        let mut guard = self.inner.lock().unwrap();
        guard.insert(id.clone(), entry);
        id
    }

    pub fn append_output(&self, id: &str, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        let mut guard = self.inner.lock().unwrap();
        let Some(entry) = guard.get_mut(id) else { return; };
        if entry.output.len() + chunk.len() > MAX_OUTPUT_BYTES {
            let overflow = entry.output.len() + chunk.len() - MAX_OUTPUT_BYTES;
            if overflow >= entry.output.len() {
                entry.output.clear();
            } else {
                entry.output.drain(0..overflow);
            }
        }
        entry.output.extend_from_slice(chunk);
    }

    pub fn mark_exited(&self, id: &str, exit_code: Option<i32>) {
        let mut guard = self.inner.lock().unwrap();
        let Some(entry) = guard.get_mut(id) else { return; };
        entry.status = ProcessStatus::Exited;
        entry.exit_code = exit_code;
        entry.child = None;
    }

    pub async fn kill(&self, id: &str) -> Result<(), String> {
        let child = {
            let mut guard = self.inner.lock().unwrap();
            let Some(entry) = guard.get_mut(id) else {
                return Err("process not found".to_string());
            };
            let Some(child) = entry.child.take() else {
                return Err("process already exited".to_string());
            };
            child
        };
        let mut child = child;
        child.kill().await.map_err(|e| format!("kill failed: {e}"))?;
        Ok(())
    }

    pub fn try_wait(&self, id: &str) -> Option<Option<i32>> {
        let mut guard = self.inner.lock().unwrap();
        let entry = guard.get_mut(id)?;
        if entry.status == ProcessStatus::Exited {
            return Some(entry.exit_code);
        }
        let child = entry.child.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                entry.status = ProcessStatus::Exited;
                entry.exit_code = status.code();
                entry.child = None;
                Some(entry.exit_code)
            }
            Ok(None) => Some(None),
            Err(_) => Some(None),
        }
    }

    pub fn list(&self, tail_bytes: usize) -> Vec<ProcessInfo> {
        let guard = self.inner.lock().unwrap();
        guard
            .values()
            .map(|entry| ProcessInfo {
                id: entry.id.clone(),
                command: entry.command.clone(),
                cwd: entry.cwd.clone(),
                started_at: entry.started_at,
                pid: entry.pid,
                status: entry.status,
                exit_code: entry.exit_code,
                output_tail: tail_bytes_to_string(&entry.output, tail_bytes),
            })
            .collect()
    }

    pub fn info(&self, id: &str, tail_bytes: usize) -> Option<ProcessInfo> {
        let guard = self.inner.lock().unwrap();
        let entry = guard.get(id)?;
        Some(ProcessInfo {
            id: entry.id.clone(),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
            started_at: entry.started_at,
            pid: entry.pid,
            status: entry.status,
            exit_code: entry.exit_code,
            output_tail: tail_bytes_to_string(&entry.output, tail_bytes),
        })
    }
}

fn tail_bytes_to_string(buf: &[u8], tail_bytes: usize) -> String {
    let slice = if tail_bytes == 0 || buf.len() <= tail_bytes {
        buf
    } else {
        &buf[buf.len() - tail_bytes..]
    };
    String::from_utf8_lossy(slice).to_string()
}

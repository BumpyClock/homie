use std::collections::VecDeque;

pub(super) const HISTORY_CHUNK_BYTES: usize = 16 * 1024;
const DEFAULT_HISTORY_BYTES: usize = 2 * 1024 * 1024;

pub(super) struct HistoryBuffer {
    data: VecDeque<u8>,
    max_bytes: usize,
}

impl HistoryBuffer {
    pub(super) fn new(max_bytes: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_bytes,
        }
    }

    pub(super) fn push(&mut self, chunk: &[u8]) {
        if self.max_bytes == 0 || chunk.is_empty() {
            return;
        }
        if chunk.len() >= self.max_bytes {
            self.data.clear();
            self.data
                .extend(chunk[chunk.len() - self.max_bytes..].iter().copied());
            return;
        }
        while self.data.len() + chunk.len() > self.max_bytes {
            self.data.pop_front();
        }
        self.data.extend(chunk.iter().copied());
    }

    pub(super) fn snapshot(&self) -> Vec<u8> {
        if self.data.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.data.len());
        out.extend(self.data.iter().copied());
        out
    }
}

pub(super) fn history_limit_bytes() -> usize {
    std::env::var("HOMIE_HISTORY_BYTES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_HISTORY_BYTES)
}

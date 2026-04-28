pub(super) const SCHEDULER_TICK_SECS: u64 = 1;
pub(super) const MAX_MISSED_RUNS: usize = 32;
pub(super) const MAX_OUTPUT_BYTES: usize = 16_384;

pub(super) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

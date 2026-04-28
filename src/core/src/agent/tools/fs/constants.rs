use tokio::time::Duration;

pub(super) const DEFAULT_READ_LIMIT: usize = 2000;
pub(super) const DEFAULT_LS_LIMIT: usize = 200;
pub(super) const DEFAULT_LS_DEPTH: usize = 2;
pub(super) const DEFAULT_GREP_LIMIT: usize = 100;
pub(super) const MAX_GREP_LIMIT: usize = 2000;
pub(super) const DEFAULT_FIND_LIMIT: usize = 200;
pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

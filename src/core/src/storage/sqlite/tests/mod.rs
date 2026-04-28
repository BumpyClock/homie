use super::*;
use crate::storage::types::{JobStatus, PairingStatus, SessionStatus};
use rusqlite::params;

fn make_store() -> SqliteStore {
    SqliteStore::open_memory().unwrap()
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

mod chat;
mod cron;
mod jobs;
mod notifications;
mod pairings;
mod raw_events;
mod terminal;

#[test]
fn migration_creates_query_indexes() {
    let store = make_store();
    let conn = store.conn.lock().unwrap();
    for (table, index) in [
        ("chat_raw_events", "idx_chat_raw_events_thread_created"),
        ("chat_raw_events", "idx_chat_raw_events_run_id"),
        ("chat_runs", "idx_chat_runs_started_at"),
        ("cron_runs", "idx_cron_runs_scheduled_at"),
        ("cron_runs", "idx_cron_runs_finished_at"),
        ("jobs", "idx_jobs_created_at"),
    ] {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_index_list(?1) WHERE name = ?2",
                params![table, index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing index {index} on {table}");
    }
}

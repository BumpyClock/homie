use rusqlite::params;
use uuid::Uuid;

use crate::storage::types::ChatRawEventRecord;

use super::{now_unix, parse_chat_thread_state_json, SqliteStore, MAX_RAW_EVENT_BYTES};

pub(super) fn insert_chat_raw_event(
    store: &SqliteStore,
    run_id: &str,
    thread_id: &str,
    method: &str,
    params: &serde_json::Value,
) -> Result<(), String> {
    let params_json =
        serde_json::to_string(params).map_err(|e| format!("serialize raw event: {e}"))?;
    if params_json.len() > MAX_RAW_EVENT_BYTES {
        return Ok(());
    }
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let started_at = now_unix();
    conn.execute(
        "INSERT OR IGNORE INTO chat_runs (run_id, thread_id, started_at)
         VALUES (?1, ?2, ?3)",
        params![run_id, thread_id, started_at as i64],
    )
    .map_err(|e| format!("insert_chat_raw_event run: {e}"))?;
    conn.execute(
        "INSERT INTO chat_raw_events (event_id, run_id, thread_id, method, params_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            run_id,
            thread_id,
            method,
            params_json,
            started_at as i64
        ],
    )
    .map_err(|e| format!("insert_chat_raw_event event: {e}"))?;
    Ok(())
}

pub(super) fn list_chat_raw_events(
    store: &SqliteStore,
    thread_id: &str,
    limit: usize,
) -> Result<Vec<ChatRawEventRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let max_rows = limit.clamp(1, 10_000) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT run_id, thread_id, method, params_json, created_at
             FROM chat_raw_events
             WHERE thread_id = ?1
             ORDER BY created_at ASC, rowid ASC
             LIMIT ?2",
        )
        .map_err(|e| format!("list_chat_raw_events prepare: {e}"))?;

    let rows = stmt
        .query_map(params![thread_id, max_rows], |row| {
            let raw: String = row.get(3)?;
            Ok(ChatRawEventRecord {
                run_id: row.get(0)?,
                thread_id: row.get(1)?,
                method: row.get(2)?,
                params: parse_chat_thread_state_json(raw)?,
                created_at: row.get::<_, i64>(4)? as u64,
            })
        })
        .map_err(|e| format!("list_chat_raw_events query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_chat_raw_events collect: {e}"))
}

pub(super) fn prune_chat_raw_events(store: &SqliteStore, max_runs: usize) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "DELETE FROM chat_raw_events
         WHERE run_id NOT IN (
            SELECT run_id FROM chat_runs ORDER BY started_at DESC LIMIT ?1
         )",
        params![max_runs as i64],
    )
    .map_err(|e| format!("prune_chat_raw_events events: {e}"))?;
    conn.execute(
        "DELETE FROM chat_runs
         WHERE run_id NOT IN (
            SELECT run_id FROM chat_runs ORDER BY started_at DESC LIMIT ?1
         )",
        params![max_runs as i64],
    )
    .map_err(|e| format!("prune_chat_raw_events runs: {e}"))?;
    Ok(())
}

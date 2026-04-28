use rusqlite::params;

use crate::storage::types::{CronRecord, CronRunRecord, CronRunStatus, CronStatus};

use super::{now_unix, SqliteStore};

pub(super) fn upsert_cron(store: &SqliteStore, cron: &CronRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "INSERT INTO cron_jobs (cron_id, name, schedule, command, status, skip_overlap, created_at, updated_at, last_run_at, next_run_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(cron_id) DO UPDATE SET
            name = excluded.name,
            schedule = excluded.schedule,
            command = excluded.command,
            status = excluded.status,
            skip_overlap = excluded.skip_overlap,
            updated_at = excluded.updated_at,
            last_run_at = excluded.last_run_at,
            next_run_at = excluded.next_run_at",
        params![
            cron.cron_id,
            cron.name,
            cron.schedule,
            cron.command,
            cron.status.as_str(),
            cron.skip_overlap,
            cron.created_at as i64,
            cron.updated_at as i64,
            cron.last_run_at.map(|v| v as i64),
            cron.next_run_at.map(|v| v as i64),
        ],
    )
    .map_err(|e| format!("upsert_cron: {e}"))?;
    Ok(())
}

pub(super) fn get_cron(store: &SqliteStore, cron_id: &str) -> Result<Option<CronRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT cron_id, name, schedule, command, status, skip_overlap, created_at,
                    updated_at, last_run_at, next_run_at
             FROM cron_jobs WHERE cron_id = ?1",
        )
        .map_err(|e| format!("get_cron prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![cron_id], |row| {
            Ok(CronRecord {
                cron_id: row.get(0)?,
                name: row.get(1)?,
                schedule: row.get(2)?,
                command: row.get(3)?,
                status: CronStatus::from_label(&row.get::<_, String>(4)?),
                skip_overlap: row.get(5)?,
                created_at: row.get::<_, i64>(6)? as u64,
                updated_at: row.get::<_, i64>(7)? as u64,
                last_run_at: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                next_run_at: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
            })
        })
        .map_err(|e| format!("get_cron query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_cron row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_crons(store: &SqliteStore) -> Result<Vec<CronRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT cron_id, name, schedule, command, status, skip_overlap, created_at,
                    updated_at, last_run_at, next_run_at
             FROM cron_jobs
             ORDER BY created_at DESC",
        )
        .map_err(|e| format!("list_crons prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CronRecord {
                cron_id: row.get(0)?,
                name: row.get(1)?,
                schedule: row.get(2)?,
                command: row.get(3)?,
                status: CronStatus::from_label(&row.get::<_, String>(4)?),
                skip_overlap: row.get(5)?,
                created_at: row.get::<_, i64>(6)? as u64,
                updated_at: row.get::<_, i64>(7)? as u64,
                last_run_at: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                next_run_at: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
            })
        })
        .map_err(|e| format!("list_crons query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_crons collect: {e}"))
}

pub(super) fn delete_cron(store: &SqliteStore, cron_id: &str) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute("DELETE FROM cron_jobs WHERE cron_id = ?1", params![cron_id])
        .map_err(|e| format!("delete_cron: {e}"))?;
    conn.execute("DELETE FROM cron_runs WHERE cron_id = ?1", params![cron_id])
        .map_err(|e| format!("delete_cron runs: {e}"))?;
    Ok(())
}

pub(super) fn upsert_cron_run(store: &SqliteStore, run: &CronRunRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "INSERT INTO cron_runs (
            run_id, cron_id, scheduled_at, started_at, finished_at, status, exit_code, output, error
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(run_id) DO UPDATE SET
            scheduled_at = excluded.scheduled_at,
            started_at = excluded.started_at,
            finished_at = excluded.finished_at,
            status = excluded.status,
            exit_code = excluded.exit_code,
            output = excluded.output,
            error = excluded.error",
        params![
            run.run_id,
            run.cron_id,
            run.scheduled_at as i64,
            run.started_at.map(|v| v as i64),
            run.finished_at.map(|v| v as i64),
            run.status.as_str(),
            run.exit_code,
            run.output,
            run.error,
        ],
    )
    .map_err(|e| format!("upsert_cron_run: {e}"))?;
    Ok(())
}

pub(super) fn get_cron_run(
    store: &SqliteStore,
    run_id: &str,
) -> Result<Option<CronRunRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT run_id, cron_id, scheduled_at, started_at, finished_at, status, exit_code, output, error
             FROM cron_runs WHERE run_id = ?1",
        )
        .map_err(|e| format!("get_cron_run prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![run_id], |row| {
            Ok(CronRunRecord {
                run_id: row.get(0)?,
                cron_id: row.get(1)?,
                scheduled_at: row.get::<_, i64>(2)? as u64,
                started_at: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                finished_at: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                status: CronRunStatus::from_label(&row.get::<_, String>(5)?),
                exit_code: row.get(6)?,
                output: row.get(7)?,
                error: row.get(8)?,
            })
        })
        .map_err(|e| format!("get_cron_run query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_cron_run row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_cron_runs(
    store: &SqliteStore,
    cron_id: &str,
    limit: usize,
) -> Result<Vec<CronRunRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let max_rows = limit.clamp(1, 10_000) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT run_id, cron_id, scheduled_at, started_at, finished_at, status, exit_code, output, error
             FROM cron_runs
             WHERE cron_id = ?1
             ORDER BY scheduled_at DESC, rowid DESC
             LIMIT ?2",
        )
        .map_err(|e| format!("list_cron_runs prepare: {e}"))?;

    let rows = stmt
        .query_map(params![cron_id, max_rows], |row| {
            Ok(CronRunRecord {
                run_id: row.get(0)?,
                cron_id: row.get(1)?,
                scheduled_at: row.get::<_, i64>(2)? as u64,
                started_at: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                finished_at: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                status: CronRunStatus::from_label(&row.get::<_, String>(5)?),
                exit_code: row.get(6)?,
                output: row.get(7)?,
                error: row.get(8)?,
            })
        })
        .map_err(|e| format!("list_cron_runs query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_cron_runs collect: {e}"))
}

pub(super) fn list_latest_cron_runs(
    store: &SqliteStore,
    limit: usize,
) -> Result<Vec<CronRunRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let max_rows = limit.clamp(1, 10_000) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT run_id, cron_id, scheduled_at, started_at, finished_at, status, exit_code, output, error
             FROM cron_runs
             ORDER BY scheduled_at DESC, rowid DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("list_latest_cron_runs prepare: {e}"))?;

    let rows = stmt
        .query_map(params![max_rows], |row| {
            Ok(CronRunRecord {
                run_id: row.get(0)?,
                cron_id: row.get(1)?,
                scheduled_at: row.get::<_, i64>(2)? as u64,
                started_at: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                finished_at: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                status: CronRunStatus::from_label(&row.get::<_, String>(5)?),
                exit_code: row.get(6)?,
                output: row.get(7)?,
                error: row.get(8)?,
            })
        })
        .map_err(|e| format!("list_latest_cron_runs query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_latest_cron_runs collect: {e}"))
}

pub(super) fn get_cron_last_run(
    store: &SqliteStore,
    cron_id: &str,
) -> Result<Option<CronRunRecord>, String> {
    let runs = list_cron_runs(store, cron_id, 1)?;
    Ok(runs.into_iter().next())
}

pub(super) fn cron_has_running(store: &SqliteStore, cron_id: &str) -> Result<bool, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT 1 FROM cron_runs WHERE cron_id = ?1 AND status = 'running' LIMIT 1")
        .map_err(|e| format!("cron_has_running prepare: {e}"))?;
    let mut rows = stmt
        .query_map(params![cron_id], |row| row.get::<_, i32>(0))
        .map_err(|e| format!("cron_has_running query: {e}"))?;
    Ok(rows.next().is_some())
}

pub(super) fn prune_cron_runs(
    store: &SqliteStore,
    retention_days: u64,
    max_runs: usize,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let cutoff = now_unix().saturating_sub(retention_days.saturating_mul(86_400));
    conn.execute(
        "DELETE FROM cron_runs WHERE finished_at IS NOT NULL AND finished_at < ?1",
        params![cutoff as i64],
    )
    .map_err(|e| format!("prune_cron_runs cutoff: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT DISTINCT cron_id FROM cron_runs")
        .map_err(|e| format!("prune_cron_runs cron_ids: {e}"))?;
    let cron_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("prune_cron_runs list crons: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("prune_cron_runs collect crons: {e}"))?;

    for cron_id in cron_ids {
        let mut stmt = conn
            .prepare(
                "SELECT run_id FROM cron_runs
                 WHERE cron_id = ?1
                 ORDER BY scheduled_at DESC, rowid DESC
                 LIMIT -1 OFFSET ?2",
            )
            .map_err(|e| format!("prune_cron_runs runs: {e}"))?;
        let run_ids = stmt
            .query_map(params![cron_id, max_runs as i64], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| format!("prune_cron_runs run ids: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("prune_cron_runs run id collect: {e}"))?;

        for run_id in run_ids {
            conn.execute("DELETE FROM cron_runs WHERE run_id = ?1", params![run_id])
                .map_err(|e| format!("prune_cron_runs delete run: {e}"))?;
        }
    }
    Ok(())
}

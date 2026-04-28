use rusqlite::params;

use crate::storage::types::{JobRecord, JobStatus};

use super::{now_unix, SqliteStore};

pub(super) fn upsert_job(store: &SqliteStore, job: &JobRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let spec = serde_json::to_string(&job.spec).map_err(|e| format!("job spec: {e}"))?;
    let logs = serde_json::to_string(&job.logs).map_err(|e| format!("job logs: {e}"))?;
    conn.execute(
        "INSERT INTO jobs (job_id, name, status, created_at, updated_at, spec, logs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(job_id) DO UPDATE SET
            name = excluded.name,
            status = excluded.status,
            updated_at = excluded.updated_at,
            spec = excluded.spec,
            logs = excluded.logs",
        params![
            job.job_id,
            job.name,
            job.status.as_str(),
            job.created_at as i64,
            job.updated_at as i64,
            spec,
            logs,
        ],
    )
    .map_err(|e| format!("upsert_job: {e}"))?;
    Ok(())
}

pub(super) fn get_job(store: &SqliteStore, job_id: &str) -> Result<Option<JobRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT job_id, name, status, created_at, updated_at, spec, logs
             FROM jobs WHERE job_id = ?1",
        )
        .map_err(|e| format!("get_job prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![job_id], |row| {
            let spec: String = row.get(5)?;
            let logs: String = row.get(6)?;
            let spec = serde_json::from_str(&spec).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let logs = serde_json::from_str(&logs).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(JobRecord {
                job_id: row.get(0)?,
                name: row.get(1)?,
                status: JobStatus::from_label(&row.get::<_, String>(2)?),
                created_at: row.get::<_, i64>(3)? as u64,
                updated_at: row.get::<_, i64>(4)? as u64,
                spec,
                logs,
            })
        })
        .map_err(|e| format!("get_job query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_job row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_jobs(store: &SqliteStore) -> Result<Vec<JobRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT job_id, name, status, created_at, updated_at, spec, logs
             FROM jobs ORDER BY created_at DESC",
        )
        .map_err(|e| format!("list_jobs prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let spec: String = row.get(5)?;
            let logs: String = row.get(6)?;
            let spec = serde_json::from_str(&spec).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let logs = serde_json::from_str(&logs).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(JobRecord {
                job_id: row.get(0)?,
                name: row.get(1)?,
                status: JobStatus::from_label(&row.get::<_, String>(2)?),
                created_at: row.get::<_, i64>(3)? as u64,
                updated_at: row.get::<_, i64>(4)? as u64,
                spec,
                logs,
            })
        })
        .map_err(|e| format!("list_jobs query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_jobs collect: {e}"))
}

pub(super) fn prune_jobs(
    store: &SqliteStore,
    retention_days: u64,
    max_jobs: usize,
) -> Result<(), String> {
    let cutoff = now_unix().saturating_sub(retention_days.saturating_mul(86_400));
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "DELETE FROM jobs WHERE created_at < ?1",
        params![cutoff as i64],
    )
    .map_err(|e| format!("prune_jobs: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT job_id FROM jobs ORDER BY created_at DESC")
        .map_err(|e| format!("prune_jobs prepare: {e}"))?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("prune_jobs query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("prune_jobs collect: {e}"))?;

    if ids.len() > max_jobs {
        for job_id in ids.iter().skip(max_jobs) {
            conn.execute("DELETE FROM jobs WHERE job_id = ?1", params![job_id])
                .map_err(|e| format!("prune_jobs delete: {e}"))?;
        }
    }

    Ok(())
}

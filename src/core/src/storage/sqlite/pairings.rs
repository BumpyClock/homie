use rusqlite::params;

use crate::storage::types::{PairingRecord, PairingStatus};

use super::{now_unix, SqliteStore};

pub(super) fn upsert_pairing(store: &SqliteStore, pairing: &PairingRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "INSERT INTO pairings (pairing_id, nonce, status, created_at, expires_at, approved_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(pairing_id) DO UPDATE SET
            status = excluded.status,
            expires_at = excluded.expires_at,
            approved_by = excluded.approved_by",
        params![
            pairing.pairing_id,
            pairing.nonce,
            pairing.status.as_str(),
            pairing.created_at as i64,
            pairing.expires_at as i64,
            pairing.approved_by,
        ],
    )
    .map_err(|e| format!("upsert_pairing: {e}"))?;
    Ok(())
}

pub(super) fn get_pairing(
    store: &SqliteStore,
    pairing_id: &str,
) -> Result<Option<PairingRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT pairing_id, nonce, status, created_at, expires_at, approved_by
             FROM pairings WHERE pairing_id = ?1",
        )
        .map_err(|e| format!("get_pairing prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![pairing_id], |row| {
            Ok(PairingRecord {
                pairing_id: row.get(0)?,
                nonce: row.get(1)?,
                status: PairingStatus::from_label(&row.get::<_, String>(2)?),
                created_at: row.get::<_, i64>(3)? as u64,
                expires_at: row.get::<_, i64>(4)? as u64,
                approved_by: row.get(5)?,
            })
        })
        .map_err(|e| format!("get_pairing query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_pairing row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_pairings(store: &SqliteStore) -> Result<Vec<PairingRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT pairing_id, nonce, status, created_at, expires_at, approved_by
             FROM pairings ORDER BY created_at DESC",
        )
        .map_err(|e| format!("list_pairings prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PairingRecord {
                pairing_id: row.get(0)?,
                nonce: row.get(1)?,
                status: PairingStatus::from_label(&row.get::<_, String>(2)?),
                created_at: row.get::<_, i64>(3)? as u64,
                expires_at: row.get::<_, i64>(4)? as u64,
                approved_by: row.get(5)?,
            })
        })
        .map_err(|e| format!("list_pairings query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_pairings collect: {e}"))
}

pub(super) fn prune_pairings(store: &SqliteStore, retention_secs: u64) -> Result<(), String> {
    let now = now_unix();
    let cutoff = now.saturating_sub(retention_secs);
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "UPDATE pairings SET status = 'expired'
         WHERE status IN ('pending', 'approved') AND expires_at < ?1",
        params![now as i64],
    )
    .map_err(|e| format!("prune_pairings update: {e}"))?;

    conn.execute(
        "DELETE FROM pairings WHERE expires_at < ?1",
        params![cutoff as i64],
    )
    .map_err(|e| format!("prune_pairings delete: {e}"))?;
    Ok(())
}

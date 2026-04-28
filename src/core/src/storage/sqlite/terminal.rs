use rusqlite::{params, types::Type};
use uuid::Uuid;

use crate::storage::types::{SessionStatus, TerminalRecord};

use super::SqliteStore;

pub(super) fn upsert_terminal(store: &SqliteStore, rec: &TerminalRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "INSERT INTO terminals (session_id, name, shell, cols, rows, started_at, status, exit_code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(session_id) DO UPDATE SET
            name = excluded.name,
            status = excluded.status,
            exit_code = excluded.exit_code,
            cols = excluded.cols,
            rows = excluded.rows",
        params![
            rec.session_id.to_string(),
            rec.name,
            rec.shell,
            rec.cols,
            rec.rows,
            rec.started_at,
            rec.status.as_str(),
            rec.exit_code,
        ],
    )
    .map_err(|e| format!("upsert_terminal: {e}"))?;
    Ok(())
}

pub(super) fn get_terminal(
    store: &SqliteStore,
    session_id: Uuid,
) -> Result<Option<TerminalRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT session_id, name, shell, cols, rows, started_at, status, exit_code
             FROM terminals WHERE session_id = ?1",
        )
        .map_err(|e| format!("get_terminal prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![session_id.to_string()], |row| {
            let sid: String = row.get(0)?;
            Ok(TerminalRecord {
                session_id: parse_terminal_session_id(&sid)?,
                name: row.get(1)?,
                shell: row.get(2)?,
                cols: row.get::<_, u32>(3)? as u16,
                rows: row.get::<_, u32>(4)? as u16,
                started_at: row.get(5)?,
                status: SessionStatus::from_label(&row.get::<_, String>(6)?),
                exit_code: row.get(7)?,
            })
        })
        .map_err(|e| format!("get_terminal query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_terminal row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_terminals(store: &SqliteStore) -> Result<Vec<TerminalRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT session_id, name, shell, cols, rows, started_at, status, exit_code
             FROM terminals ORDER BY started_at DESC",
        )
        .map_err(|e| format!("list_terminals prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let sid: String = row.get(0)?;
            Ok(TerminalRecord {
                session_id: parse_terminal_session_id(&sid)?,
                name: row.get(1)?,
                shell: row.get(2)?,
                cols: row.get::<_, u32>(3)? as u16,
                rows: row.get::<_, u32>(4)? as u16,
                started_at: row.get(5)?,
                status: SessionStatus::from_label(&row.get::<_, String>(6)?),
                exit_code: row.get(7)?,
            })
        })
        .map_err(|e| format!("list_terminals query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_terminals collect: {e}"))
}

pub(super) fn delete_terminal(store: &SqliteStore, session_id: Uuid) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "DELETE FROM terminals WHERE session_id = ?1",
        [session_id.to_string()],
    )
    .map_err(|e| format!("delete_terminal: {e}"))?;
    Ok(())
}

pub(super) fn mark_all_inactive(store: &SqliteStore) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute_batch(
        "UPDATE chats SET status = 'inactive' WHERE status = 'active';
         UPDATE terminals SET status = 'inactive' WHERE status = 'active';",
    )
    .map_err(|e| format!("mark_all_inactive: {e}"))?;
    Ok(())
}

fn parse_terminal_session_id(raw: &str) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(raw)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(e)))
}

use rusqlite::{params, OptionalExtension};

use crate::storage::types::{ChatRecord, SessionStatus};

use super::{
    now_unix, parse_chat_thread_state_json, parse_settings_json, serialize_chat_thread_state,
    serialize_settings, SqliteStore,
};

pub(super) fn upsert_chat(store: &SqliteStore, chat: &ChatRecord) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let settings_json = serialize_settings(chat.settings.as_ref())?;
    conn.execute(
        "INSERT INTO chats (chat_id, thread_id, created_at, status, event_pointer, settings_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(chat_id) DO UPDATE SET
            thread_id = excluded.thread_id,
            status = excluded.status,
            event_pointer = excluded.event_pointer,
            settings_json = COALESCE(excluded.settings_json, chats.settings_json)",
        params![
            chat.chat_id,
            chat.thread_id,
            chat.created_at,
            chat.status.as_str(),
            chat.event_pointer as i64,
            settings_json,
        ],
    )
    .map_err(|e| format!("upsert_chat: {e}"))?;
    Ok(())
}

pub(super) fn get_chat(store: &SqliteStore, chat_id: &str) -> Result<Option<ChatRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT chat_id, thread_id, created_at, status, event_pointer, settings_json
             FROM chats WHERE chat_id = ?1",
        )
        .map_err(|e| format!("get_chat prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![chat_id], |row| {
            let settings = parse_settings_json(row.get(5)?)?;
            Ok(ChatRecord {
                chat_id: row.get(0)?,
                thread_id: row.get(1)?,
                created_at: row.get(2)?,
                status: SessionStatus::from_label(&row.get::<_, String>(3)?),
                event_pointer: row.get::<_, i64>(4)? as u64,
                settings,
            })
        })
        .map_err(|e| format!("get_chat query: {e}"))?;

    match rows.next() {
        Some(Ok(rec)) => Ok(Some(rec)),
        Some(Err(e)) => Err(format!("get_chat row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn list_chats(store: &SqliteStore) -> Result<Vec<ChatRecord>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT chat_id, thread_id, created_at, status, event_pointer, settings_json
             FROM chats ORDER BY created_at DESC",
        )
        .map_err(|e| format!("list_chats prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let settings = parse_settings_json(row.get(5)?)?;
            Ok(ChatRecord {
                chat_id: row.get(0)?,
                thread_id: row.get(1)?,
                created_at: row.get(2)?,
                status: SessionStatus::from_label(&row.get::<_, String>(3)?),
                event_pointer: row.get::<_, i64>(4)? as u64,
                settings,
            })
        })
        .map_err(|e| format!("list_chats query: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_chats collect: {e}"))
}

pub(super) fn delete_chat(store: &SqliteStore, chat_id: &str) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute("DELETE FROM chats WHERE chat_id = ?1", params![chat_id])
        .map_err(|e| format!("delete_chat: {e}"))?;
    Ok(())
}

pub(super) fn update_event_pointer(
    store: &SqliteStore,
    chat_id: &str,
    pointer: u64,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let updated = conn
        .execute(
            "UPDATE chats SET event_pointer = ?1 WHERE chat_id = ?2 AND event_pointer < ?1",
            params![pointer as i64, chat_id],
        )
        .map_err(|e| format!("update_event_pointer: {e}"))?;
    if updated == 1 {
        return Ok(());
    }

    let existing = conn
        .query_row(
            "SELECT event_pointer FROM chats WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| format!("update_event_pointer lookup: {e}"))?;

    match existing {
        Some(_) => Ok(()),
        None => Err(format!("update_event_pointer: missing chat {chat_id}")),
    }
}

pub(super) fn update_chat_settings(
    store: &SqliteStore,
    chat_id: &str,
    settings: Option<&serde_json::Value>,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let settings_json = serialize_settings(settings)?;
    conn.execute(
        "UPDATE chats SET settings_json = ?1 WHERE chat_id = ?2",
        params![settings_json, chat_id],
    )
    .map_err(|e| format!("update_chat_settings: {e}"))?;
    Ok(())
}

pub(super) fn upsert_chat_thread_state(
    store: &SqliteStore,
    thread_id: &str,
    state: &serde_json::Value,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let state_json = serialize_chat_thread_state(state)?;
    conn.execute(
        "INSERT INTO chat_thread_states (thread_id, state_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(thread_id) DO UPDATE SET
            state_json = excluded.state_json,
            updated_at = excluded.updated_at",
        params![thread_id, state_json, now_unix() as i64],
    )
    .map_err(|e| format!("upsert_chat_thread_state: {e}"))?;
    Ok(())
}

pub(super) fn get_chat_thread_state(
    store: &SqliteStore,
    thread_id: &str,
) -> Result<Option<serde_json::Value>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT state_json FROM chat_thread_states WHERE thread_id = ?1")
        .map_err(|e| format!("get_chat_thread_state prepare: {e}"))?;

    let mut rows = stmt
        .query_map(params![thread_id], |row| {
            let raw: String = row.get(0)?;
            parse_chat_thread_state_json(raw)
        })
        .map_err(|e| format!("get_chat_thread_state query: {e}"))?;

    match rows.next() {
        Some(Ok(state)) => Ok(Some(state)),
        Some(Err(e)) => Err(format!("get_chat_thread_state row: {e}")),
        None => Ok(None),
    }
}

pub(super) fn delete_chat_thread_state(store: &SqliteStore, thread_id: &str) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    conn.execute(
        "DELETE FROM chat_thread_states WHERE thread_id = ?1",
        params![thread_id],
    )
    .map_err(|e| format!("delete_chat_thread_state: {e}"))?;
    Ok(())
}

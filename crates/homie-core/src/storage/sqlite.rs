use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use uuid::Uuid;

use super::types::{ChatRecord, SessionStatus, TerminalRecord};
use super::Store;

/// SQLite-backed store for chat + terminal metadata.
///
/// Uses a `Mutex<Connection>` for thread-safe interior mutability.
/// The database is created/migrated on `open()`.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open (or create) a sqlite database at the given path.
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("sqlite open: {e}"))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory database (useful for tests).
    pub fn open_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("sqlite open: {e}"))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chats (
                chat_id       TEXT PRIMARY KEY,
                thread_id     TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                status        TEXT NOT NULL DEFAULT 'active',
                event_pointer INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS terminals (
                session_id  TEXT PRIMARY KEY,
                shell       TEXT NOT NULL,
                cols        INTEGER NOT NULL,
                rows        INTEGER NOT NULL,
                started_at  TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'active',
                exit_code   INTEGER
            );
            ",
        )
        .map_err(|e| format!("migrate: {e}"))?;

        Ok(())
    }
}

impl Store for SqliteStore {
    fn upsert_chat(&self, chat: &ChatRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO chats (chat_id, thread_id, created_at, status, event_pointer)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(chat_id) DO UPDATE SET
                thread_id = excluded.thread_id,
                status = excluded.status,
                event_pointer = excluded.event_pointer",
            params![
                chat.chat_id,
                chat.thread_id,
                chat.created_at,
                chat.status.as_str(),
                chat.event_pointer as i64,
            ],
        )
        .map_err(|e| format!("upsert_chat: {e}"))?;
        Ok(())
    }

    fn get_chat(&self, chat_id: &str) -> Result<Option<ChatRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT chat_id, thread_id, created_at, status, event_pointer
                 FROM chats WHERE chat_id = ?1",
            )
            .map_err(|e| format!("get_chat prepare: {e}"))?;

        let mut rows = stmt
            .query_map(params![chat_id], |row| {
                Ok(ChatRecord {
                    chat_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    created_at: row.get(2)?,
                    status: SessionStatus::from_label(&row.get::<_, String>(3)?),
                    event_pointer: row.get::<_, i64>(4)? as u64,
                })
            })
            .map_err(|e| format!("get_chat query: {e}"))?;

        match rows.next() {
            Some(Ok(rec)) => Ok(Some(rec)),
            Some(Err(e)) => Err(format!("get_chat row: {e}")),
            None => Ok(None),
        }
    }

    fn list_chats(&self) -> Result<Vec<ChatRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT chat_id, thread_id, created_at, status, event_pointer
                 FROM chats ORDER BY created_at DESC",
            )
            .map_err(|e| format!("list_chats prepare: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ChatRecord {
                    chat_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    created_at: row.get(2)?,
                    status: SessionStatus::from_label(&row.get::<_, String>(3)?),
                    event_pointer: row.get::<_, i64>(4)? as u64,
                })
            })
            .map_err(|e| format!("list_chats query: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list_chats collect: {e}"))
    }

    fn update_event_pointer(&self, chat_id: &str, pointer: u64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "UPDATE chats SET event_pointer = ?1 WHERE chat_id = ?2",
            params![pointer as i64, chat_id],
        )
        .map_err(|e| format!("update_event_pointer: {e}"))?;
        Ok(())
    }

    fn upsert_terminal(&self, rec: &TerminalRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO terminals (session_id, shell, cols, rows, started_at, status, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                status = excluded.status,
                exit_code = excluded.exit_code,
                cols = excluded.cols,
                rows = excluded.rows",
            params![
                rec.session_id.to_string(),
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

    fn get_terminal(&self, session_id: Uuid) -> Result<Option<TerminalRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, shell, cols, rows, started_at, status, exit_code
                 FROM terminals WHERE session_id = ?1",
            )
            .map_err(|e| format!("get_terminal prepare: {e}"))?;

        let mut rows = stmt
            .query_map(params![session_id.to_string()], |row| {
                let sid: String = row.get(0)?;
                Ok(TerminalRecord {
                    session_id: sid.parse().unwrap_or(Uuid::nil()),
                    shell: row.get(1)?,
                    cols: row.get::<_, u32>(2)? as u16,
                    rows: row.get::<_, u32>(3)? as u16,
                    started_at: row.get(4)?,
                    status: SessionStatus::from_label(&row.get::<_, String>(5)?),
                    exit_code: row.get(6)?,
                })
            })
            .map_err(|e| format!("get_terminal query: {e}"))?;

        match rows.next() {
            Some(Ok(rec)) => Ok(Some(rec)),
            Some(Err(e)) => Err(format!("get_terminal row: {e}")),
            None => Ok(None),
        }
    }

    fn list_terminals(&self) -> Result<Vec<TerminalRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, shell, cols, rows, started_at, status, exit_code
                 FROM terminals ORDER BY started_at DESC",
            )
            .map_err(|e| format!("list_terminals prepare: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let sid: String = row.get(0)?;
                Ok(TerminalRecord {
                    session_id: sid.parse().unwrap_or(Uuid::nil()),
                    shell: row.get(1)?,
                    cols: row.get::<_, u32>(2)? as u16,
                    rows: row.get::<_, u32>(3)? as u16,
                    started_at: row.get(4)?,
                    status: SessionStatus::from_label(&row.get::<_, String>(5)?),
                    exit_code: row.get(6)?,
                })
            })
            .map_err(|e| format!("list_terminals query: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list_terminals collect: {e}"))
    }

    fn mark_all_inactive(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute_batch(
            "UPDATE chats SET status = 'inactive' WHERE status = 'active';
             UPDATE terminals SET status = 'inactive' WHERE status = 'active';",
        )
        .map_err(|e| format!("mark_all_inactive: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> SqliteStore {
        SqliteStore::open_memory().unwrap()
    }

    #[test]
    fn upsert_and_get_chat() {
        let store = make_store();
        let chat = ChatRecord {
            chat_id: "c1".into(),
            thread_id: "t1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 0,
        };
        store.upsert_chat(&chat).unwrap();

        let loaded = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(loaded.chat_id, "c1");
        assert_eq!(loaded.thread_id, "t1");
        assert_eq!(loaded.status, SessionStatus::Active);
    }

    #[test]
    fn upsert_chat_updates_on_conflict() {
        let store = make_store();
        let chat = ChatRecord {
            chat_id: "c1".into(),
            thread_id: "t1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 0,
        };
        store.upsert_chat(&chat).unwrap();

        let updated = ChatRecord {
            status: SessionStatus::Inactive,
            event_pointer: 42,
            ..chat
        };
        store.upsert_chat(&updated).unwrap();

        let loaded = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(loaded.status, SessionStatus::Inactive);
        assert_eq!(loaded.event_pointer, 42);
    }

    #[test]
    fn list_chats_ordered() {
        let store = make_store();
        for (id, ts) in [("a", "100s"), ("b", "200s"), ("c", "150s")] {
            store
                .upsert_chat(&ChatRecord {
                    chat_id: id.into(),
                    thread_id: id.into(),
                    created_at: ts.into(),
                    status: SessionStatus::Active,
                    event_pointer: 0,
                })
                .unwrap();
        }
        let chats = store.list_chats().unwrap();
        assert_eq!(chats.len(), 3);
        // DESC order by created_at (string sort: "200s" > "150s" > "100s")
        assert_eq!(chats[0].chat_id, "b");
        assert_eq!(chats[1].chat_id, "c");
        assert_eq!(chats[2].chat_id, "a");
    }

    #[test]
    fn update_event_pointer() {
        let store = make_store();
        store
            .upsert_chat(&ChatRecord {
                chat_id: "c1".into(),
                thread_id: "t1".into(),
                created_at: "100s".into(),
                status: SessionStatus::Active,
                event_pointer: 0,
            })
            .unwrap();

        store.update_event_pointer("c1", 99).unwrap();
        let loaded = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(loaded.event_pointer, 99);
    }

    #[test]
    fn upsert_and_get_terminal() {
        let store = make_store();
        let sid = Uuid::new_v4();
        let rec = TerminalRecord {
            session_id: sid,
            shell: "/bin/bash".into(),
            cols: 80,
            rows: 24,
            started_at: "100s".into(),
            status: SessionStatus::Active,
            exit_code: None,
        };
        store.upsert_terminal(&rec).unwrap();

        let loaded = store.get_terminal(sid).unwrap().unwrap();
        assert_eq!(loaded.session_id, sid);
        assert_eq!(loaded.shell, "/bin/bash");
        assert_eq!(loaded.status, SessionStatus::Active);
        assert_eq!(loaded.exit_code, None);
    }

    #[test]
    fn upsert_terminal_updates_status() {
        let store = make_store();
        let sid = Uuid::new_v4();
        let rec = TerminalRecord {
            session_id: sid,
            shell: "/bin/bash".into(),
            cols: 80,
            rows: 24,
            started_at: "100s".into(),
            status: SessionStatus::Active,
            exit_code: None,
        };
        store.upsert_terminal(&rec).unwrap();

        let updated = TerminalRecord {
            status: SessionStatus::Exited,
            exit_code: Some(0),
            ..rec
        };
        store.upsert_terminal(&updated).unwrap();

        let loaded = store.get_terminal(sid).unwrap().unwrap();
        assert_eq!(loaded.status, SessionStatus::Exited);
        assert_eq!(loaded.exit_code, Some(0));
    }

    #[test]
    fn list_terminals_ordered() {
        let store = make_store();
        for (i, ts) in [(1, "100s"), (2, "200s"), (3, "150s")] {
            store
                .upsert_terminal(&TerminalRecord {
                    session_id: Uuid::from_u128(i),
                    shell: "/bin/sh".into(),
                    cols: 80,
                    rows: 24,
                    started_at: ts.into(),
                    status: SessionStatus::Active,
                    exit_code: None,
                })
                .unwrap();
        }
        let terms = store.list_terminals().unwrap();
        assert_eq!(terms.len(), 3);
        assert_eq!(terms[0].session_id, Uuid::from_u128(2));
    }

    #[test]
    fn mark_all_inactive() {
        let store = make_store();
        store
            .upsert_chat(&ChatRecord {
                chat_id: "c1".into(),
                thread_id: "t1".into(),
                created_at: "100s".into(),
                status: SessionStatus::Active,
                event_pointer: 0,
            })
            .unwrap();
        let sid = Uuid::new_v4();
        store
            .upsert_terminal(&TerminalRecord {
                session_id: sid,
                shell: "/bin/sh".into(),
                cols: 80,
                rows: 24,
                started_at: "100s".into(),
                status: SessionStatus::Active,
                exit_code: None,
            })
            .unwrap();

        store.mark_all_inactive().unwrap();

        let chat = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(chat.status, SessionStatus::Inactive);
        let term = store.get_terminal(sid).unwrap().unwrap();
        assert_eq!(term.status, SessionStatus::Inactive);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = make_store();
        assert!(store.get_chat("nonexistent").unwrap().is_none());
        assert!(store.get_terminal(Uuid::new_v4()).unwrap().is_none());
    }

    #[test]
    fn mark_all_inactive_skips_exited() {
        let store = make_store();
        store
            .upsert_chat(&ChatRecord {
                chat_id: "c1".into(),
                thread_id: "t1".into(),
                created_at: "100s".into(),
                status: SessionStatus::Exited,
                event_pointer: 5,
            })
            .unwrap();

        store.mark_all_inactive().unwrap();

        let chat = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(chat.status, SessionStatus::Exited);
    }
}

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use uuid::Uuid;

use super::types::{
    ChatRecord, JobRecord, JobStatus, NotificationEvent, NotificationSubscription, PairingRecord,
    PairingStatus, SessionStatus, TerminalRecord,
};
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

            CREATE TABLE IF NOT EXISTS jobs (
                job_id      TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                status      TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                spec        TEXT NOT NULL,
                logs        TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS pairings (
                pairing_id  TEXT PRIMARY KEY,
                nonce       TEXT NOT NULL,
                status      TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                approved_by TEXT
            );

            CREATE TABLE IF NOT EXISTS notification_subscriptions (
                subscription_id TEXT PRIMARY KEY,
                target          TEXT NOT NULL,
                kind            TEXT,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notification_events (
                notification_id TEXT PRIMARY KEY,
                title           TEXT NOT NULL,
                body            TEXT NOT NULL,
                target          TEXT,
                created_at      INTEGER NOT NULL
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

    fn upsert_job(&self, job: &JobRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_jobs(&self) -> Result<Vec<JobRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn prune_jobs(&self, retention_days: u64, max_jobs: usize) -> Result<(), String> {
        let cutoff = now_unix().saturating_sub(retention_days.saturating_mul(86_400));
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn upsert_pairing(&self, pairing: &PairingRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_pairing(&self, pairing_id: &str) -> Result<Option<PairingRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_pairings(&self) -> Result<Vec<PairingRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn prune_pairings(&self, retention_secs: u64) -> Result<(), String> {
        let now = now_unix();
        let cutoff = now.saturating_sub(retention_secs);
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn upsert_notification_subscription(
        &self,
        subscription: &NotificationSubscription,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO notification_subscriptions (subscription_id, target, kind, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(subscription_id) DO UPDATE SET
                target = excluded.target,
                kind = excluded.kind,
                updated_at = excluded.updated_at",
            params![
                subscription.subscription_id,
                subscription.target,
                subscription.kind,
                subscription.created_at as i64,
                subscription.updated_at as i64,
            ],
        )
        .map_err(|e| format!("upsert_notification_subscription: {e}"))?;
        Ok(())
    }

    fn list_notification_subscriptions(&self) -> Result<Vec<NotificationSubscription>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT subscription_id, target, kind, created_at, updated_at
                 FROM notification_subscriptions ORDER BY created_at DESC",
            )
            .map_err(|e| format!("list_notification_subscriptions prepare: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(NotificationSubscription {
                    subscription_id: row.get(0)?,
                    target: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get::<_, i64>(3)? as u64,
                    updated_at: row.get::<_, i64>(4)? as u64,
                })
            })
            .map_err(|e| format!("list_notification_subscriptions query: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list_notification_subscriptions collect: {e}"))
    }

    fn has_notification_target(&self, target: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT 1 FROM notification_subscriptions WHERE target = ?1 LIMIT 1")
            .map_err(|e| format!("has_notification_target prepare: {e}"))?;
        let mut rows = stmt
            .query_map(params![target], |row| row.get::<_, i32>(0))
            .map_err(|e| format!("has_notification_target query: {e}"))?;
        Ok(rows.next().is_some())
    }

    fn insert_notification_event(&self, event: &NotificationEvent) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "INSERT INTO notification_events (notification_id, title, body, target, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.notification_id,
                event.title,
                event.body,
                event.target,
                event.created_at as i64,
            ],
        )
        .map_err(|e| format!("insert_notification_event: {e}"))?;
        Ok(())
    }

    fn prune_notifications(&self, retention_days: u64) -> Result<(), String> {
        let cutoff = now_unix().saturating_sub(retention_days.saturating_mul(86_400));
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "DELETE FROM notification_events WHERE created_at < ?1",
            params![cutoff as i64],
        )
        .map_err(|e| format!("prune_notifications events: {e}"))?;
        conn.execute(
            "DELETE FROM notification_subscriptions WHERE updated_at < ?1",
            params![cutoff as i64],
        )
        .map_err(|e| format!("prune_notifications subs: {e}"))?;
        Ok(())
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> SqliteStore {
        SqliteStore::open_memory().unwrap()
    }

    fn now_unix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
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

    #[test]
    fn upsert_and_get_job() {
        let store = make_store();
        let now = now_unix();
        let job = JobRecord {
            job_id: "job-1".into(),
            name: "build".into(),
            status: JobStatus::Queued,
            created_at: now,
            updated_at: now,
            spec: serde_json::json!({"kind": "command", "command": "echo"}),
            logs: vec!["line1".into(), "line2".into()],
        };
        store.upsert_job(&job).unwrap();

        let loaded = store.get_job("job-1").unwrap().unwrap();
        assert_eq!(loaded.name, "build");
        assert_eq!(loaded.status, JobStatus::Queued);
        assert_eq!(loaded.logs.len(), 2);
    }

    #[test]
    fn prune_jobs_removes_old_and_excess() {
        let store = make_store();
        let now = now_unix();
        let old = JobRecord {
            job_id: "old".into(),
            name: "old".into(),
            status: JobStatus::Succeeded,
            created_at: now.saturating_sub(900_000),
            updated_at: now,
            spec: serde_json::json!({}),
            logs: vec![],
        };
        let recent = JobRecord {
            job_id: "new".into(),
            name: "new".into(),
            status: JobStatus::Queued,
            created_at: now,
            updated_at: now,
            spec: serde_json::json!({}),
            logs: vec![],
        };
        store.upsert_job(&old).unwrap();
        store.upsert_job(&recent).unwrap();

        store.prune_jobs(1, 1).unwrap();
        let jobs = store.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "new");
    }

    #[test]
    fn upsert_and_get_pairing() {
        let store = make_store();
        let now = now_unix();
        let pairing = PairingRecord {
            pairing_id: "p1".into(),
            nonce: "n1".into(),
            status: PairingStatus::Pending,
            created_at: now,
            expires_at: now + 60,
            approved_by: None,
        };
        store.upsert_pairing(&pairing).unwrap();

        let loaded = store.get_pairing("p1").unwrap().unwrap();
        assert_eq!(loaded.status, PairingStatus::Pending);
    }

    #[test]
    fn prune_pairings_expires_and_deletes() {
        let store = make_store();
        let now = now_unix();
        let expired = PairingRecord {
            pairing_id: "old".into(),
            nonce: "n".into(),
            status: PairingStatus::Pending,
            created_at: now,
            expires_at: now.saturating_sub(10),
            approved_by: None,
        };
        let deleted = PairingRecord {
            pairing_id: "gone".into(),
            nonce: "n2".into(),
            status: PairingStatus::Pending,
            created_at: now,
            expires_at: now.saturating_sub(10_000),
            approved_by: None,
        };
        store.upsert_pairing(&expired).unwrap();
        store.upsert_pairing(&deleted).unwrap();

        store.prune_pairings(3600).unwrap();
        let pairings = store.list_pairings().unwrap();
        assert_eq!(pairings.len(), 1);
        assert_eq!(pairings[0].pairing_id, "old");
        assert_eq!(pairings[0].status, PairingStatus::Expired);
    }

    #[test]
    fn notifications_store_and_prune() {
        let store = make_store();
        let now = now_unix();
        let sub = NotificationSubscription {
            subscription_id: "s1".into(),
            target: "device-1".into(),
            kind: None,
            created_at: now,
            updated_at: now,
        };
        store.upsert_notification_subscription(&sub).unwrap();
        assert!(store.has_notification_target("device-1").unwrap());

        let event = NotificationEvent {
            notification_id: "e1".into(),
            title: "t".into(),
            body: "b".into(),
            target: Some("device-1".into()),
            created_at: now.saturating_sub(900_000),
        };
        store.insert_notification_event(&event).unwrap();

        store.prune_notifications(1).unwrap();
        let subs = store.list_notification_subscriptions().unwrap();
        assert_eq!(subs.len(), 1);
    }
}

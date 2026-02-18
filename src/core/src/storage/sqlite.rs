use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, types::Type, Connection};
use uuid::Uuid;

use super::types::{
    ChatRawEventRecord, ChatRecord, CronRecord, CronRunRecord, CronRunStatus, CronStatus,
    JobRecord, JobStatus, NotificationEvent, NotificationSubscription, PairingRecord,
    PairingStatus, SessionStatus, TerminalRecord,
};
use super::Store;

const MAX_RAW_EVENT_BYTES: usize = 64 * 1024;

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
                event_pointer INTEGER NOT NULL DEFAULT 0,
                settings_json TEXT
            );

            CREATE TABLE IF NOT EXISTS chat_thread_states (
                thread_id  TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS terminals (
                session_id  TEXT PRIMARY KEY,
                name        TEXT,
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

            CREATE TABLE IF NOT EXISTS chat_runs (
                run_id      TEXT PRIMARY KEY,
                thread_id   TEXT NOT NULL,
                started_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chat_raw_events (
                event_id    TEXT PRIMARY KEY,
                run_id      TEXT NOT NULL,
                thread_id   TEXT NOT NULL,
                method      TEXT NOT NULL,
                params_json TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cron_jobs (
                cron_id       TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                schedule      TEXT NOT NULL,
                command       TEXT NOT NULL,
                status        TEXT NOT NULL DEFAULT 'active',
                skip_overlap  INTEGER NOT NULL DEFAULT 1,
                created_at    INTEGER NOT NULL,
                updated_at    INTEGER NOT NULL,
                last_run_at   INTEGER,
                next_run_at   INTEGER
            );

            CREATE TABLE IF NOT EXISTS cron_runs (
                run_id       TEXT PRIMARY KEY,
                cron_id      TEXT NOT NULL,
                scheduled_at INTEGER NOT NULL,
                started_at   INTEGER,
                finished_at  INTEGER,
                status       TEXT NOT NULL DEFAULT 'queued',
                exit_code    INTEGER,
                output       TEXT,
                error        TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_cron_runs_cron_id
                ON cron_runs (cron_id, scheduled_at DESC);
            ",
        )
        .map_err(|e| format!("migrate: {e}"))?;

        if let Err(e) = conn.execute("ALTER TABLE terminals ADD COLUMN name TEXT", []) {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("duplicate column") {
                return Err(format!("migrate add terminals.name: {e}"));
            }
        }

        if let Err(e) = conn.execute("ALTER TABLE chats ADD COLUMN settings_json TEXT", []) {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("duplicate column") {
                return Err(format!("migrate add chats.settings_json: {e}"));
            }
        }

        Ok(())
    }
}

impl Store for SqliteStore {
    fn upsert_chat(&self, chat: &ChatRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_chat(&self, chat_id: &str) -> Result<Option<ChatRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_chats(&self) -> Result<Vec<ChatRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn delete_chat(&self, chat_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute("DELETE FROM chats WHERE chat_id = ?1", params![chat_id])
            .map_err(|e| format!("delete_chat: {e}"))?;
        Ok(())
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

    fn update_chat_settings(
        &self,
        chat_id: &str,
        settings: Option<&serde_json::Value>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let settings_json = serialize_settings(settings)?;
        conn.execute(
            "UPDATE chats SET settings_json = ?1 WHERE chat_id = ?2",
            params![settings_json, chat_id],
        )
        .map_err(|e| format!("update_chat_settings: {e}"))?;
        Ok(())
    }

    fn upsert_chat_thread_state(
        &self,
        thread_id: &str,
        state: &serde_json::Value,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_chat_thread_state(&self, thread_id: &str) -> Result<Option<serde_json::Value>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn delete_chat_thread_state(&self, thread_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "DELETE FROM chat_thread_states WHERE thread_id = ?1",
            params![thread_id],
        )
        .map_err(|e| format!("delete_chat_thread_state: {e}"))?;
        Ok(())
    }

    fn upsert_terminal(&self, rec: &TerminalRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_terminal(&self, session_id: Uuid) -> Result<Option<TerminalRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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
                    session_id: sid.parse().unwrap_or(Uuid::nil()),
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

    fn list_terminals(&self) -> Result<Vec<TerminalRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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
                    session_id: sid.parse().unwrap_or(Uuid::nil()),
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

    fn delete_terminal(&self, session_id: Uuid) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            "DELETE FROM terminals WHERE session_id = ?1",
            [session_id.to_string()],
        )
        .map_err(|e| format!("delete_terminal: {e}"))?;
        Ok(())
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

    fn insert_chat_raw_event(
        &self,
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
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_chat_raw_events(
        &self,
        thread_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatRawEventRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let max_rows = limit.max(1).min(10_000) as i64;
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

    fn prune_chat_raw_events(&self, max_runs: usize) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn upsert_cron(&self, cron: &CronRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_cron(&self, cron_id: &str) -> Result<Option<CronRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_crons(&self) -> Result<Vec<CronRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn delete_cron(&self, cron_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute("DELETE FROM cron_jobs WHERE cron_id = ?1", params![cron_id])
            .map_err(|e| format!("delete_cron: {e}"))?;
        conn.execute("DELETE FROM cron_runs WHERE cron_id = ?1", params![cron_id])
            .map_err(|e| format!("delete_cron runs: {e}"))?;
        Ok(())
    }

    fn upsert_cron_run(&self, run: &CronRunRecord) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn get_cron_run(&self, run_id: &str) -> Result<Option<CronRunRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

    fn list_cron_runs(&self, cron_id: &str, limit: usize) -> Result<Vec<CronRunRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let max_rows = limit.max(1).min(10_000) as i64;
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

    fn list_latest_cron_runs(&self, limit: usize) -> Result<Vec<CronRunRecord>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let max_rows = limit.max(1).min(10_000) as i64;
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

    fn get_cron_last_run(&self, cron_id: &str) -> Result<Option<CronRunRecord>, String> {
        let runs = self.list_cron_runs(cron_id, 1)?;
        Ok(runs.into_iter().next())
    }

    fn cron_has_running(&self, cron_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT 1 FROM cron_runs WHERE cron_id = ?1 AND status = 'running' LIMIT 1")
            .map_err(|e| format!("cron_has_running prepare: {e}"))?;
        let mut rows = stmt
            .query_map(params![cron_id], |row| row.get::<_, i32>(0))
            .map_err(|e| format!("cron_has_running query: {e}"))?;
        Ok(rows.next().is_some())
    }

    fn prune_cron_runs(&self, retention_days: u64, max_runs: usize) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
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
}

fn parse_settings_json(raw: Option<String>) -> Result<Option<serde_json::Value>, rusqlite::Error> {
    match raw {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(e))),
        None => Ok(None),
    }
}

fn serialize_settings(settings: Option<&serde_json::Value>) -> Result<Option<String>, String> {
    match settings {
        Some(value) => serde_json::to_string(value)
            .map(Some)
            .map_err(|e| format!("serialize chat settings: {e}")),
        None => Ok(None),
    }
}

fn serialize_chat_thread_state(state: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(state).map_err(|e| format!("serialize chat thread state: {e}"))
}

fn parse_chat_thread_state_json(raw: String) -> Result<serde_json::Value, rusqlite::Error> {
    serde_json::from_str(&raw)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(e)))
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
            settings: None,
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
            settings: None,
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
                    settings: None,
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
                settings: None,
            })
            .unwrap();

        store.update_event_pointer("c1", 99).unwrap();
        let loaded = store.get_chat("c1").unwrap().unwrap();
        assert_eq!(loaded.event_pointer, 99);
    }

    #[test]
    fn chat_thread_state_roundtrip_save_load_delete() {
        let store = make_store();
        let thread_id = "thread-1";
        let first = serde_json::json!({
            "cursor": 12,
            "provider": "roci",
            "pending": ["tool-a", "tool-b"]
        });

        store
            .upsert_chat_thread_state(thread_id, &first)
            .expect("save first state");
        let loaded_first = store
            .get_chat_thread_state(thread_id)
            .expect("load first state")
            .expect("missing first state");
        assert_eq!(loaded_first, first);

        let second = serde_json::json!({
            "cursor": 33,
            "provider": "roci",
            "pending": []
        });
        store
            .upsert_chat_thread_state(thread_id, &second)
            .expect("save second state");
        let loaded_second = store
            .get_chat_thread_state(thread_id)
            .expect("load second state")
            .expect("missing second state");
        assert_eq!(loaded_second, second);

        store
            .delete_chat_thread_state(thread_id)
            .expect("delete state");
        let deleted = store
            .get_chat_thread_state(thread_id)
            .expect("load deleted state");
        assert!(deleted.is_none());
    }

    #[test]
    fn upsert_and_get_terminal() {
        let store = make_store();
        let sid = Uuid::new_v4();
        let rec = TerminalRecord {
            session_id: sid,
            name: None,
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
            name: None,
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
                    name: None,
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
                settings: None,
            })
            .unwrap();
        let sid = Uuid::new_v4();
        store
            .upsert_terminal(&TerminalRecord {
                session_id: sid,
                name: None,
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
                settings: None,
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

    #[test]
    fn raw_event_pruning_keeps_the_latest_runs() {
        let store = make_store();
        let params = serde_json::json!({"ok": true});
        store
            .insert_chat_raw_event("run-1", "thread-1", "m1", &params)
            .unwrap();
        store
            .insert_chat_raw_event("run-2", "thread-2", "m2", &params)
            .unwrap();
        store
            .insert_chat_raw_event("run-3", "thread-3", "m3", &params)
            .unwrap();

        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE chat_runs SET started_at = ?1 WHERE run_id = ?2",
                params![1_i64, "run-1"],
            )
            .unwrap();
            conn.execute(
                "UPDATE chat_runs SET started_at = ?1 WHERE run_id = ?2",
                params![2_i64, "run-2"],
            )
            .unwrap();
            conn.execute(
                "UPDATE chat_runs SET started_at = ?1 WHERE run_id = ?2",
                params![3_i64, "run-3"],
            )
            .unwrap();
        }

        store.prune_chat_raw_events(2).unwrap();

        let conn = store.conn.lock().unwrap();
        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chat_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(run_count, 2);
        let run1_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_runs WHERE run_id = 'run-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(run1_count, 0);
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_raw_events WHERE run_id = 'run-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 0);
    }

    #[test]
    fn list_chat_raw_events_returns_thread_events_in_order() {
        let store = make_store();
        let thread_id = "thread-list";
        store
            .insert_chat_raw_event(
                "run-list",
                thread_id,
                "turn/started",
                &serde_json::json!({"threadId": thread_id, "turnId": "t1"}),
            )
            .unwrap();
        store
            .insert_chat_raw_event(
                "run-list",
                thread_id,
                "item/started",
                &serde_json::json!({
                    "threadId": thread_id,
                    "turnId": "t1",
                    "item": {"id":"u1","type":"userMessage","content":[{"type":"text","text":"hello"}]}
                }),
            )
            .unwrap();
        store
            .insert_chat_raw_event(
                "run-other",
                "thread-other",
                "turn/started",
                &serde_json::json!({"threadId": "thread-other", "turnId": "tx"}),
            )
            .unwrap();

        let events = store.list_chat_raw_events(thread_id, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].method, "turn/started");
        assert_eq!(events[1].method, "item/started");
        assert_eq!(events[0].thread_id, thread_id);
        assert_eq!(events[1].thread_id, thread_id);
    }

    #[test]
    fn cron_upsert_and_get() {
        let store = make_store();
        let now = now_unix();
        let cron = CronRecord {
            cron_id: "cron-1".into(),
            name: "heartbeat".into(),
            schedule: "* * * * * *".into(),
            command: "echo hi".into(),
            status: CronStatus::Active,
            skip_overlap: true,
            created_at: now,
            updated_at: now,
            next_run_at: Some(now + 60),
            last_run_at: None,
        };
        store.upsert_cron(&cron).unwrap();

        let loaded = store.get_cron("cron-1").unwrap().unwrap();
        assert_eq!(loaded.name, "heartbeat");
        assert_eq!(loaded.status, CronStatus::Active);
        assert!(loaded.skip_overlap);
        assert_eq!(loaded.next_run_at, Some(now + 60));
    }

    #[test]
    fn cron_list_and_update_ordered_and_remove() {
        let store = make_store();
        let now = now_unix();
        let a = CronRecord {
            cron_id: "a".into(),
            name: "every-min".into(),
            schedule: "* * * * * *".into(),
            command: "echo a".into(),
            status: CronStatus::Active,
            skip_overlap: true,
            created_at: now,
            updated_at: now,
            next_run_at: Some(now + 60),
            last_run_at: None,
        };
        let b = CronRecord {
            cron_id: "b".into(),
            name: "every-sec".into(),
            schedule: "* * * * * *".into(),
            command: "echo b".into(),
            status: CronStatus::Paused,
            skip_overlap: true,
            created_at: now + 1,
            updated_at: now + 1,
            next_run_at: Some(now + 30),
            last_run_at: None,
        };
        store.upsert_cron(&a).unwrap();
        store.upsert_cron(&b).unwrap();

        let items = store.list_crons().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].cron_id, "b");

        let mut updated = a;
        updated.name = "updated".into();
        updated.status = CronStatus::Paused;
        store.upsert_cron(&updated).unwrap();
        let reloaded = store.get_cron("a").unwrap().unwrap();
        assert_eq!(reloaded.name, "updated");
        assert_eq!(reloaded.status, CronStatus::Paused);

        store.delete_cron("a").unwrap();
        assert!(store.get_cron("a").unwrap().is_none());
    }

    #[test]
    fn cron_runs_store_and_retrieve() {
        let store = make_store();
        let now = now_unix();
        let run = CronRunRecord {
            run_id: "run-1".into(),
            cron_id: "cron-1".into(),
            scheduled_at: now,
            started_at: Some(now),
            finished_at: Some(now + 1),
            status: CronRunStatus::Succeeded,
            exit_code: Some(0),
            output: Some("ok".into()),
            error: None,
        };
        store.upsert_cron_run(&run).unwrap();
        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "run-2".into(),
                cron_id: "cron-1".into(),
                scheduled_at: now + 10,
                started_at: Some(now + 10),
                finished_at: Some(now + 11),
                status: CronRunStatus::Failed,
                exit_code: Some(1),
                output: Some("oops".into()),
                error: Some("boom".into()),
            })
            .unwrap();

        let latest = store.get_cron_last_run("cron-1").unwrap().unwrap();
        assert_eq!(latest.run_id, "run-2");

        let items = store.list_cron_runs("cron-1", 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].status, CronRunStatus::Failed);
        assert!(store.cron_has_running("cron-1").unwrap() == false);
        assert_eq!(items[0].run_id, "run-2");
    }

    #[test]
    fn cron_runs_pruning_keeps_recent_per_cron() {
        let store = make_store();
        let now = now_unix();
        for i in 0..6 {
            store
                .upsert_cron_run(&CronRunRecord {
                    run_id: format!("run-{i}"),
                    cron_id: "cron-1".into(),
                    scheduled_at: now + (i as u64),
                    started_at: Some(now + (i as u64)),
                    finished_at: Some(now + (i as u64) + 1),
                    status: CronRunStatus::Succeeded,
                    exit_code: Some(0),
                    output: None,
                    error: None,
                })
                .unwrap();
        }

        store.prune_cron_runs(0, 3).unwrap();
        let runs = store.list_cron_runs("cron-1", 10).unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].run_id, "run-5");
    }

    #[test]
    fn cron_has_running_detects_running_runs() {
        let store = make_store();
        let cron_id = "cron-running-check";
        let now = now_unix();
        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "running-1".into(),
                cron_id: cron_id.into(),
                scheduled_at: now,
                started_at: Some(now),
                finished_at: None,
                status: CronRunStatus::Running,
                exit_code: None,
                output: None,
                error: None,
            })
            .unwrap();
        assert!(store.cron_has_running(cron_id).unwrap());

        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "running-1".into(),
                cron_id: cron_id.into(),
                scheduled_at: now,
                started_at: Some(now),
                finished_at: Some(now + 1),
                status: CronRunStatus::Succeeded,
                exit_code: Some(0),
                output: None,
                error: None,
            })
            .unwrap();
        assert!(!store.cron_has_running(cron_id).unwrap());
    }

    #[test]
    fn cron_runs_prune_respects_retention_and_per_cron_cap() {
        let store = make_store();
        let now = now_unix();
        store
            .upsert_cron(&CronRecord {
                cron_id: "cron-prune".into(),
                name: "keep".into(),
                schedule: "* * * * * *".into(),
                command: "echo hi".into(),
                status: CronStatus::Active,
                skip_overlap: true,
                created_at: now,
                updated_at: now,
                next_run_at: None,
                last_run_at: None,
            })
            .unwrap();

        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "old".into(),
                cron_id: "cron-prune".into(),
                scheduled_at: now - (3 * 86_400),
                started_at: Some(now - (3 * 86_400)),
                finished_at: Some(now - (3 * 86_400)),
                status: CronRunStatus::Succeeded,
                exit_code: Some(0),
                output: None,
                error: None,
            })
            .unwrap();
        for i in 0..5 {
            store
                .upsert_cron_run(&CronRunRecord {
                    run_id: format!("new-{i}"),
                    cron_id: "cron-prune".into(),
                    scheduled_at: now + i as u64,
                    started_at: Some(now + i as u64),
                    finished_at: Some(now + i as u64 + 1),
                    status: CronRunStatus::Succeeded,
                    exit_code: Some(0),
                    output: None,
                    error: None,
                })
                .unwrap();
        }

        store.prune_cron_runs(1, 3).unwrap();

        let runs = store.list_cron_runs("cron-prune", 10).unwrap();
        assert_eq!(runs.len(), 3);
        assert!(!runs.iter().any(|run| run.run_id == "old"));
    }
}

mod chat;
mod cron;
mod jobs;
mod notifications;
mod pairings;
mod raw_events;
mod terminal;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{types::Type, Connection};
use uuid::Uuid;

use super::types::{
    ChatRawEventRecord, ChatRecord, CronRecord, CronRunRecord, JobRecord, NotificationEvent,
    NotificationSubscription, PairingRecord, TerminalRecord,
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

            CREATE INDEX IF NOT EXISTS idx_chats_created_at
                ON chats (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_terminals_started_at
                ON terminals (started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_jobs_created_at
                ON jobs (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_pairings_created_at
                ON pairings (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_pairings_expires_at
                ON pairings (expires_at);
            CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_target
                ON notification_subscriptions (target);
            CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_created_at
                ON notification_subscriptions (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_updated_at
                ON notification_subscriptions (updated_at);
            CREATE INDEX IF NOT EXISTS idx_notification_events_created_at
                ON notification_events (created_at);
            CREATE INDEX IF NOT EXISTS idx_chat_runs_started_at
                ON chat_runs (started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_raw_events_thread_created
                ON chat_raw_events (thread_id, created_at ASC);
            CREATE INDEX IF NOT EXISTS idx_chat_raw_events_run_id
                ON chat_raw_events (run_id);
            CREATE INDEX IF NOT EXISTS idx_cron_jobs_created_at
                ON cron_jobs (created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_cron_runs_cron_id
                ON cron_runs (cron_id, scheduled_at DESC);
            CREATE INDEX IF NOT EXISTS idx_cron_runs_scheduled_at
                ON cron_runs (scheduled_at DESC);
            CREATE INDEX IF NOT EXISTS idx_cron_runs_finished_at
                ON cron_runs (finished_at);
            CREATE INDEX IF NOT EXISTS idx_cron_runs_cron_status
                ON cron_runs (cron_id, status);
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
        chat::upsert_chat(self, chat)
    }

    fn get_chat(&self, chat_id: &str) -> Result<Option<ChatRecord>, String> {
        chat::get_chat(self, chat_id)
    }

    fn list_chats(&self) -> Result<Vec<ChatRecord>, String> {
        chat::list_chats(self)
    }

    fn delete_chat(&self, chat_id: &str) -> Result<(), String> {
        chat::delete_chat(self, chat_id)
    }

    fn update_event_pointer(&self, chat_id: &str, pointer: u64) -> Result<(), String> {
        chat::update_event_pointer(self, chat_id, pointer)
    }

    fn update_chat_settings(
        &self,
        chat_id: &str,
        settings: Option<&serde_json::Value>,
    ) -> Result<(), String> {
        chat::update_chat_settings(self, chat_id, settings)
    }

    fn upsert_chat_thread_state(
        &self,
        thread_id: &str,
        state: &serde_json::Value,
    ) -> Result<(), String> {
        chat::upsert_chat_thread_state(self, thread_id, state)
    }

    fn get_chat_thread_state(&self, thread_id: &str) -> Result<Option<serde_json::Value>, String> {
        chat::get_chat_thread_state(self, thread_id)
    }

    fn delete_chat_thread_state(&self, thread_id: &str) -> Result<(), String> {
        chat::delete_chat_thread_state(self, thread_id)
    }

    fn upsert_terminal(&self, rec: &TerminalRecord) -> Result<(), String> {
        terminal::upsert_terminal(self, rec)
    }

    fn get_terminal(&self, session_id: Uuid) -> Result<Option<TerminalRecord>, String> {
        terminal::get_terminal(self, session_id)
    }

    fn list_terminals(&self) -> Result<Vec<TerminalRecord>, String> {
        terminal::list_terminals(self)
    }

    fn delete_terminal(&self, session_id: Uuid) -> Result<(), String> {
        terminal::delete_terminal(self, session_id)
    }

    fn mark_all_inactive(&self) -> Result<(), String> {
        terminal::mark_all_inactive(self)
    }

    fn upsert_job(&self, job: &JobRecord) -> Result<(), String> {
        jobs::upsert_job(self, job)
    }

    fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, String> {
        jobs::get_job(self, job_id)
    }

    fn list_jobs(&self) -> Result<Vec<JobRecord>, String> {
        jobs::list_jobs(self)
    }

    fn prune_jobs(&self, retention_days: u64, max_jobs: usize) -> Result<(), String> {
        jobs::prune_jobs(self, retention_days, max_jobs)
    }

    fn upsert_pairing(&self, pairing: &PairingRecord) -> Result<(), String> {
        pairings::upsert_pairing(self, pairing)
    }

    fn get_pairing(&self, pairing_id: &str) -> Result<Option<PairingRecord>, String> {
        pairings::get_pairing(self, pairing_id)
    }

    fn list_pairings(&self) -> Result<Vec<PairingRecord>, String> {
        pairings::list_pairings(self)
    }

    fn prune_pairings(&self, retention_secs: u64) -> Result<(), String> {
        pairings::prune_pairings(self, retention_secs)
    }

    fn upsert_notification_subscription(
        &self,
        subscription: &NotificationSubscription,
    ) -> Result<(), String> {
        notifications::upsert_notification_subscription(self, subscription)
    }

    fn list_notification_subscriptions(&self) -> Result<Vec<NotificationSubscription>, String> {
        notifications::list_notification_subscriptions(self)
    }

    fn has_notification_target(&self, target: &str) -> Result<bool, String> {
        notifications::has_notification_target(self, target)
    }

    fn insert_notification_event(&self, event: &NotificationEvent) -> Result<(), String> {
        notifications::insert_notification_event(self, event)
    }

    fn prune_notifications(&self, retention_days: u64) -> Result<(), String> {
        notifications::prune_notifications(self, retention_days)
    }

    fn insert_chat_raw_event(
        &self,
        run_id: &str,
        thread_id: &str,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<(), String> {
        raw_events::insert_chat_raw_event(self, run_id, thread_id, method, params)
    }

    fn list_chat_raw_events(
        &self,
        thread_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatRawEventRecord>, String> {
        raw_events::list_chat_raw_events(self, thread_id, limit)
    }

    fn prune_chat_raw_events(&self, max_runs: usize) -> Result<(), String> {
        raw_events::prune_chat_raw_events(self, max_runs)
    }

    fn upsert_cron(&self, cron: &CronRecord) -> Result<(), String> {
        cron::upsert_cron(self, cron)
    }

    fn get_cron(&self, cron_id: &str) -> Result<Option<CronRecord>, String> {
        cron::get_cron(self, cron_id)
    }

    fn list_crons(&self) -> Result<Vec<CronRecord>, String> {
        cron::list_crons(self)
    }

    fn delete_cron(&self, cron_id: &str) -> Result<(), String> {
        cron::delete_cron(self, cron_id)
    }

    fn upsert_cron_run(&self, run: &CronRunRecord) -> Result<(), String> {
        cron::upsert_cron_run(self, run)
    }

    fn get_cron_run(&self, run_id: &str) -> Result<Option<CronRunRecord>, String> {
        cron::get_cron_run(self, run_id)
    }

    fn list_cron_runs(&self, cron_id: &str, limit: usize) -> Result<Vec<CronRunRecord>, String> {
        cron::list_cron_runs(self, cron_id, limit)
    }

    fn list_latest_cron_runs(&self, limit: usize) -> Result<Vec<CronRunRecord>, String> {
        cron::list_latest_cron_runs(self, limit)
    }

    fn get_cron_last_run(&self, cron_id: &str) -> Result<Option<CronRunRecord>, String> {
        cron::get_cron_last_run(self, cron_id)
    }

    fn cron_has_running(&self, cron_id: &str) -> Result<bool, String> {
        cron::cron_has_running(self, cron_id)
    }

    fn prune_cron_runs(&self, retention_days: u64, max_runs: usize) -> Result<(), String> {
        cron::prune_cron_runs(self, retention_days, max_runs)
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
mod tests;

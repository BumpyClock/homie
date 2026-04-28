use rusqlite::params;

use crate::storage::types::{NotificationEvent, NotificationSubscription};

use super::{now_unix, SqliteStore};

pub(super) fn upsert_notification_subscription(
    store: &SqliteStore,
    subscription: &NotificationSubscription,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

pub(super) fn list_notification_subscriptions(
    store: &SqliteStore,
) -> Result<Vec<NotificationSubscription>, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

pub(super) fn has_notification_target(store: &SqliteStore, target: &str) -> Result<bool, String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT 1 FROM notification_subscriptions WHERE target = ?1 LIMIT 1")
        .map_err(|e| format!("has_notification_target prepare: {e}"))?;
    let mut rows = stmt
        .query_map(params![target], |row| row.get::<_, i32>(0))
        .map_err(|e| format!("has_notification_target query: {e}"))?;
    Ok(rows.next().is_some())
}

pub(super) fn insert_notification_event(
    store: &SqliteStore,
    event: &NotificationEvent,
) -> Result<(), String> {
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

pub(super) fn prune_notifications(store: &SqliteStore, retention_days: u64) -> Result<(), String> {
    let cutoff = now_unix().saturating_sub(retention_days.saturating_mul(86_400));
    let conn = store.conn.lock().map_err(|e| format!("lock: {e}"))?;
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

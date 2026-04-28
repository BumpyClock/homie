use super::*;

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

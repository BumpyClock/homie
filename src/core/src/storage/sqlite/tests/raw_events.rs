use super::*;

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

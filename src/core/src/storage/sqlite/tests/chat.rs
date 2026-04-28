use super::*;

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
fn update_event_pointer_errors_for_missing_chat() {
    let store = make_store();

    let err = store
        .update_event_pointer("missing", 1)
        .expect_err("missing chat should fail");

    assert!(err.contains("missing chat"));
}

#[test]
fn update_event_pointer_does_not_move_backward() {
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: "c1".into(),
            thread_id: "t1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 10,
            settings: None,
        })
        .unwrap();

    store.update_event_pointer("c1", 4).unwrap();

    let loaded = store.get_chat("c1").unwrap().unwrap();
    assert_eq!(loaded.event_pointer, 10);
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

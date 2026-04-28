use super::*;

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
fn invalid_persisted_terminal_uuid_returns_error() {
    let store = make_store();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO terminals (session_id, shell, cols, rows, started_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["not-a-uuid", "/bin/sh", 80_i64, 24_i64, "100s", "active"],
        )
        .unwrap();
    }

    let err = store.list_terminals().unwrap_err();
    assert!(err.contains("list_terminals collect"));
    assert!(err.contains("invalid character") || err.contains("invalid length"));
}

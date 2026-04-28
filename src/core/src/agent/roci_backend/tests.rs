use std::sync::Arc;

use roci::agent::CollaborationMode;
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::router::ReapEvent;
use crate::storage::{SqliteStore, Store};
use crate::{ExecPolicy, HomieConfig};

use super::persistence::{decode_persisted_runtime_thread, persist_runtime_thread};
use super::RociBackend;

fn backend() -> (RociBackend, Arc<dyn Store>, broadcast::Receiver<ReapEvent>) {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_memory().expect("store"));
    let (event_tx, event_rx) = broadcast::channel(256);
    let backend = RociBackend::new(
        store.clone(),
        event_tx,
        Arc::new(ExecPolicy::empty()),
        Arc::new(HomieConfig::default()),
    );
    (backend, store, event_rx)
}

#[tokio::test]
async fn ensure_thread_uses_runtime_snapshot_as_thread_view() {
    let (backend, _store, _event_rx) = backend();
    let thread_id = Uuid::new_v4().to_string();
    let model = RociBackend::parse_model(None).expect("model");

    backend
        .ensure_thread(
            &thread_id,
            &thread_id,
            model,
            roci::config::RociConfig::from_env(),
            None,
            None,
        )
        .await
        .expect("thread");

    let thread = backend.thread_read(&thread_id).await.expect("thread view");
    assert_eq!(thread["id"].as_str(), Some(thread_id.as_str()));
    assert_eq!(thread["turns"].as_array().map(Vec::len), Some(0));
}

#[test]
fn parse_collaboration_mode_is_typed() {
    assert_eq!(
        RociBackend::parse_collaboration_mode(Some(&json!("plan"))),
        Some(CollaborationMode::Plan)
    );
    assert_eq!(
        RociBackend::parse_collaboration_mode(Some(&json!({ "mode": "code" }))),
        Some(CollaborationMode::Code)
    );
    assert_eq!(
        RociBackend::parse_collaboration_mode(Some(&json!("other"))),
        None
    );
}

#[tokio::test]
async fn persisted_runtime_thread_roundtrips_snapshot_and_provider_ledger() {
    let (backend, store, _event_rx) = backend();
    let thread_id = Uuid::new_v4().to_string();
    let model = RociBackend::parse_model(None).expect("model");

    backend
        .ensure_thread(
            &thread_id,
            &thread_id,
            model,
            roci::config::RociConfig::from_env(),
            None,
            None,
        )
        .await
        .expect("thread");

    let raw = store
        .get_chat_thread_state(&thread_id)
        .expect("state query");
    assert!(
        raw.is_none(),
        "empty runtime is not persisted until events occur"
    );

    let runtime_thread = {
        let model = RociBackend::parse_model(None).expect("model");
        backend
            .ensure_thread(
                &thread_id,
                &thread_id,
                model,
                roci::config::RociConfig::from_env(),
                None,
                None,
            )
            .await
            .expect("thread");
        backend.thread_read(&thread_id).await.expect("view")
    };
    assert_eq!(runtime_thread["id"].as_str(), Some(thread_id.as_str()));

    let thread = roci::agent::ThreadSnapshot {
        thread_id: roci::agent::ThreadId::from(Uuid::parse_str(&thread_id).unwrap()),
        revision: 0,
        last_seq: 0,
        active_turn_id: None,
        turns: Vec::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        approvals: Vec::new(),
        reasoning: Vec::new(),
        plans: Vec::new(),
        diffs: Vec::new(),
    };
    persist_runtime_thread(
        &store,
        &thread_id,
        thread.clone(),
        vec![roci::types::ModelMessage::user("hi")],
    );
    let decoded = decode_persisted_runtime_thread(
        store
            .get_chat_thread_state(&thread_id)
            .expect("state")
            .expect("persisted"),
    )
    .expect("decode")
    .expect("runtime thread");
    assert_eq!(decoded.thread.thread_id, thread.thread_id);
    assert_eq!(decoded.model_messages.len(), 1);
}

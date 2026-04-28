use super::*;
use crate::agent::service::events::{forward_mapped_event, EventForwardOutcome};

#[test]
fn codex_method_maps_agent_message_delta_to_chat_delta() {
    assert_eq!(
        codex_method_to_topics("item/agentMessage/delta"),
        Some(("chat.message.delta", "agent.chat.delta"))
    );
}

#[test]
fn codex_method_maps_turn_events() {
    assert_eq!(
        codex_method_to_topics("turn/started"),
        Some(("chat.turn.started", "agent.chat.turn.started"))
    );
    assert_eq!(
        codex_method_to_topics("turn/completed"),
        Some(("chat.turn.completed", "agent.chat.turn.completed"))
    );
}

#[test]
fn codex_method_maps_item_events() {
    assert_eq!(
        codex_method_to_topics("item/started"),
        Some(("chat.item.started", "agent.chat.item.started"))
    );
    assert_eq!(
        codex_method_to_topics("item/completed"),
        Some(("chat.item.completed", "agent.chat.item.completed"))
    );
}

#[test]
fn codex_method_maps_approval_requests() {
    assert_eq!(
        codex_method_to_topics("item/commandExecution/requestApproval"),
        Some(("chat.approval.required", "agent.chat.approval.required"))
    );
    assert_eq!(
        codex_method_to_topics("item/fileChange/requestApproval"),
        Some(("chat.approval.required", "agent.chat.approval.required"))
    );
}

#[test]
fn codex_method_maps_output_deltas() {
    assert_eq!(
        codex_method_to_topics("item/commandExecution/outputDelta"),
        Some(("chat.command.output", "agent.chat.command.output"))
    );
    assert_eq!(
        codex_method_to_topics("item/fileChange/outputDelta"),
        Some(("chat.file.output", "agent.chat.file.output"))
    );
}

#[test]
fn codex_method_maps_token_usage_updates() {
    assert_eq!(
        codex_method_to_topics("thread/tokenUsage/updated"),
        Some(("chat.token.usage.updated", "agent.chat.token.usage.updated"))
    );
}

#[test]
fn codex_method_maps_reasoning_and_plan() {
    assert_eq!(
        codex_method_to_topics("item/reasoning/summaryTextDelta"),
        Some(("chat.reasoning.delta", "agent.chat.reasoning.delta"))
    );
    assert_eq!(
        codex_method_to_topics("turn/diff/updated"),
        Some(("chat.diff.updated", "agent.chat.diff.updated"))
    );
    assert_eq!(
        codex_method_to_topics("turn/plan/updated"),
        Some(("chat.plan.updated", "agent.chat.plan.updated"))
    );
}

#[test]
fn unknown_codex_method_returns_none() {
    assert_eq!(codex_method_to_topics("unknown/method"), None);
}

#[test]
fn forward_mapped_event_advances_pointer_after_outbound_enqueues() {
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: "chat-1".into(),
            thread_id: "chat-1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 0,
            settings: None,
        })
        .unwrap();
    let (outbound_tx, _outbound_rx) = mpsc::channel(2);

    let outcome = forward_mapped_event(
        &outbound_tx,
        &store,
        "chat.turn.started",
        "agent.chat.turn.started",
        json!({"threadId": "chat-1", "turnId": "turn-1"}),
    );

    assert_eq!(outcome, EventForwardOutcome::Sent);
    let chat = store.get_chat("chat-1").unwrap().unwrap();
    assert_eq!(chat.event_pointer, 1);
}

#[test]
fn forward_mapped_event_does_not_advance_pointer_when_outbound_full() {
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: "chat-1".into(),
            thread_id: "chat-1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 0,
            settings: None,
        })
        .unwrap();
    let (outbound_tx, _outbound_rx) = mpsc::channel(1);
    outbound_tx
        .try_send(OutboundMessage::event("preexisting", None))
        .unwrap();

    let outcome = forward_mapped_event(
        &outbound_tx,
        &store,
        "chat.turn.started",
        "agent.chat.turn.started",
        json!({"threadId": "chat-1", "turnId": "turn-1"}),
    );

    assert_eq!(outcome, EventForwardOutcome::Dropped);
    let chat = store.get_chat("chat-1").unwrap().unwrap();
    assert_eq!(chat.event_pointer, 0);
}

#[test]
fn forward_mapped_event_reports_closed_outbound() {
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: "chat-1".into(),
            thread_id: "chat-1".into(),
            created_at: "100s".into(),
            status: SessionStatus::Active,
            event_pointer: 0,
            settings: None,
        })
        .unwrap();
    let (outbound_tx, outbound_rx) = mpsc::channel(2);
    drop(outbound_rx);

    let outcome = forward_mapped_event(
        &outbound_tx,
        &store,
        "chat.turn.started",
        "agent.chat.turn.started",
        json!({"threadId": "chat-1", "turnId": "turn-1"}),
    );

    assert_eq!(outcome, EventForwardOutcome::OutboundClosed);
    let chat = store.get_chat("chat-1").unwrap().unwrap();
    assert_eq!(chat.event_pointer, 0);
}

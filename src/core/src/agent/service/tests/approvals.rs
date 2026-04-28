use super::*;

#[tokio::test]
async fn approval_respond_rejects_unknown_decision() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = AgentService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );

    let resp = svc
        .handle_request(
            Uuid::new_v4(),
            "agent.chat.approval.respond",
            Some(json!({
                "codex_request_id": "approval-1",
                "decision": "bogus"
            })),
        )
        .await;

    let err = resp.error.expect("unknown decision should fail");
    assert_eq!(err.code, error_codes::INVALID_PARAMS);
}

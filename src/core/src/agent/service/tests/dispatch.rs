use super::*;

#[tokio::test]
async fn agent_service_returns_error_for_unknown_method() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = AgentService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let id = Uuid::new_v4();
    let resp = svc.handle_request(id, "agent.unknown.method", None).await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
}

#[test]
fn agent_service_namespace_is_agent() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let svc = AgentService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    assert_eq!(svc.namespace(), "agent");
}

#[test]
fn agent_service_reap_returns_empty_initially() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = AgentService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    assert!(svc.reap().is_empty());
}

#[tokio::test]
async fn chat_list_returns_empty_initially() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = AgentService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let id = Uuid::new_v4();
    let resp = svc.handle_request(id, "agent.chat.list", None).await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    let chats = result["chats"].as_array().unwrap();
    assert!(chats.is_empty());
}

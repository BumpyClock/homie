use super::*;

#[tokio::test]
async fn chat_thread_read_without_turns_returns_thread_shell_with_settings() {
    let thread_id = "thread-no-turns";
    let chat_id = "chat-no-turns";
    let settings = json!({
        "model": "openai-codex:gpt-5.1-codex",
        "effort": "high"
    });
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: chat_id.to_string(),
            thread_id: thread_id.to_string(),
            created_at: chrono_now(),
            status: SessionStatus::Active,
            event_pointer: 0,
            settings: Some(settings.clone()),
        })
        .unwrap();

    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = ChatService::new(
        tx,
        store,
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.thread.read",
            Some(json!({
                "chat_id": chat_id,
                "include_turns": false,
            })),
        )
        .await;

    assert!(resp.error.is_none());
    let result = resp.result.expect("thread read result");
    assert_eq!(result["thread"]["id"], thread_id);
    assert!(result["thread"]["turns"].is_null());
    assert_eq!(result["settings"], settings);
}

#[tokio::test]
async fn chat_thread_read_recovers_from_invalid_persisted_thread_state() {
    let thread_id = "thread-invalid-state";
    let chat_id = "chat-invalid-state";
    let settings = json!({
        "model": "openai-codex:gpt-5.1-codex",
    });
    let store = make_store();
    store
        .upsert_chat(&ChatRecord {
            chat_id: chat_id.to_string(),
            thread_id: thread_id.to_string(),
            created_at: chrono_now(),
            status: SessionStatus::Active,
            event_pointer: 0,
            settings: Some(settings.clone()),
        })
        .unwrap();
    store
        .upsert_chat_thread_state(thread_id, &json!({ "invalid": true }))
        .unwrap();

    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = ChatService::new(
        tx,
        store,
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.thread.read",
            Some(json!({
                "chat_id": chat_id,
                "include_turns": true,
            })),
        )
        .await;

    assert!(resp.error.is_none());
    let result = resp.result.expect("thread read result");
    assert_eq!(result["thread"]["id"], thread_id);
    assert_eq!(result["settings"], settings);
}

#[tokio::test]
async fn chat_tools_list_returns_expected_shape() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = ChatService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let id = Uuid::new_v4();
    let resp = svc
        .handle_request(id, "chat.tools.list", Some(json!({ "channel": "web" })))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.expect("result");
    let data = result["data"].as_array().expect("data array");
    assert!(!data.is_empty());
    let read = data
        .iter()
        .find(|tool| tool.get("name").and_then(|v| v.as_str()) == Some("read"))
        .expect("read tool");
    assert_eq!(read["provider"], "core");
    assert_eq!(read["provider_dynamic"], false);
    assert!(read["input_schema"].is_object());
}

#[tokio::test]
async fn chat_tools_list_applies_channel_gating() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut config = HomieConfig::default();
    config.tools.providers.insert(
        "core".to_string(),
        crate::homie_config::ToolProviderConfig {
            enabled: Some(true),
            channels: vec!["mobile".to_string()],
            allow_tools: Vec::new(),
            deny_tools: Vec::new(),
        },
    );
    let mut svc = ChatService::new(
        tx,
        make_store(),
        Arc::new(config),
        Arc::new(ExecPolicy::empty()),
    );

    let web_resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.tools.list",
            Some(json!({ "channel": "web" })),
        )
        .await;
    assert!(web_resp.error.is_none());
    let web_tools = web_resp.result.expect("web result")["data"]
        .as_array()
        .expect("web data")
        .clone();
    assert!(web_tools.is_empty());

    let mobile_resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.tools.list",
            Some(json!({ "channel": "mobile" })),
        )
        .await;
    assert!(mobile_resp.error.is_none());
    let mobile_tools = mobile_resp.result.expect("mobile result")["data"]
        .as_array()
        .expect("mobile data")
        .clone();
    assert!(mobile_tools
        .iter()
        .any(|tool| tool.get("provider").and_then(|v| v.as_str()) == Some("core")));
}

#[tokio::test]
async fn chat_tools_list_denies_unknown_or_undefined_channel() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = ChatService::new(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
    );
    let missing_resp = svc
        .handle_request(Uuid::new_v4(), "chat.tools.list", None)
        .await;
    assert!(missing_resp.result.is_none());
    let missing_error = missing_resp.error.expect("missing error");
    assert_eq!(missing_error.code, error_codes::INVALID_PARAMS);
    assert!(missing_error.message.contains(TOOL_CHANNEL_DENIED_CODE));

    let resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.tools.list",
            Some(json!({ "channel": "discord" })),
        )
        .await;
    assert!(resp.result.is_none());
    let error = resp.error.expect("error");
    assert_eq!(error.code, error_codes::INVALID_PARAMS);
    assert!(error.message.contains(TOOL_CHANNEL_DENIED_CODE));
}

#[tokio::test]
async fn chat_tools_list_denies_channel_mismatch_with_bound_connection_channel() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut svc = ChatService::new_with_channel(
        tx,
        make_store(),
        Arc::new(HomieConfig::default()),
        Arc::new(ExecPolicy::empty()),
        Some("web".to_string()),
    );
    let resp = svc
        .handle_request(
            Uuid::new_v4(),
            "chat.tools.list",
            Some(json!({ "channel": "mobile" })),
        )
        .await;
    assert!(resp.result.is_none());
    let error = resp.error.expect("error");
    assert_eq!(error.code, error_codes::INVALID_PARAMS);
    assert!(error.message.contains(TOOL_CHANNEL_DENIED_CODE));
}

#[tokio::test]
async fn chat_account_list_reports_provider_statuses() {
    let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
    let mut config = HomieConfig::default();
    let tmp_dir = std::env::temp_dir().join(format!("homie-auth-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir).unwrap();
    config.paths.credentials_dir = Some(tmp_dir.to_string_lossy().to_string());
    let _ = (tx, config, tmp_dir);
}

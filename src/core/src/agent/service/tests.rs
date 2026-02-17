mod tests {
    use crate::agent::process::CodexRequestId;
    use crate::agent::service::events::codex_method_to_topics;
    use crate::agent::service::models::{chrono_now, roci_model_catalog};
    use crate::agent::service::params::{
        normalize_model_selector, parse_approval_params, parse_cancel_params, parse_message_params,
        parse_tool_channel,
    };
    use crate::agent::service::dispatch::{AgentService, ChatService};
    use crate::execpolicy::ExecPolicy;
    use crate::homie_config::{HomieConfig, ProvidersConfig};
    use crate::outbound::OutboundMessage;
    use crate::ServiceHandler;
    use crate::storage::{ChatRecord, SessionStatus, SqliteStore, Store};
    use homie_protocol::error_codes;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn make_store() -> Arc<dyn Store> {
        Arc::new(SqliteStore::open_memory().unwrap())
    }

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
    fn parse_message_params_extracts_chat_id_and_message() {
        let params = Some(json!({
            "chat_id": "abc-123",
            "message": "hello world"
        }));
        let (chat_id, message, model, effort, approval_policy, collaboration_mode, inject) =
            parse_message_params(&params).unwrap();
        assert_eq!(chat_id, "abc-123");
        assert_eq!(message, "hello world");
        assert!(model.is_none());
        assert!(effort.is_none());
        assert!(approval_policy.is_none());
        assert!(collaboration_mode.is_none());
        assert!(!inject);
    }

    #[test]
    fn parse_message_params_returns_none_when_missing_fields() {
        assert!(parse_message_params(&None).is_none());
        assert!(parse_message_params(&Some(json!({"chat_id": "x"}))).is_none());
        assert!(parse_message_params(&Some(json!({"message": "x"}))).is_none());
    }

    #[test]
    fn parse_message_params_reads_inject_flag() {
        let params = Some(json!({
            "chat_id": "abc-123",
            "message": "hello world",
            "inject": true
        }));
        let (_, _, _, _, _, _, inject) = parse_message_params(&params).unwrap();
        assert!(inject);
    }

    #[test]
    fn normalize_model_selector_upgrades_legacy_copilot_ids() {
        let providers = ProvidersConfig {
            github_copilot: crate::homie_config::GithubCopilotProviderConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let normalized = normalize_model_selector("openai-compatible:gpt-5.2-codex", &providers);
        assert_eq!(normalized, "github-copilot:gpt-5.2-codex");
    }

    #[test]
    fn normalize_model_selector_keeps_unknown_compat_ids() {
        let providers = ProvidersConfig {
            github_copilot: crate::homie_config::GithubCopilotProviderConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let normalized =
            normalize_model_selector("openai-compatible:custom-proxy-model", &providers);
        assert_eq!(normalized, "openai-compatible:custom-proxy-model");
    }

    #[test]
    fn normalize_model_selector_upgrades_known_cross_provider_copilot_ids() {
        let providers = ProvidersConfig {
            github_copilot: crate::homie_config::GithubCopilotProviderConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let normalized = normalize_model_selector("openai-compatible:claude-opus-4.6", &providers);
        assert_eq!(normalized, "github-copilot:claude-opus-4.6");
    }

    #[test]
    fn roci_model_catalog_uses_github_copilot_prefix() {
        let providers = ProvidersConfig {
            github_copilot: crate::homie_config::GithubCopilotProviderConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let models = roci_model_catalog(&providers);
        assert!(
            models.iter().any(|m| {
                m.get("model")
                    .and_then(|v| v.as_str())
                    .map(|v| v.starts_with("github-copilot:"))
                    .unwrap_or(false)
            }),
            "expected at least one github-copilot model in catalog"
        );
        assert!(
            !models.iter().any(|m| {
                m.get("model")
                    .and_then(|v| v.as_str())
                    .map(|v| v.starts_with("openai-compatible:"))
                    .unwrap_or(false)
            }),
            "did not expect openai-compatible fallback entries for copilot catalog"
        );
        assert!(
            models.iter().any(|m| {
                m.get("model")
                    .and_then(|v| v.as_str())
                    .map(|v| v == "github-copilot:claude-opus-4.6")
                    .unwrap_or(false)
            }),
            "expected curated github-copilot fallback entries from docs"
        );
    }

    #[test]
    fn parse_tool_channel_defaults_to_web() {
        assert_eq!(parse_tool_channel(&None), "web");
        assert_eq!(parse_tool_channel(&Some(json!({}))), "web");
        assert_eq!(parse_tool_channel(&Some(json!({"channel": "   "}))), "web");
    }

    #[test]
    fn parse_tool_channel_normalizes_value() {
        assert_eq!(
            parse_tool_channel(&Some(json!({"channel": "  DisCord "}))),
            "discord"
        );
    }

    #[test]
    fn parse_cancel_params_extracts_ids() {
        let params = Some(json!({
            "chat_id": "c1",
            "turn_id": "t1"
        }));
        let (chat_id, turn_id) = parse_cancel_params(&params).unwrap();
        assert_eq!(chat_id, "c1");
        assert_eq!(turn_id, "t1");
    }

    #[test]
    fn parse_approval_params_extracts_id_and_decision() {
        let params = Some(json!({
            "codex_request_id": 42,
            "decision": "accept"
        }));
        let (id, decision) = parse_approval_params(&params).unwrap();
        assert!(matches!(id, CodexRequestId::Number(42)));
        assert_eq!(decision, "accept");
    }

    #[test]
    fn parse_approval_params_returns_none_for_invalid_input() {
        assert!(parse_approval_params(&None).is_none());
        assert!(
            parse_approval_params(&Some(json!({"codex_request_id": { "bad": true } }))).is_none()
        );
    }

    #[test]
    fn parse_approval_params_accepts_string_id() {
        let params = Some(json!({
            "codex_request_id": "abc-123",
            "decision": "decline"
        }));
        let (id, decision) = parse_approval_params(&params).unwrap();
        assert!(matches!(id, CodexRequestId::Text(ref s) if s == "abc-123"));
        assert_eq!(decision, "decline");
    }

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
        let resp = svc.handle_request(id, "chat.tools.list", None).await;
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
                channels: vec!["discord".to_string()],
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
            .handle_request(Uuid::new_v4(), "chat.tools.list", None)
            .await;
        assert!(web_resp.error.is_none());
        let web_tools = web_resp.result.expect("web result")["data"]
            .as_array()
            .expect("web data")
            .clone();
        assert!(web_tools.is_empty());

        let discord_resp = svc
            .handle_request(
                Uuid::new_v4(),
                "chat.tools.list",
                Some(json!({ "channel": "discord" })),
            )
            .await;
        assert!(discord_resp.error.is_none());
        let discord_tools = discord_resp.result.expect("discord result")["data"]
            .as_array()
            .expect("discord data")
            .clone();
        assert!(discord_tools
            .iter()
            .any(|tool| tool.get("provider").and_then(|v| v.as_str()) == Some("core")));
    }

    #[tokio::test]
    async fn chat_account_list_reports_provider_statuses() {
        let (tx, _rx) = mpsc::channel::<OutboundMessage>(16);
        let mut config = HomieConfig::default();
        let tmp_dir = std::env::temp_dir().join(format!("homie-auth-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        config.paths.credentials_dir = Some(tmp_dir.to_string_lossy().to_string());
        // TODO: implement this test
        let _ = (tx, config, tmp_dir);
    }
}

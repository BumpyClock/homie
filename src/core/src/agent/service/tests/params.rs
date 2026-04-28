use super::*;

#[test]
fn parse_message_params_extracts_chat_id_and_message() {
    let params = Some(json!({
        "chat_id": "abc-123",
        "message": "hello world"
    }));
    let MessageParams {
        chat_id,
        message,
        model,
        effort,
        approval_policy,
        collaboration_mode,
        inject,
    } = parse_message_params(&params).unwrap();
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
    let MessageParams { inject, .. } = parse_message_params(&params).unwrap();
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
    let normalized = normalize_model_selector("openai-compatible:custom-proxy-model", &providers);
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
fn parse_tool_channel_requires_non_empty_value() {
    assert_eq!(parse_tool_channel(&None), None);
    assert_eq!(parse_tool_channel(&Some(json!({}))), None);
    assert_eq!(parse_tool_channel(&Some(json!({"channel": "   "}))), None);
}

#[test]
fn parse_tool_channel_normalizes_value() {
    assert_eq!(
        parse_tool_channel(&Some(json!({"channel": "  DisCord "}))),
        Some("discord".to_string())
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
    assert!(parse_approval_params(&Some(json!({"codex_request_id": { "bad": true } }))).is_none());
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

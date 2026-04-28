use roci::agent::CollaborationMode;
use roci::agent_loop::ApprovalPolicy;
use roci::models::LanguageModel;
use roci::types::{GenerationSettings, ReasoningEffort};
use serde_json::Value;

use super::RociBackend;

impl RociBackend {
    pub fn parse_model(input: Option<&String>) -> Result<LanguageModel, String> {
        let raw = input
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(default_roci_model);
        let normalized = if let Some(model_id) = raw.strip_prefix("openai-codex:") {
            format!("codex:{model_id}")
        } else if raw.contains(':') {
            raw
        } else {
            format!("openai:{raw}")
        };
        normalized
            .parse::<LanguageModel>()
            .map_err(|e| format!("invalid model: {e}"))
    }

    pub fn parse_settings(
        effort: Option<&String>,
        stream_idle_timeout_ms: Option<u64>,
    ) -> GenerationSettings {
        let mut settings = GenerationSettings::default();
        if let Some(effort) = effort {
            if let Ok(parsed) = effort.parse::<ReasoningEffort>() {
                settings.reasoning_effort = Some(parsed);
            }
        }
        settings.stream_idle_timeout_ms = stream_idle_timeout_ms;
        settings
    }

    pub fn parse_approval_policy(policy: Option<&String>) -> ApprovalPolicy {
        match policy.map(|p| p.trim().to_lowercase()) {
            Some(value) if value == "never" => ApprovalPolicy::Always,
            Some(value) if value == "always" => ApprovalPolicy::Always,
            Some(value) if value == "on-request" => ApprovalPolicy::Ask,
            Some(value) if value == "untrusted" => ApprovalPolicy::Ask,
            _ => ApprovalPolicy::Ask,
        }
    }

    pub fn parse_collaboration_mode(mode: Option<&Value>) -> Option<CollaborationMode> {
        let raw = mode?;
        if let Some(value) = raw.as_str() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return collaboration_mode_from_str(trimmed);
            }
        }
        let obj = raw.as_object()?;
        let value = obj
            .get("mode")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("id").and_then(|v| v.as_str()))?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            collaboration_mode_from_str(trimmed)
        }
    }
}

fn default_roci_model() -> String {
    std::env::var("HOMIE_ROCI_MODEL").unwrap_or_else(|_| super::DEFAULT_ROCI_MODEL.to_string())
}

fn collaboration_mode_from_str(value: &str) -> Option<CollaborationMode> {
    match value.trim().to_lowercase().as_str() {
        "plan" => Some(CollaborationMode::Plan),
        "code" => Some(CollaborationMode::Code),
        _ => None,
    }
}

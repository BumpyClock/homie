use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::service::core::CodexChatCore;
use crate::agent::tools::{list_tools, ToolContext, TOOL_CHANNEL_DENIED_CODE};

use super::files::list_homie_skills;
use super::models::{
    append_openai_compatible_models, discover_github_copilot_models,
    discover_openai_compatible_models, roci_model_catalog,
};
use super::params::parse_tool_channel;

impl CodexChatCore {
    pub(super) async fn chat_skills_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let skills = match list_homie_skills() {
                Ok(skills) => skills,
                Err(err) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("skills list failed: {err}"),
                    )
                }
            };
            return Response::success(req_id, json!({ "data": [{ "skills": skills }] }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("skills/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("skills/list failed: {e}"),
            ),
        }
    }

    pub(super) async fn chat_model_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let mut models = roci_model_catalog(&self.homie_config.providers);
            if self.homie_config.providers.github_copilot.enabled {
                match self.roci_token_store() {
                    Ok(store) => {
                        let auth = self.github_copilot_auth(store, "default");
                        match discover_github_copilot_models(&auth).await {
                            Ok(copilot_models) => {
                                super::models::replace_github_copilot_models(
                                    &mut models,
                                    copilot_models,
                                );
                            }
                            Err(err) => {
                                tracing::debug!("github-copilot model discovery skipped: {err}");
                            }
                        }
                    }
                    Err(err) => {
                        tracing::debug!("github-copilot model discovery init failed: {err}");
                    }
                }
            }
            match discover_openai_compatible_models(&self.homie_config.providers.openai_compatible)
                .await
            {
                Ok(compat_models) => {
                    append_openai_compatible_models(&mut models, compat_models);
                }
                Err(err) => {
                    tracing::debug!("openai-compatible model discovery skipped: {err}");
                }
            }
            return Response::success(req_id, json!({ "data": models }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("model/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("model/list failed: {e}"),
            ),
        }
    }

    pub(super) async fn chat_tools_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let requested_channel = parse_tool_channel(&params);
            if let (Some(bound_channel), Some(requested)) =
                (self.tool_channel.as_deref(), requested_channel.as_deref())
            {
                if !bound_channel.eq_ignore_ascii_case(requested) {
                    tracing::info!(
                        provider = "*",
                        tool = "*",
                        channel = requested,
                        bound_channel = bound_channel,
                        decision = "deny",
                        "tool channel policy"
                    );
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        TOOL_CHANNEL_DENIED_CODE,
                    );
                }
            }
            let effective_channel = self
                .tool_channel
                .as_deref()
                .or(requested_channel.as_deref());
            let ctx =
                ToolContext::new_with_channel(self.homie_config.clone(), effective_channel);
            let resolved_channel = match ctx.channel.clone() {
                Some(channel) => channel,
                None => {
                    tracing::info!(
                        provider = "*",
                        tool = "*",
                        channel = effective_channel.unwrap_or("undefined"),
                        decision = "deny",
                        "tool channel policy"
                    );
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        TOOL_CHANNEL_DENIED_CODE,
                    );
                }
            };
            let tools = match list_tools(ctx, &self.homie_config) {
                Ok(tools) => tools,
                Err(err) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("tools list failed: {err}"),
                    )
                }
            };
            tracing::debug!(
                provider = "*",
                tool = "*",
                channel = resolved_channel,
                decision = "allow",
                "tool channel policy"
            );
            let data: Vec<Value> = tools
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "provider": tool.provider_id,
                        "provider_dynamic": tool.provider_dynamic,
                        "input_schema": tool.input_schema,
                    })
                })
                .collect();
            return Response::success(req_id, json!({ "data": data }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("tools/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("tools/list failed: {e}"),
            ),
        }
    }

    pub(super) async fn chat_collaboration_mode_list(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            let modes = vec![
                json!({
                    "name": "Plan",
                    "mode": "plan",
                    "description": "Plan steps before executing.",
                }),
                json!({
                    "name": "Code",
                    "mode": "code",
                    "description": "Act immediately on user requests.",
                }),
            ];
            return Response::success(req_id, json!({ "data": modes }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        let params = params.or_else(|| Some(json!({})));
        match process.send_request("collaborationMode/list", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("collaborationMode/list failed: {e}"),
            ),
        }
    }

    pub(super) async fn chat_skills_config_write(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        if self.use_roci() {
            return Response::success(req_id, json!({ "ok": true }));
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("skills/config/write", params).await {
            Ok(result) => Response::success(req_id, result),
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("skills/config/write failed: {e}"),
            ),
        }
    }
}

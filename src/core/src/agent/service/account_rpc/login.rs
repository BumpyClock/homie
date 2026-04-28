use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use super::super::core::CodexChatCore;
use super::super::params::{
    device_code_poll_json, device_code_session_json, parse_account_provider_params,
    parse_device_code_session,
};

impl CodexChatCore {
    pub(crate) async fn chat_account_login_start(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (provider_id, profile, _param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login start failed: {e}"),
                )
            }
        };

        match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                match auth.start_device_code().await {
                    Ok(session) => Response::success(
                        req_id,
                        json!({ "session": device_code_session_json(&session) }),
                    ),
                    Err(e) => Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("device code start failed: {e}"),
                    ),
                }
            }
            "claude-code" => Response::error(
                req_id,
                error_codes::INVALID_PARAMS,
                "claude-code does not support device-code login",
            ),
            _ => Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider"),
        }
    }

    pub(crate) async fn chat_account_login_poll(
        &mut self,
        req_id: Uuid,
        params: Option<Value>,
    ) -> Response {
        let (provider_id, profile, param_map) = match parse_account_provider_params(&params) {
            Some(value) => value,
            None => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "missing provider")
            }
        };
        if !self.provider_enabled(&provider_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "provider disabled");
        }
        let session = match parse_device_code_session(&param_map, &provider_id) {
            Some(session) => session,
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing session"),
        };

        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account login poll failed: {e}"),
                )
            }
        };

        let poll = match provider_id.as_str() {
            "openai-codex" => {
                let auth = self.openai_codex_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "github-copilot" => {
                let auth = self.github_copilot_auth(store, &profile);
                auth.poll_device_code(&session).await
            }
            "claude-code" => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "claude-code does not support device-code login",
                )
            }
            _ => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unsupported provider")
            }
        };

        match poll {
            Ok(result) => {
                Response::success(req_id, device_code_poll_json(result, session.interval_secs))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("device code poll failed: {e}"),
            ),
        }
    }
}

use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use super::super::core::CodexChatCore;

impl CodexChatCore {
    pub(crate) async fn chat_account_read(&mut self, req_id: Uuid) -> Response {
        if self.use_roci() {
            let store = match self.roci_token_store() {
                Ok(store) => store,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read init failed: {e}"),
                    )
                }
            };
            self.import_enabled_provider_credentials(&self.homie_config.providers, &store);
            let providers = match self.account_provider_statuses(&store) {
                Ok(providers) => providers,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INTERNAL_ERROR,
                        format!("account read failed: {e}"),
                    )
                }
            };
            let any_logged_in = providers.iter().any(|entry| {
                entry
                    .get("logged_in")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            });
            let account = if any_logged_in {
                json!({ "providers": providers })
            } else {
                Value::Null
            };
            return Response::success(
                req_id,
                json!({
                    "account": account,
                    "providers": providers,
                    "requires_openai_auth": !any_logged_in,
                }),
            );
        }

        if let Err(e) = self.ensure_process().await {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let process = self.process.as_ref().unwrap();
        match process.send_request("account/read", Some(json!({}))).await {
            Ok(result) => {
                tracing::info!(result = %result, "codex account/read");
                Response::success(req_id, result)
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("account/read failed: {e}"),
            ),
        }
    }
}

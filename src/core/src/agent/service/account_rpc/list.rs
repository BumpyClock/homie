use homie_protocol::{error_codes, Response};
use serde_json::json;
use uuid::Uuid;

use super::super::core::CodexChatCore;

impl CodexChatCore {
    pub(crate) async fn chat_account_list(&mut self, req_id: Uuid) -> Response {
        let store = match self.roci_token_store() {
            Ok(store) => store,
            Err(e) => {
                return Response::error(
                    req_id,
                    error_codes::INTERNAL_ERROR,
                    format!("account list init failed: {e}"),
                )
            }
        };

        self.import_enabled_provider_credentials(&self.homie_config.providers, &store);
        match self.account_provider_statuses(&store) {
            Ok(providers) => Response::success(req_id, json!({ "providers": providers })),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }
}

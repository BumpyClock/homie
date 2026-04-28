use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use super::super::core::CodexChatCore;

impl CodexChatCore {
    pub(crate) fn chat_list(&self, req_id: Uuid) -> Response {
        match self.store.list_chats() {
            Ok(records) => {
                let chats: Vec<Value> = records
                    .into_iter()
                    .map(|r| {
                        json!({
                            "chat_id": r.chat_id,
                            "thread_id": r.thread_id,
                            "created_at": r.created_at,
                            "status": r.status,
                            "event_pointer": r.event_pointer,
                            "settings": r.settings,
                        })
                    })
                    .collect();
                Response::success(req_id, json!({ "chats": chats }))
            }
            Err(e) => Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("list failed: {e}"),
            ),
        }
    }
}

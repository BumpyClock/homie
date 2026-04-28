use homie_protocol::{error_codes, Response};
use serde_json::{json, Value};
use uuid::Uuid;

use super::super::core::CodexChatCore;
use super::super::files::{extract_attached_folder, search_files_in_folder};
use super::super::params::{
    merge_settings, normalize_settings_models, parse_files_search_params,
    parse_settings_update_params,
};

impl CodexChatCore {
    pub(crate) fn chat_settings_update(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, updates) = match parse_settings_update_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or settings",
                )
            }
        };
        let updates = normalize_settings_models(updates, &self.homie_config.providers);

        let existing = self
            .store
            .get_chat(&chat_id)
            .ok()
            .flatten()
            .and_then(|rec| rec.settings);
        let merged = merge_settings(existing, updates);
        if let Err(e) = self.store.update_chat_settings(&chat_id, Some(&merged)) {
            return Response::error(
                req_id,
                error_codes::INTERNAL_ERROR,
                format!("settings update failed: {e}"),
            );
        }
        Response::success(req_id, json!({ "ok": true, "settings": merged }))
    }

    pub(crate) fn chat_files_search(&self, req_id: Uuid, params: Option<Value>) -> Response {
        let (chat_id, query, limit, base_override) = match parse_files_search_params(&params) {
            Some(v) => v,
            None => {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "missing chat_id or query",
                )
            }
        };

        let settings = match self.store.get_chat(&chat_id) {
            Ok(Some(rec)) => rec.settings,
            _ => None,
        };
        let base = extract_attached_folder(settings.as_ref()).or_else(|| base_override.clone());
        let base = match base {
            Some(path) => path,
            None => {
                tracing::debug!(%chat_id, "chat files search skipped: no attached folder");
                return Response::success(req_id, json!({ "files": [] }));
            }
        };

        tracing::debug!(%chat_id, %base, %query, %limit, "chat files search");
        match search_files_in_folder(&base, &query, limit) {
            Ok(files) => {
                tracing::debug!(%chat_id, count = files.len(), "chat files search complete");
                Response::success(req_id, json!({ "files": files, "base_path": base }))
            }
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }
}

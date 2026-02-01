use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};

use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{PairingRecord, PairingStatus, Store};

#[derive(Debug, Deserialize)]
struct RequestParams {
    ttl_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApproveParams {
    pairing_id: String,
    approved_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RevokeParams {
    pairing_id: String,
}

/// Pairing service backed by the persistent store.
pub struct PairingService {
    store: Arc<dyn Store>,
    default_ttl_secs: u64,
    retention_secs: u64,
}

impl PairingService {
    pub fn new(store: Arc<dyn Store>, default_ttl_secs: u64, retention_secs: u64) -> Self {
        Self {
            store,
            default_ttl_secs,
            retention_secs,
        }
    }

    fn request(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let ttl = match params {
            Some(v) => match serde_json::from_value::<RequestParams>(v) {
                Ok(p) => p.ttl_secs.unwrap_or(self.default_ttl_secs),
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => self.default_ttl_secs,
        };

        let now = now_unix();
        let session = PairingRecord {
            pairing_id: Uuid::new_v4().to_string(),
            nonce: Uuid::new_v4().to_string(),
            status: PairingStatus::Pending,
            created_at: now,
            expires_at: now.saturating_add(ttl),
            approved_by: None,
        };

        if let Err(e) = self.store.upsert_pairing(&session) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "pairing": session }))
    }

    fn approve(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: ApproveParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let mut session = match self.store.get_pairing(&params.pairing_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown pairing")
            }
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        if now_unix() > session.expires_at {
            session.status = PairingStatus::Expired;
        } else {
            session.status = PairingStatus::Approved;
            session.approved_by = params.approved_by;
        }

        if let Err(e) = self.store.upsert_pairing(&session) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "pairing": session }))
    }

    fn revoke(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: RevokeParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let mut session = match self.store.get_pairing(&params.pairing_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown pairing")
            }
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        session.status = PairingStatus::Revoked;

        if let Err(e) = self.store.upsert_pairing(&session) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "pairing": session }))
    }

    fn list(&mut self, req_id: Uuid) -> Response {
        if let Err(e) = self.store.prune_pairings(self.retention_secs) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }
        match self.store.list_pairings() {
            Ok(pairings) => Response::success(req_id, json!({ "pairings": pairings })),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }
}

impl ServiceHandler for PairingService {
    fn namespace(&self) -> &str {
        "pairing"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let resp = match method {
            "pairing.request" => self.request(id, params),
            "pairing.approve" => self.approve(id, params),
            "pairing.list" => self.list(id),
            "pairing.revoke" => self.revoke(id, params),
            _ => Response::error(
                id,
                error_codes::METHOD_NOT_FOUND,
                format!("unknown method: {method}"),
            ),
        };
        Box::pin(async move { resp })
    }

    fn handle_binary(&mut self, _frame: &BinaryFrame) {}

    fn reap(&mut self) -> Vec<ReapEvent> {
        Vec::new()
    }

    fn shutdown(&mut self) {}
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

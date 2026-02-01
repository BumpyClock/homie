use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response, ServiceCapability};

use crate::router::{ReapEvent, ServiceHandler};

use super::registry::{NodeInfo, NodeRegistry};

#[derive(Debug, Deserialize)]
struct RegisterParams {
    node_id: String,
    name: Option<String>,
    version: Option<String>,
    services: Option<Vec<ServiceCapability>>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatParams {
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct UnregisterParams {
    node_id: String,
}

/// Presence service for node registration and heartbeats.
pub struct PresenceService {
    registry: Arc<Mutex<NodeRegistry>>,
    registered: Vec<String>,
}

impl PresenceService {
    pub fn new(registry: Arc<Mutex<NodeRegistry>>) -> Self {
        Self {
            registry,
            registered: Vec::new(),
        }
    }

    fn register(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: RegisterParams = match params {
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

        let info = NodeInfo {
            node_id: params.node_id.clone(),
            name: params.name,
            version: params.version,
            services: params.services.unwrap_or_default(),
        };

        let mut registry = match self.registry.lock() {
            Ok(r) => r,
            Err(_) => {
                return Response::error(req_id, error_codes::INTERNAL_ERROR, "registry lock failed")
            }
        };

        registry.register(info);
        if !self.registered.contains(&params.node_id) {
            self.registered.push(params.node_id);
        }

        Response::success(req_id, json!({ "ok": true }))
    }

    fn heartbeat(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: HeartbeatParams = match params {
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

        let mut registry = match self.registry.lock() {
            Ok(r) => r,
            Err(_) => {
                return Response::error(req_id, error_codes::INTERNAL_ERROR, "registry lock failed")
            }
        };

        match registry.heartbeat(&params.node_id) {
            Ok(()) => Response::success(req_id, json!({ "ok": true })),
            Err(e) => Response::error(req_id, error_codes::INVALID_PARAMS, e),
        }
    }

    fn unregister(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: UnregisterParams = match params {
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

        let mut registry = match self.registry.lock() {
            Ok(r) => r,
            Err(_) => {
                return Response::error(req_id, error_codes::INTERNAL_ERROR, "registry lock failed")
            }
        };

        let ok = registry.unregister(&params.node_id);
        Response::success(req_id, json!({ "ok": ok }))
    }

    fn list(&self, req_id: Uuid) -> Response {
        let registry = match self.registry.lock() {
            Ok(r) => r,
            Err(_) => {
                return Response::error(req_id, error_codes::INTERNAL_ERROR, "registry lock failed")
            }
        };

        let nodes = registry.list();
        Response::success(req_id, json!({ "nodes": nodes }))
    }
}

impl ServiceHandler for PresenceService {
    fn namespace(&self) -> &str {
        "presence"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let resp = match method {
            "presence.register" => self.register(id, params),
            "presence.heartbeat" => self.heartbeat(id, params),
            "presence.unregister" => self.unregister(id, params),
            "presence.list" => self.list(id),
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

    fn shutdown(&mut self) {
        if self.registered.is_empty() {
            return;
        }

        let mut registry = match self.registry.lock() {
            Ok(r) => r,
            Err(_) => return,
        };

        for node_id in self.registered.drain(..) {
            registry.unregister(&node_id);
        }
    }
}

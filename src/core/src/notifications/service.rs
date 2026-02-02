use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};

use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{NotificationEvent, NotificationSubscription, Store};

#[derive(Debug, Deserialize)]
struct RegisterParams {
    target: String,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendParams {
    title: String,
    body: String,
    target: Option<String>,
}

/// Notifications service backed by the persistent store.
pub struct NotificationsService {
    store: Arc<dyn Store>,
    outbound_tx: tokio::sync::mpsc::Sender<OutboundMessage>,
}

impl NotificationsService {
    pub fn new(
        store: Arc<dyn Store>,
        outbound_tx: tokio::sync::mpsc::Sender<OutboundMessage>,
    ) -> Self {
        Self { store, outbound_tx }
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

        let now = now_unix();
        let subscription = NotificationSubscription {
            subscription_id: Uuid::new_v4().to_string(),
            target: params.target,
            kind: params.kind,
            created_at: now,
            updated_at: now,
        };

        if let Err(e) = self.store.upsert_notification_subscription(&subscription) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "subscription": subscription }))
    }

    fn list(&mut self, req_id: Uuid) -> Response {
        match self.store.list_notification_subscriptions() {
            Ok(subs) => Response::success(req_id, json!({ "subscriptions": subs })),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }

    fn send(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: SendParams = match params {
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

        if let Some(target) = params.target.as_deref() {
            match self.store.has_notification_target(target) {
                Ok(true) => {}
                Ok(false) => {
                    return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown target")
                }
                Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
            }
        }

        let notification_id = Uuid::new_v4().to_string();
        let now = now_unix();
        let event = NotificationEvent {
            notification_id: notification_id.clone(),
            title: params.title,
            body: params.body,
            target: params.target.clone(),
            created_at: now,
        };

        if let Err(e) = self.store.insert_notification_event(&event) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        let payload = json!({
            "notification_id": event.notification_id,
            "title": event.title,
            "body": event.body,
            "target": event.target,
            "created_at": event.created_at,
        });

        let _ = self.outbound_tx.try_send(OutboundMessage::event(
            "notifications.sent",
            Some(payload.clone()),
        ));

        Response::success(req_id, json!({ "notification_id": notification_id }))
    }
}

impl ServiceHandler for NotificationsService {
    fn namespace(&self) -> &str {
        "notifications"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let resp = match method {
            "notifications.register" => self.register(id, params),
            "notifications.list" => self.list(id),
            "notifications.send" => self.send(id, params),
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

use axum::extract::ws::{Message, WebSocket};
use futures::{stream::SplitSink, SinkExt};
use homie_protocol::{
    decode_message, encode_message, error_codes, Message as ProtoMessage, Response,
};
use serde_json::json;
use serde_json::Value;
use uuid::Uuid;

use crate::authz::{scope_for_method, AuthContext};
use crate::connection::legacy::{decode_legacy_request, LegacyDecode};
use crate::router::{MessageRouter, SubscriptionManager};

pub(super) async fn handle_text_message(
    sink: &mut SplitSink<WebSocket, Message>,
    text: &str,
    authz: AuthContext,
    router: &mut MessageRouter,
    subscriptions: &mut SubscriptionManager,
) {
    match decode_message(text) {
        Ok(ProtoMessage::Request(req)) => {
            tracing::debug!(method = %req.method, id = %req.id, "request");
            route_and_respond(
                sink,
                authz,
                router,
                subscriptions,
                RouteRequest {
                    req_id: req.id,
                    method: req.method,
                    params: req.params,
                    response_id_override: None,
                },
            )
            .await;
        }
        Ok(other) => {
            tracing::debug!(?other, "non-request message from client (ignored)");
        }
        Err(e) => match decode_legacy_request(text) {
            Some(LegacyDecode::Request(legacy)) => {
                tracing::debug!(
                    method = %legacy.method,
                    client_id = %legacy.response_id,
                    internal_id = %legacy.req_id,
                    "legacy request id accepted"
                );
                route_and_respond(
                    sink,
                    authz,
                    router,
                    subscriptions,
                    RouteRequest {
                        req_id: legacy.req_id,
                        method: legacy.method,
                        params: legacy.params,
                        response_id_override: Some(legacy.response_id),
                    },
                )
                .await;
            }
            Some(LegacyDecode::NonRequest) => {
                tracing::debug!("non-request legacy message from client (ignored)");
            }
            None => {
                tracing::warn!("failed to decode message: {e}");
            }
        },
    }
}

async fn route_and_respond(
    sink: &mut SplitSink<WebSocket, Message>,
    authz: AuthContext,
    router: &mut MessageRouter,
    subscriptions: &mut SubscriptionManager,
    req: RouteRequest,
) {
    let RouteRequest {
        req_id,
        method,
        params,
        response_id_override,
    } = req;

    if let Some(scope) = scope_for_method(&method) {
        if !authz.allows(scope) {
            let resp = Response::error(req_id, error_codes::UNAUTHORIZED, "unauthorized");
            send_response(sink, resp, response_id_override).await;
            return;
        }
    }

    // Handle built-in subscription methods.
    let resp = match method.as_str() {
        "events.subscribe" => handle_subscribe(req_id, params, subscriptions),
        "events.unsubscribe" => handle_unsubscribe(req_id, params, subscriptions),
        "agent.chat.event.subscribe" | "agent.codex.event.subscribe" => {
            let params = agent_subscribe_params(params);
            handle_subscribe(req_id, params, subscriptions)
        }
        "chat.event.subscribe" => {
            let params = chat_subscribe_params(params);
            handle_subscribe(req_id, params, subscriptions)
        }
        _ => router.route_request(req_id, &method, params).await,
    };

    send_response(sink, resp, response_id_override).await;
}

async fn send_response(
    sink: &mut SplitSink<WebSocket, Message>,
    resp: Response,
    response_id_override: Option<Value>,
) {
    match response_id_override {
        None => {
            let msg = ProtoMessage::Response(resp);
            if let Ok(json) = encode_message(&msg) {
                let _ = sink.send(Message::Text(json.into())).await;
            }
        }
        Some(override_id) => {
            let mut payload = match serde_json::to_value(&resp) {
                Ok(v) => v,
                Err(_) => return,
            };
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("type".into(), json!("response"));
                obj.insert("id".into(), override_id);
                if obj.get("result").is_some_and(Value::is_null) {
                    obj.remove("result");
                }
                if obj.get("error").is_some_and(Value::is_null) {
                    obj.remove("error");
                }
            }
            if let Ok(text) = serde_json::to_string(&payload) {
                let _ = sink.send(Message::Text(text.into())).await;
            }
        }
    }
}

fn handle_subscribe(
    req_id: Uuid,
    params: Option<serde_json::Value>,
    subs: &mut SubscriptionManager,
) -> Response {
    let topic = params
        .as_ref()
        .and_then(|p| p.get("topic"))
        .and_then(|v| v.as_str());

    match topic {
        Some(pattern) => {
            let sub_id = subs.subscribe(pattern);
            tracing::debug!(%sub_id, pattern, "subscribed");
            Response::success(req_id, json!({ "subscription_id": sub_id }))
        }
        None => Response::error(
            req_id,
            error_codes::INVALID_PARAMS,
            "missing 'topic' parameter",
        ),
    }
}

fn handle_unsubscribe(
    req_id: Uuid,
    params: Option<serde_json::Value>,
    subs: &mut SubscriptionManager,
) -> Response {
    let sub_id = params
        .as_ref()
        .and_then(|p| p.get("subscription_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok());

    match sub_id {
        Some(id) => {
            if subs.unsubscribe(id) {
                tracing::debug!(%id, "unsubscribed");
                Response::success(req_id, json!({ "ok": true }))
            } else {
                Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    "subscription not found",
                )
            }
        }
        None => Response::error(
            req_id,
            error_codes::INVALID_PARAMS,
            "missing or invalid 'subscription_id'",
        ),
    }
}

fn agent_subscribe_params(params: Option<serde_json::Value>) -> Option<serde_json::Value> {
    let mut params = params.unwrap_or_else(|| json!({}));
    match params.as_object_mut() {
        Some(map) => {
            map.entry("topic".to_string())
                .or_insert_with(|| json!("agent.chat.*"));
            Some(params)
        }
        None => Some(json!({ "topic": "agent.chat.*" })),
    }
}

fn chat_subscribe_params(params: Option<serde_json::Value>) -> Option<serde_json::Value> {
    let mut params = params.unwrap_or_else(|| json!({}));
    match params.as_object_mut() {
        Some(map) => {
            map.entry("topic".to_string())
                .or_insert_with(|| json!("chat.*"));
            Some(params)
        }
        None => Some(json!({ "topic": "chat.*" })),
    }
}

struct RouteRequest {
    req_id: Uuid,
    method: String,
    params: Option<Value>,
    response_id_override: Option<Value>,
}

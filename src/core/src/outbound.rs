use axum::extract::ws::Message as WsMessage;
use serde_json::Value;

/// Messages emitted by services to the WS connection loop.
#[derive(Debug)]
pub enum OutboundMessage {
    Raw(WsMessage),
    Event {
        topic: String,
        params: Option<Value>,
    },
}

impl OutboundMessage {
    pub fn raw(msg: WsMessage) -> Self {
        Self::Raw(msg)
    }

    pub fn event(topic: impl Into<String>, params: Option<Value>) -> Self {
        Self::Event {
            topic: topic.into(),
            params,
        }
    }
}

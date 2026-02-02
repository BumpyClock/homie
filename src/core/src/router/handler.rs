use serde_json::Value;
use uuid::Uuid;

use homie_protocol::{BinaryFrame, Response};

/// Event emitted by a service that should be published to subscribers.
#[derive(Debug, Clone)]
pub struct ReapEvent {
    pub topic: String,
    pub params: Option<Value>,
}

impl ReapEvent {
    pub fn new(topic: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            topic: topic.into(),
            params,
        }
    }
}

/// Trait implemented by each service (terminal, agent.chat, etc.).
///
/// Services are connection-scoped: one instance per WS connection, dropped
/// when the connection closes.
pub trait ServiceHandler: Send {
    /// The service namespace prefix (e.g. "terminal").
    fn namespace(&self) -> &str;

    /// Handle an RPC request. `method` is the full dotted method name
    /// (e.g. "terminal.session.start").
    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>>;

    /// Handle an inbound binary frame (e.g. PTY stdin).
    fn handle_binary(&mut self, frame: &BinaryFrame);

    /// Poll for events to publish (e.g. exited sessions).
    /// Called periodically by the message loop.
    fn reap(&mut self) -> Vec<ReapEvent>;

    /// Graceful shutdown — clean up resources.
    fn shutdown(&mut self);
}

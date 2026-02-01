use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};

use super::handler::{ReapEvent, ServiceHandler};

/// Routes RPC requests to the correct service handler based on method prefix.
///
/// Method names use `service.method` convention (e.g. "terminal.session.start").
/// The router extracts the first dotted segment as the namespace and delegates
/// to the matching `ServiceHandler`.
pub struct MessageRouter {
    /// namespace → handler
    services: HashMap<String, Box<dyn ServiceHandler>>,
}

impl MessageRouter {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service handler. The handler's `namespace()` is used as key.
    pub fn register(&mut self, handler: Box<dyn ServiceHandler>) {
        let ns = handler.namespace().to_string();
        self.services.insert(ns, handler);
    }

    /// Extract namespace from a dotted method name.
    /// e.g. "terminal.session.start" → "terminal"
    fn extract_namespace(method: &str) -> Option<&str> {
        method.split('.').next().filter(|s| !s.is_empty())
    }

    /// Route an RPC request to the appropriate service handler.
    pub async fn route_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> Response {
        let ns = match Self::extract_namespace(method) {
            Some(ns) => ns,
            None => {
                return Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("invalid method format: {method}"),
                )
            }
        };

        // Look up the handler by namespace.
        match self.services.get_mut(ns) {
            Some(handler) => handler.handle_request(id, method, params).await,
            None => Response::error(
                id,
                error_codes::METHOD_NOT_FOUND,
                format!("unknown service: {ns}"),
            ),
        }
    }

    /// Route a binary frame. Currently uses the "terminal" service for all
    /// binary frames (PTY stdin). Future services may use a header byte to
    /// disambiguate.
    pub fn route_binary(&mut self, frame: &BinaryFrame) {
        // Binary frames are currently always PTY data → terminal service.
        // Future: could add a service discriminator to the frame header.
        if let Some(handler) = self.services.get_mut("terminal") {
            handler.handle_binary(frame);
        } else {
            tracing::debug!("no terminal service registered for binary frame");
        }
    }

    /// Poll all services for reap events.
    pub fn reap_all(&mut self) -> Vec<ReapEvent> {
        let mut events = Vec::new();
        for handler in self.services.values_mut() {
            events.extend(handler.reap());
        }
        events
    }

    /// Shutdown all registered services.
    pub fn shutdown_all(&mut self) {
        for handler in self.services.values_mut() {
            handler.shutdown();
        }
    }

    /// List registered namespace names.
    pub fn namespaces(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use homie_protocol::{error_codes, StreamType};
    use serde_json::json;
    use std::pin::Pin;

    /// Minimal stub service for testing the router.
    struct StubService {
        ns: String,
        last_method: Option<String>,
        binary_count: usize,
        reap_events: Vec<ReapEvent>,
    }

    impl StubService {
        fn new(ns: &str) -> Self {
            Self {
                ns: ns.to_string(),
                last_method: None,
                binary_count: 0,
                reap_events: vec![],
            }
        }
    }

    impl ServiceHandler for StubService {
        fn namespace(&self) -> &str {
            &self.ns
        }

        fn handle_request(
            &mut self,
            id: Uuid,
            method: &str,
            _params: Option<Value>,
        ) -> Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
            self.last_method = Some(method.to_string());
            let resp = Response::success(id, json!({ "routed_to": self.ns }));
            Box::pin(async move { resp })
        }

        fn handle_binary(&mut self, _frame: &BinaryFrame) {
            self.binary_count += 1;
        }

        fn reap(&mut self) -> Vec<ReapEvent> {
            std::mem::take(&mut self.reap_events)
        }

        fn shutdown(&mut self) {}
    }

    #[tokio::test]
    async fn routes_to_correct_service() {
        let mut router = MessageRouter::new();
        router.register(Box::new(StubService::new("terminal")));
        router.register(Box::new(StubService::new("agent")));

        let id = Uuid::new_v4();
        let resp = router
            .route_request(id, "terminal.session.start", None)
            .await;
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["routed_to"], "terminal");

        let id2 = Uuid::new_v4();
        let resp2 = router.route_request(id2, "agent.chat.create", None).await;
        assert!(resp2.error.is_none());
        assert_eq!(resp2.result.unwrap()["routed_to"], "agent");
    }

    #[tokio::test]
    async fn unknown_service_returns_error() {
        let mut router = MessageRouter::new();
        router.register(Box::new(StubService::new("terminal")));

        let id = Uuid::new_v4();
        let resp = router.route_request(id, "files.list", None).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_method_format() {
        let mut router = MessageRouter::new();
        let id = Uuid::new_v4();
        let resp = router.route_request(id, "", None).await;
        assert!(resp.error.is_some());
    }

    #[test]
    fn binary_routes_to_terminal() {
        let mut router = MessageRouter::new();
        router.register(Box::new(StubService::new("terminal")));

        let frame = BinaryFrame {
            session_id: Uuid::new_v4(),
            stream: StreamType::Stdin,
            payload: vec![0x41],
        };
        router.route_binary(&frame);
        // No panic = success; the stub increments binary_count internally.
    }

    #[test]
    fn reap_collects_from_all_services() {
        let mut svc = StubService::new("terminal");
        svc.reap_events.push(ReapEvent {
            topic: "terminal.session.exit".into(),
            params: Some(json!({"session_id": "abc"})),
        });

        let mut router = MessageRouter::new();
        router.register(Box::new(svc));

        let events = router.reap_all();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].topic, "terminal.session.exit");
    }
}

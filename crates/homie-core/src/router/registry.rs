use homie_protocol::ServiceCapability;

/// Describes a registered service type (not a per-connection instance).
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    /// Namespace prefix (e.g. "terminal", "agent.chat").
    pub namespace: String,
    /// Semver-ish version string.
    pub version: String,
    /// Node ID where this service runs. For local-only, always "local".
    pub node_id: String,
}

/// Registry of available service types.
///
/// Currently local-only: all services run on the same node. The structure
/// supports future multi-node expansion where services may be registered
/// from remote nodes.
#[derive(Debug, Clone, Default)]
pub struct ServiceRegistry {
    entries: Vec<ServiceEntry>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. In multi-node future, `node_id` distinguishes
    /// origin nodes.
    pub fn register(&mut self, namespace: impl Into<String>, version: impl Into<String>) {
        self.entries.push(ServiceEntry {
            namespace: namespace.into(),
            version: version.into(),
            node_id: "local".into(),
        });
    }

    /// Return capabilities for the handshake `ServerHello`.
    pub fn capabilities(&self) -> Vec<ServiceCapability> {
        self.entries
            .iter()
            .map(|e| ServiceCapability {
                service: e.namespace.clone(),
                version: e.version.clone(),
            })
            .collect()
    }

    /// Check if a namespace is registered.
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.entries.iter().any(|e| e.namespace == namespace)
    }

    /// All registered entries (for introspection / future routing).
    pub fn entries(&self) -> &[ServiceEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut reg = ServiceRegistry::new();
        reg.register("terminal", "1.0");
        reg.register("agent.chat", "1.0");

        assert!(reg.has_namespace("terminal"));
        assert!(reg.has_namespace("agent.chat"));
        assert!(!reg.has_namespace("files"));

        let caps = reg.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].service, "terminal");
    }
}

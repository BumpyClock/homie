use serde::{Deserialize, Serialize};

use crate::VersionRange;

/// Client → Server handshake sent as the first text frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Protocol version range the client supports.
    pub protocol: VersionRange,
    /// Human-readable client identifier (e.g. "homie-web/0.1.0").
    pub client_id: String,
    /// Optional authentication token (unused in Tailscale-only MVP).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// Capabilities the client requests (informational).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

/// Server → Client handshake response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Negotiated protocol version.
    pub protocol_version: u16,
    /// Server identifier (e.g. "homie-gateway/0.1.0").
    pub server_id: String,
    /// Authenticated identity (Tailscale user, node name, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// Services available on this gateway.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ServiceCapability>,
}

/// A service capability advertised by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceCapability {
    /// Service namespace (e.g. "terminal", "agent.chat").
    pub service: String,
    /// Service version string.
    pub version: String,
}

/// Handshake rejection sent instead of `ServerHello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloReject {
    /// Machine-readable error code.
    pub code: HelloRejectCode,
    /// Human-readable reason.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelloRejectCode {
    VersionMismatch,
    Unauthorized,
    ServerError,
}

/// Wraps either a successful or rejected handshake response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HandshakeResponse {
    Hello(ServerHello),
    Reject(HelloReject),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VersionRange;

    #[test]
    fn client_hello_roundtrip() {
        let hello = ClientHello {
            protocol: VersionRange::new(1, 1),
            client_id: "homie-web/0.1.0".into(),
            auth_token: None,
            capabilities: vec!["terminal".into()],
        };
        let json = serde_json::to_string(&hello).unwrap();
        let decoded: ClientHello = serde_json::from_str(&json).unwrap();
        assert_eq!(hello, decoded);
    }

    #[test]
    fn server_hello_roundtrip() {
        let hello = ServerHello {
            protocol_version: 1,
            server_id: "homie-gateway/0.1.0".into(),
            identity: Some("user@tailnet".into()),
            services: vec![ServiceCapability {
                service: "terminal".into(),
                version: "1.0".into(),
            }],
        };
        let json = serde_json::to_string(&hello).unwrap();
        let decoded: ServerHello = serde_json::from_str(&json).unwrap();
        assert_eq!(hello, decoded);
    }

    #[test]
    fn reject_roundtrip() {
        let reject = HandshakeResponse::Reject(HelloReject {
            code: HelloRejectCode::VersionMismatch,
            reason: "server requires protocol >= 2".into(),
        });
        let json = serde_json::to_string(&reject).unwrap();
        let decoded: HandshakeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(reject, decoded);
    }

    #[test]
    fn client_hello_optional_fields_omitted() {
        let hello = ClientHello {
            protocol: VersionRange::default(),
            client_id: "test".into(),
            auth_token: None,
            capabilities: vec![],
        };
        let json = serde_json::to_string(&hello).unwrap();
        assert!(!json.contains("auth_token"));
        assert!(!json.contains("capabilities"));
    }
}

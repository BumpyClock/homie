use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// Identity resolved from Tailscale headers + whois verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailscaleIdentity {
    pub login: String,
    pub display_name: String,
    pub profile_pic: Option<String>,
    pub tailnet: Option<String>,
}

/// Result of authentication attempt.
#[derive(Debug, Clone)]
pub enum AuthOutcome {
    /// Loopback connection — trusted without identity headers.
    Local,
    /// LAN connection — trusted without identity headers.
    Lan,
    /// Tailscale Serve identity verified.
    Tailscale(TailscaleIdentity),
    /// Rejected with reason.
    Rejected(String),
}

fn is_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(v6) => v6.is_unique_local(),
    }
}

impl AuthOutcome {
    pub fn is_ok(&self) -> bool {
        !matches!(self, Self::Rejected(_))
    }

    /// Friendly identity string for ServerHello.
    pub fn identity_string(&self) -> Option<String> {
        match self {
            Self::Tailscale(id) => Some(id.login.clone()),
            Self::Local => Some("local".into()),
            Self::Lan => Some("lan".into()),
            Self::Rejected(_) => None,
        }
    }
}

// ── Tailscale header extraction ──────────────────────────────────────

const HDR_USER_LOGIN: &str = "tailscale-user-login";
const HDR_USER_NAME: &str = "tailscale-user-name";
const HDR_USER_PIC: &str = "tailscale-user-profile-pic";

/// Extract Tailscale identity from Serve-injected headers.
fn extract_tailscale_headers(headers: &HeaderMap) -> Option<TailscaleIdentity> {
    let login = headers
        .get(HDR_USER_LOGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    let display_name = headers
        .get(HDR_USER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| login.clone());

    let profile_pic = headers
        .get(HDR_USER_PIC)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(TailscaleIdentity {
        login,
        display_name,
        profile_pic,
        tailnet: None,
    })
}

/// Check that the request came through Tailscale Serve (proxy headers
/// present AND connection is from loopback).
fn is_tailscale_proxy(headers: &HeaderMap, remote_ip: IpAddr) -> bool {
    let has_proxy = headers.contains_key("x-forwarded-for")
        && headers.contains_key("x-forwarded-proto")
        && headers.contains_key("x-forwarded-host");
    has_proxy && remote_ip.is_loopback()
}

/// Extract the real client IP from `x-forwarded-for`.
fn forwarded_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ── tailscale whois ──────────────────────────────────────────────────

/// Object-safe whois lookup trait.
pub trait TailscaleWhois: Send + Sync + 'static {
    fn whois(&self, ip: &str) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>>;
}

/// Real implementation that shells out to `tailscale whois --json`.
#[derive(Debug, Clone, Default)]
pub struct LiveWhois;

impl TailscaleWhois for LiveWhois {
    fn whois(&self, ip: &str) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>> {
        let ip = ip.to_string();
        Box::pin(async move {
            let output = Command::new("tailscale")
                .args(["whois", "--json", &ip])
                .output()
                .await
                .ok()?;

            if !output.status.success() {
                tracing::warn!(ip, "tailscale whois failed");
                return None;
            }

            let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

            let login = parsed
                .pointer("/UserProfile/LoginName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())?;

            let display_name = parsed
                .pointer("/UserProfile/DisplayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| login.clone());

            let profile_pic = parsed
                .pointer("/UserProfile/ProfilePicURL")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let tailnet = parsed
                .pointer("/Node/ComputedName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            Some(TailscaleIdentity {
                login,
                display_name,
                profile_pic,
                tailnet,
            })
        })
    }
}

// ── Resolve auth for an incoming WS upgrade ──────────────────────────

/// Authenticate an incoming connection.
///
/// - Loopback connections without Tailscale Serve → `AuthOutcome::Local`
/// - Tailscale Serve proxy → extract headers, cross-verify via whois
/// - Otherwise → rejected
pub async fn authenticate(
    headers: &HeaderMap,
    remote_ip: IpAddr,
    tailscale_serve: bool,
    allow_lan: bool,
    whois: &Arc<dyn TailscaleWhois>,
) -> AuthOutcome {
    // When not behind Tailscale Serve, only allow loopback.
    if !tailscale_serve {
        return if remote_ip.is_loopback() {
            AuthOutcome::Local
        } else if allow_lan && is_lan_ip(remote_ip) {
            AuthOutcome::Lan
        } else {
            AuthOutcome::Rejected("non-loopback without tailscale serve".into())
        };
    }

    // Behind Tailscale Serve: verify proxy headers.
    if !is_tailscale_proxy(headers, remote_ip) {
        return if remote_ip.is_loopback() {
            // Direct loopback access even when serve is enabled.
            AuthOutcome::Local
        } else if allow_lan && is_lan_ip(remote_ip) {
            AuthOutcome::Lan
        } else {
            AuthOutcome::Rejected("missing tailscale proxy headers".into())
        };
    }

    // Extract identity headers.
    let header_id = match extract_tailscale_headers(headers) {
        Some(id) => id,
        None => return AuthOutcome::Rejected("tailscale identity headers missing".into()),
    };

    // Cross-verify with whois.
    let client_ip = match forwarded_client_ip(headers) {
        Some(ip) => ip,
        None => return AuthOutcome::Rejected("x-forwarded-for missing".into()),
    };

    let whois_id = match whois.whois(&client_ip).await {
        Some(id) => id,
        None => return AuthOutcome::Rejected("tailscale whois failed".into()),
    };

    // Compare logins case-insensitively.
    if header_id.login.to_lowercase() != whois_id.login.to_lowercase() {
        return AuthOutcome::Rejected(format!(
            "identity mismatch: header={} whois={}",
            header_id.login, whois_id.login,
        ));
    }

    AuthOutcome::Tailscale(TailscaleIdentity {
        login: whois_id.login,
        display_name: whois_id.display_name,
        profile_pic: header_id.profile_pic.or(whois_id.profile_pic),
        tailnet: whois_id.tailnet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::net::{IpAddr, Ipv4Addr};

    /// Stub whois that returns a fixed identity.
    struct StubWhois(Option<TailscaleIdentity>);

    impl TailscaleWhois for StubWhois {
        fn whois(
            &self,
            _ip: &str,
        ) -> Pin<Box<dyn Future<Output = Option<TailscaleIdentity>> + Send>> {
            let result = self.0.clone();
            Box::pin(async move { result })
        }
    }

    fn stub(id: Option<TailscaleIdentity>) -> Arc<dyn TailscaleWhois> {
        Arc::new(StubWhois(id))
    }

    fn loopback() -> IpAddr {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }

    fn remote() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))
    }

    fn make_tailscale_headers(login: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(HDR_USER_LOGIN, HeaderValue::from_str(login).unwrap());
        h.insert(HDR_USER_NAME, HeaderValue::from_static("Test User"));
        h.insert("x-forwarded-for", HeaderValue::from_static("100.64.0.1"));
        h.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        h.insert("x-forwarded-host", HeaderValue::from_static("host.ts.net"));
        h
    }

    #[tokio::test]
    async fn loopback_no_serve_is_local() {
        let result = authenticate(&HeaderMap::new(), loopback(), false, false, &stub(None)).await;
        assert!(matches!(result, AuthOutcome::Local));
    }

    #[tokio::test]
    async fn remote_no_serve_is_rejected() {
        let result = authenticate(&HeaderMap::new(), remote(), false, false, &stub(None)).await;
        assert!(matches!(result, AuthOutcome::Rejected(_)));
    }

    #[tokio::test]
    async fn tailscale_serve_valid() {
        let headers = make_tailscale_headers("alice");
        let whois = stub(Some(TailscaleIdentity {
            login: "alice".into(),
            display_name: "Alice".into(),
            profile_pic: None,
            tailnet: Some("mynet".into()),
        }));

        let result = authenticate(&headers, loopback(), true, false, &whois).await;
        match result {
            AuthOutcome::Tailscale(id) => {
                assert_eq!(id.login, "alice");
                assert_eq!(id.tailnet.as_deref(), Some("mynet"));
            }
            other => panic!("expected Tailscale, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn tailscale_serve_mismatch_rejected() {
        let headers = make_tailscale_headers("alice");
        let whois = stub(Some(TailscaleIdentity {
            login: "bob".into(),
            display_name: "Bob".into(),
            profile_pic: None,
            tailnet: None,
        }));

        let result = authenticate(&headers, loopback(), true, false, &whois).await;
        assert!(matches!(result, AuthOutcome::Rejected(_)));
    }

    #[tokio::test]
    async fn tailscale_serve_whois_fails_rejected() {
        let headers = make_tailscale_headers("alice");
        let result = authenticate(&headers, loopback(), true, false, &stub(None)).await;
        assert!(matches!(result, AuthOutcome::Rejected(_)));
    }

    #[tokio::test]
    async fn serve_enabled_but_direct_loopback_is_local() {
        let result = authenticate(&HeaderMap::new(), loopback(), true, false, &stub(None)).await;
        assert!(matches!(result, AuthOutcome::Local));
    }

    #[tokio::test]
    async fn serve_enabled_non_loopback_no_proxy_rejected() {
        let result = authenticate(&HeaderMap::new(), remote(), true, false, &stub(None)).await;
        assert!(matches!(result, AuthOutcome::Rejected(_)));
    }

    #[tokio::test]
    async fn case_insensitive_login_match() {
        let headers = make_tailscale_headers("Alice");
        let whois = stub(Some(TailscaleIdentity {
            login: "alice".into(),
            display_name: "Alice".into(),
            profile_pic: None,
            tailnet: None,
        }));

        let result = authenticate(&headers, loopback(), true, false, &whois).await;
        assert!(matches!(result, AuthOutcome::Tailscale(_)));
    }
}

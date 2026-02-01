use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use homie_core::{build_router, LiveWhois, Role, ServerConfig, SqliteStore};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let defaults = ServerConfig::default();

    let bind = parse_socket("HOMIE_BIND", defaults.bind);
    let tailnet_bind = parse_optional_socket("HOMIE_TAILNET_BIND");
    let tailscale_serve = parse_bool("HOMIE_TAILSCALE_SERVE", defaults.tailscale_serve);
    let heartbeat_interval = parse_duration("HOMIE_HEARTBEAT_SECS", defaults.heartbeat_interval);
    let idle_timeout = parse_duration("HOMIE_IDLE_SECS", defaults.idle_timeout);
    let node_timeout = parse_duration("HOMIE_NODE_TIMEOUT_SECS", defaults.node_timeout);
    let job_retention_days = parse_u64("HOMIE_JOB_RETENTION_DAYS", defaults.job_retention_days);
    let job_max_records = parse_usize("HOMIE_JOB_MAX_RECORDS", defaults.job_max_records);
    let pairing_retention_secs = parse_u64(
        "HOMIE_PAIRING_RETENTION_SECS",
        defaults.pairing_retention_secs,
    );
    let pairing_default_ttl_secs =
        parse_u64("HOMIE_PAIRING_TTL_SECS", defaults.pairing_default_ttl_secs);
    let notification_retention_days = parse_u64(
        "HOMIE_NOTIFICATION_RETENTION_DAYS",
        defaults.notification_retention_days,
    );
    let local_role = parse_role("HOMIE_LOCAL_ROLE", defaults.local_role);
    let tailscale_role = parse_role("HOMIE_TAILSCALE_ROLE", defaults.tailscale_role);

    let config = ServerConfig {
        bind,
        tailnet_bind,
        tailscale_serve,
        heartbeat_interval,
        idle_timeout,
        local_role,
        tailscale_role,
        node_timeout,
        job_retention_days,
        job_max_records,
        pairing_retention_secs,
        pairing_default_ttl_secs,
        notification_retention_days,
    };

    let db_path = env::var("HOMIE_DB_PATH").unwrap_or_else(|_| "homie.db".to_string());
    let store = SqliteStore::open(Path::new(&db_path))?;
    let store = Arc::new(store);

    let app = build_router(config.clone(), LiveWhois, store);

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(addr = %config.bind, "listening");

    if let Some(addr) = config.tailnet_bind {
        let app_tailnet = app.clone();
        tokio::spawn(async move {
            let listener = TcpListener::bind(addr).await;
            let listener = match listener {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(%addr, error = %e, "tailnet bind failed");
                    return;
                }
            };
            tracing::info!(addr = %addr, "tailnet listening");
            if let Err(e) = axum::serve(
                listener,
                app_tailnet.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
                tracing::error!(error = %e, "tailnet server error");
            }
        });
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn parse_socket(key: &str, default: SocketAddr) -> SocketAddr {
    match env::var(key) {
        Ok(v) => v.parse().unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_optional_socket(key: &str) -> Option<SocketAddr> {
    env::var(key).ok().and_then(|v| v.parse().ok())
}

fn parse_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => default,
    }
}

fn parse_duration(key: &str, default: Duration) -> Duration {
    match env::var(key) {
        Ok(v) => v.parse::<u64>().map(Duration::from_secs).unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_u64(key: &str, default: u64) -> u64 {
    match env::var(key) {
        Ok(v) => v.parse::<u64>().unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_usize(key: &str, default: usize) -> usize {
    match env::var(key) {
        Ok(v) => v.parse::<usize>().unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_role(key: &str, default: Role) -> Role {
    match env::var(key) {
        Ok(v) => match v.as_str() {
            "owner" | "OWNER" => Role::Owner,
            "user" | "USER" => Role::User,
            "viewer" | "VIEWER" => Role::Viewer,
            _ => default,
        },
        Err(_) => default,
    }
}

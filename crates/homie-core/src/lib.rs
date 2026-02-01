mod auth;
mod config;
mod connection;
mod server;

pub use auth::{AuthOutcome, LiveWhois, TailscaleIdentity, TailscaleWhois};
pub use config::ServerConfig;
pub use connection::Connection;
pub use server::build_router;

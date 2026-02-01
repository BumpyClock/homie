mod auth;
mod config;
mod connection;
mod server;
pub mod terminal;

pub use auth::{AuthOutcome, LiveWhois, TailscaleIdentity, TailscaleWhois};
pub use config::ServerConfig;
pub use connection::Connection;
pub use server::build_router;
pub use terminal::TerminalService;

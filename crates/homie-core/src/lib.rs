pub mod agent;
mod auth;
mod config;
mod connection;
pub mod router;
mod server;
pub mod terminal;

pub use agent::AgentService;
pub use auth::{AuthOutcome, LiveWhois, TailscaleIdentity, TailscaleWhois};
pub use config::ServerConfig;
pub use connection::Connection;
pub use router::{MessageRouter, ServiceHandler, ServiceRegistry, SubscriptionManager};
pub use server::build_router;
pub use terminal::TerminalService;

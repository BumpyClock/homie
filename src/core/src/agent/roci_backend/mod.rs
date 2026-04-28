mod backend;
mod events;
mod parsing;
mod persistence;
#[cfg(test)]
mod tests;

pub use backend::{ChatBackend, RociBackend, StartRunRequest};

const DEFAULT_ROCI_MODEL: &str = "openai-codex:gpt-5.1-codex";

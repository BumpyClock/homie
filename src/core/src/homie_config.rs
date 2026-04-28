mod config;
mod paths;
mod providers;
#[cfg(test)]
mod tests;
mod tools;

pub use config::HomieConfig;
#[cfg(test)]
pub use providers::GithubCopilotProviderConfig;
pub use providers::{OpenAiCompatibleProviderConfig, ProvidersConfig};
pub use tools::{
    BraveSearchConfig, FirecrawlConfig, SearxngSearchConfig, ToolProviderConfig, ToolsConfig,
    WebFetchBackend, WebFetchConfig, WebSearchConfig, WebToolsConfig,
};

const DEFAULT_SYSTEM_PROMPT: &str = include_str!("../system_prompt.md");

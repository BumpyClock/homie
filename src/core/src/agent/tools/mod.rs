use std::path::PathBuf;
use std::sync::Arc;

use roci::tools::Tool;

use crate::homie_config::WebToolsConfig;
use crate::storage::Store;
use crate::HomieConfig;

mod apply_patch;
mod args;
mod browser;
mod cron;
mod exec;
mod fs;
mod process;
mod process_registry;
mod registry;
mod web;

pub use process_registry::{ProcessInfo, ProcessRegistry};
pub use registry::{ListedTool, ToolProvider, ToolRegistry};

pub const TOOL_CHANNEL_WEB: &str = "web";
pub const TOOL_CHANNEL_MOBILE: &str = "mobile";
pub const TOOL_CHANNEL_WHATSAPP: &str = "whatsapp";
pub const TOOL_CHANNEL_DENIED_CODE: &str = "tool_channel_denied";
pub const CANONICAL_TOOL_CHANNELS: &[&str] =
    &[TOOL_CHANNEL_WEB, TOOL_CHANNEL_MOBILE, TOOL_CHANNEL_WHATSAPP];

#[derive(Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
    pub channel: Option<String>,
    pub processes: Arc<ProcessRegistry>,
    pub web: WebToolsConfig,
    pub store: Option<Arc<dyn Store>>,
}

impl ToolContext {
    #[allow(dead_code)]
    pub fn new(homie_config: Arc<HomieConfig>) -> Self {
        Self::new_with_channel(homie_config, None)
    }

    pub fn new_with_channel(homie_config: Arc<HomieConfig>, channel: Option<&str>) -> Self {
        let processes = Arc::new(ProcessRegistry::new());
        Self::with_processes_and_channel(processes, homie_config, channel)
    }

    pub fn with_store(mut self, store: Arc<dyn Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn with_processes_and_channel(
        processes: Arc<ProcessRegistry>,
        homie_config: Arc<HomieConfig>,
        channel: Option<&str>,
    ) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let web = homie_config.tools.web.clone();
        let channel = resolve_tool_channel(channel);
        Self {
            cwd,
            channel,
            processes,
            web,
            store: None,
        }
    }
}

pub fn resolve_tool_channel(channel: Option<&str>) -> Option<String> {
    let normalized = channel
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())?;
    if CANONICAL_TOOL_CHANNELS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
    {
        Some(normalized)
    } else {
        None
    }
}

pub fn build_tools(
    ctx: ToolContext,
    homie_config: &HomieConfig,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    ToolRegistry::new().build_tools(ctx, &homie_config.tools)
}

pub fn list_tools(ctx: ToolContext, homie_config: &HomieConfig) -> Result<Vec<ListedTool>, String> {
    ToolRegistry::new().list_tools(ctx, &homie_config.tools)
}

pub fn debug_tools_enabled() -> bool {
    matches!(std::env::var("HOMIE_DEBUG").as_deref(), Ok("1"))
        || matches!(std::env::var("HOME_DEBUG").as_deref(), Ok("1"))
}

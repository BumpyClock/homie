use std::path::PathBuf;
use std::sync::Arc;

use roci::tools::Tool;

use crate::homie_config::{OpenClawBrowserToolsConfig, WebToolsConfig};
use crate::HomieConfig;

mod apply_patch;
mod args;
mod exec;
mod fs;
mod openclaw_browser;
mod process;
mod process_registry;
mod registry;
mod web;

pub use process_registry::{ProcessInfo, ProcessRegistry};
pub use registry::{ListedTool, ToolProvider, ToolRegistry};

pub const DEFAULT_TOOL_CHANNEL: &str = "web";

#[derive(Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
    pub channel: String,
    pub processes: Arc<ProcessRegistry>,
    pub web: WebToolsConfig,
    pub openclaw_browser: OpenClawBrowserToolsConfig,
}

impl ToolContext {
    #[allow(dead_code)]
    pub fn new(homie_config: Arc<HomieConfig>) -> Self {
        Self::new_with_channel(homie_config, DEFAULT_TOOL_CHANNEL)
    }

    pub fn new_with_channel(homie_config: Arc<HomieConfig>, channel: &str) -> Self {
        let processes = Arc::new(ProcessRegistry::new());
        Self::with_processes_and_channel(processes, homie_config, channel)
    }

    pub fn with_processes(processes: Arc<ProcessRegistry>, homie_config: Arc<HomieConfig>) -> Self {
        Self::with_processes_and_channel(processes, homie_config, DEFAULT_TOOL_CHANNEL)
    }

    pub fn with_processes_and_channel(
        processes: Arc<ProcessRegistry>,
        homie_config: Arc<HomieConfig>,
        channel: &str,
    ) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let web = homie_config.tools.web.clone();
        let openclaw_browser = homie_config.tools.openclaw_browser.clone();
        let channel = normalize_channel(channel);
        Self {
            cwd,
            channel,
            processes,
            web,
            openclaw_browser,
        }
    }
}

fn normalize_channel(channel: &str) -> String {
    let trimmed = channel.trim();
    if trimmed.is_empty() {
        DEFAULT_TOOL_CHANNEL.to_string()
    } else {
        trimmed.to_lowercase()
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

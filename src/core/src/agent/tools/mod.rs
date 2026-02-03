use std::path::PathBuf;
use std::sync::Arc;

use roci::tools::Tool;


mod apply_patch;
mod exec;
mod fs;
mod process;
mod process_registry;

pub use process_registry::{ProcessInfo, ProcessRegistry};

#[derive(Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
    pub processes: Arc<ProcessRegistry>,
}

impl ToolContext {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cwd,
            processes: Arc::new(ProcessRegistry::new()),
        }
    }
}

pub fn build_tools(ctx: ToolContext) -> Vec<Arc<dyn Tool>> {
    vec![
        fs::read_tool(ctx.clone()),
        fs::ls_tool(ctx.clone()),
        fs::find_tool(ctx.clone()),
        fs::grep_tool(ctx.clone()),
        apply_patch::apply_patch_tool(ctx.clone()),
        exec::exec_tool(ctx.clone()),
        process::process_tool(ctx.clone()),
    ]
}

pub fn debug_tools_enabled() -> bool {
    matches!(std::env::var("HOMIE_DEBUG").as_deref(), Ok("1"))
        || matches!(std::env::var("HOME_DEBUG").as_deref(), Ok("1"))
}
